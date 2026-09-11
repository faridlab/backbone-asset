//! Remove the asset from the books and recognise gain/loss (hand-authored, user-owned).
//!
//! An `impl AssetWriteService` chunk over the vocabulary in [`super::asset_write_service`]: lock the
//! asset row, read `accumulated_depreciation` UNDER the lock (held across the post + status flip —
//! a concurrent `run_depreciation` takes the same row lock, so it cannot advance accumulated between
//! the read and the disposal post), then post
//! `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset` and flip status to `disposed`.
//! Idempotent (the disposal post dedupes by `dispose:{asset_id}`, and the status gate is a
//! compare-and-set).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `AssetRepository` / `AssetCategoryRepository`.

use rust_decimal::Decimal;
use uuid::Uuid;

use super::asset_events::{AssetDisposed, AssetEvent, AssetEventSink};
use super::asset_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};

use super::asset_write_service::{legacy_company_echo, relay_ambient_scope, AssetError, AssetWriteService, DisposalOutcome};

impl AssetWriteService {
    /// Dispose the asset: remove it from the books and recognise gain/loss.
    /// `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset`. Idempotent (post + status gate).
    ///
    /// The lookup is id-only (ADR-0029): on a composed deployment the caller's ambient org scope —
    /// re-bound onto the transaction below — fences the locked read, so an out-of-scope asset is
    /// simply not found.
    pub async fn dispose_asset(
        &self,
        asset_id: Uuid,
        proceeds: Decimal,
        proceeds_account_id: Uuid,
        at: chrono::NaiveDate,
        gl: &dyn GlPostSink,
        sink: &dyn AssetEventSink,
    ) -> Result<DisposalOutcome, AssetError> {
        if proceeds < Decimal::ZERO {
            return Err(AssetError::Invalid("proceeds must be non-negative".into()));
        }
        // Lock the asset row and read `accumulated_depreciation` UNDER the lock, held across the post +
        // status flip. A concurrent `run_depreciation` also takes this row lock, so it cannot advance
        // accumulated between this read and the disposal post — the Dr Accum Dep amount always matches
        // what depreciation actually credited, and the asset nets off the books (council 2026-07-06).
        //
        // The transaction this service opened carries no ambient org scope (task-local) — re-bind it
        // so a composed deployment's fence sees the caller on the row lock and the status flip.
        let mut tx = self.pool.begin().await?;
        relay_ambient_scope(&mut tx).await?;
        let row = self
            .assets
            .lock_for_disposal(&mut tx, asset_id)
            .await?
            .ok_or(AssetError::NotFound("asset"))?;
        let status: String = row.status;
        let gross: Decimal = row.gross_purchase_amount;
        let accumulated: Decimal = row.accumulated_depreciation;
        let nbv = gross - accumulated;
        if status == "disposed" {
            tx.rollback().await?;
            return Ok(DisposalOutcome { net_book_value: nbv, gain_loss: Decimal::ZERO, already: true });
        }
        if status != "active" && status != "fully_depreciated" {
            tx.rollback().await?;
            return Err(AssetError::InvalidState("asset is not disposable"));
        }
        let category_id: Uuid = row.asset_category_id;
        let asset_code: String = row.asset_code;
        let cat = self.load_category(category_id).await?;
        let gain_loss = proceeds - nbv; // + gain, − loss

        // Build the balanced disposal envelope from the locked-in accumulated.
        // The Dr Accum Dep line is emitted ONLY when something was actually depreciated — omitting it
        // when accumulated == 0 (e.g. disposal wins the row-lock race before any depreciation posts)
        // avoids a zero-amount debit line that some ledgers reject. The post stays balanced either way
        // (a zero debit contributes nothing), so this also keeps dispose coherent under the race.
        let mut lines = vec![
            GlPostLine::credit(cat.fixed_asset, gross).with_description("Fixed asset"),
        ];
        if accumulated > Decimal::ZERO {
            lines.push(GlPostLine::debit(cat.accum_dep, accumulated).with_description("Accumulated depreciation"));
        }
        if proceeds > Decimal::ZERO {
            lines.push(GlPostLine::debit(proceeds_account_id, proceeds).with_description("Disposal proceeds"));
        }
        if gain_loss > Decimal::ZERO {
            lines.push(GlPostLine::credit(cat.gain_loss, gain_loss).with_description("Gain on disposal"));
        } else if gain_loss < Decimal::ZERO {
            lines.push(GlPostLine::debit(cat.gain_loss, -gain_loss).with_description("Loss on disposal"));
        }
        let env = AccountingPostEnvelope {
            idempotency_key: format!("dispose:{asset_id}"),
            // The legacy tenancy twin echo (ADR-0029): nil when no ambient scope is bound.
            company_id: legacy_company_echo(),
            branch_id: None,
            source_type: "asset".into(),
            source_id: Uuid::new_v5(&asset_id, b"asset:dispose"),
            source_reference: Some(asset_code),
            posting_date: at,
            currency: "IDR".into(),
            posting_type: "original".into(),
            description: Some("asset disposal".into()),
            lines,
        };
        if !env.is_balanced() {
            tx.rollback().await?;
            return Err(AssetError::Invalid("unbalanced posting".into()));
        }
        // Post under the lock; on error the tx rolls back (status unchanged), a retry re-posts (dedup).
        if let Err(e) = gl.post(&env).await {
            tx.rollback().await?;
            return Err(AssetError::Gl(e.code));
        }
        self.assets.mark_disposed(&mut tx, asset_id).await?;
        tx.commit().await?;
        sink.publish(&AssetEvent::AssetDisposed(AssetDisposed {
            asset_id,
            company_id: legacy_company_echo(),
            proceeds,
            net_book_value: nbv,
            gain_loss,
        }));
        Ok(DisposalOutcome { net_book_value: nbv, gain_loss, already: false })
    }
}
