//! Post every due straight-line schedule period (hand-authored, user-owned).
//!
//! An `impl AssetWriteService` chunk over the vocabulary in [`super::asset_write_service`]: for each
//! period due on or before `up_to`, post `Dr Depreciation Expense · Cr Accum Dep` under a per-period
//! transaction that holds the asset row lock (serializes vs disposal), then claim the period (posted
//! gate) and advance the asset. Each period posts at most once; the last period flips the asset to
//! `fully_depreciated`. Idempotent at the period grain (`depr:{entry_id}`).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `AssetRepository` / `AssetCategoryRepository` / `AssetDepreciationEntryRepository`.

use rust_decimal::Decimal;
use uuid::Uuid;

use super::asset_events::{AssetEvent, AssetEventSink, DepreciationPosted};
use super::asset_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};

use super::asset_write_service::{
    legacy_company_echo, money, relay_ambient_scope, AssetError, AssetWriteService,
    DepreciationRunOutcome,
};

impl AssetWriteService {
    /// Post every schedule period due on or before `up_to`: `Dr Depreciation Expense · Cr Accum Dep`.
    /// Each period is posted at most once (post first, then the `posted` gate), advancing the asset's
    /// accumulated depreciation / net book value; the last period flips it to `fully_depreciated`.
    ///
    /// The module is tenant-agnostic (ADR-0029): lookups are id-only. On a composed deployment the
    /// caller's ambient org scope fences these reads — an out-of-scope asset is indistinguishable
    /// from a missing one (`NotFound`), so this does not leak whether the id exists. On a composed
    /// deployment an ambient org scope is re-bound onto the per-period transactions below.
    pub async fn run_depreciation(
        &self,
        asset_id: Uuid,
        up_to: chrono::DateTime<chrono::Utc>,
        gl: &dyn GlPostSink,
        sink: &dyn AssetEventSink,
    ) -> Result<DepreciationRunOutcome, AssetError> {
        let a = self.load_asset(asset_id).await?;
        if a.status == "disposed" {
            return Err(AssetError::InvalidState("asset is disposed"));
        }
        let cat = self.load_category(a.category_id).await?;
        let depreciable = a.gross - a.salvage;

        // The due read rides its own short transaction: the ambient org scope is re-bound onto it,
        // so on a composed deployment the fence sees the caller; with none bound it is plain.
        let mut due_tx = self.pool.begin().await?;
        relay_ambient_scope(&mut due_tx).await?;
        let entries = self.schedule.list_due(&mut due_tx, asset_id, up_to).await?;
        due_tx.commit().await?;

        let mut posted = 0i32;
        let mut total = Decimal::ZERO;
        let mut fully = false;
        for e in &entries {
            let entry_id: Uuid = e.id;
            let period_no: i32 = e.period_no;
            let amount: Decimal = e.depreciation_amount;
            let acc_after: Decimal = e.accumulated_after;
            let sched: chrono::DateTime<chrono::Utc> = e.schedule_date;

            let is_last = acc_after >= depreciable;
            let env = AccountingPostEnvelope {
                idempotency_key: format!("depr:{entry_id}"),
                // The legacy tenancy twin echo (ADR-0029): nil when no ambient scope is bound.
                company_id: legacy_company_echo(),
                branch_id: None,
                source_type: "asset".into(),
                source_id: Uuid::new_v5(&entry_id, b"asset:depreciate"),
                source_reference: Some(a.asset_code.clone()),
                posting_date: sched.date_naive(),
                currency: "IDR".into(),
                posting_type: "original".into(),
                description: Some(format!("depreciation period {period_no}")),
                lines: vec![
                    GlPostLine::debit(cat.dep_expense, amount).with_description("Depreciation expense"),
                    GlPostLine::credit(cat.accum_dep, amount).with_description("Accumulated depreciation"),
                ],
            };
            if !env.is_balanced() {
                return Err(AssetError::Invalid("unbalanced posting".into()));
            }

            // One transaction per period, holding the asset row lock across the post: (a) lock +
            // recheck the asset isn't disposed — serializes vs `dispose_asset`, so a period can never
            // credit Accum Dep after disposal; (b) claim the period (posted gate) — idempotent; (c) post
            // under the lock; (d) advance the asset. On any error the tx rolls back, leaving the period
            // unposted for a clean retry (council 2026-07-06).
            let mut tx = self.pool.begin().await?;
            // Re-bind the caller's ambient org scope onto this transaction, so on a composed
            // deployment the row lock, the posted gate, and the asset advance pass the fence.
            relay_ambient_scope(&mut tx).await?;
            let st: String = self.assets.lock_status(&mut tx, asset_id).await?;
            if st == "disposed" {
                tx.rollback().await?;
                break;
            }
            let g = self.schedule.claim_period(&mut tx, entry_id).await?;
            if g != 1 {
                tx.rollback().await?;
                continue; // raced/retried — skip
            }
            if let Err(e2) = gl.post(&env).await {
                tx.rollback().await?;
                return Err(AssetError::Gl(e2.code));
            }
            self.assets.advance_depreciation(&mut tx, asset_id, amount, is_last).await?;
            tx.commit().await?;

            posted += 1;
            total += amount;
            if is_last {
                fully = true;
            }
            sink.publish(&AssetEvent::DepreciationPosted(DepreciationPosted {
                asset_id,
                entry_id,
                company_id: legacy_company_echo(),
                period_no,
                amount,
                accumulated_after: acc_after,
                fully_depreciated: is_last,
            }));
        }
        Ok(DepreciationRunOutcome { periods_posted: posted, total_posted: money(total), fully_depreciated: fully })
    }
}

/// Summary of a scheduled (cross-tenant) depreciation sweep — what the background job reports.
#[derive(Debug, Clone, PartialEq)]
pub struct DueDepreciationSummary {
    /// Distinct assets that had ≥1 period posted this sweep.
    pub assets_depreciated: usize,
    pub periods_posted: i32,
    pub fully_depreciated: usize,
}

impl AssetWriteService {
    /// Run depreciation for every asset with a period due on or before `up_to`.
    ///
    /// For the scheduled job — there is no caller principal, so this enumerates via
    /// `AssetDepreciationEntryRepository::list_due_assets` (a SECURITY DEFINER function; the module's
    /// sweep is whole-deployment by design) and then runs the idempotent per-asset writes via
    /// [`Self::run_depreciation`]. Safe to run repeatedly: each period posts at most once
    /// (`depr:{entry}` idempotency key).
    pub async fn run_due_depreciation(
        &self,
        up_to: chrono::DateTime<chrono::Utc>,
        gl: &dyn GlPostSink,
        sink: &dyn AssetEventSink,
    ) -> Result<DueDepreciationSummary, AssetError> {
        let due = self.schedule.list_due_assets(&self.pool, up_to).await?;
        let mut assets = 0usize;
        let mut periods = 0i32;
        let mut fully = 0usize;
        for asset_id in due {
            let o = self.run_depreciation(asset_id, up_to, gl, sink).await?;
            if o.periods_posted > 0 {
                assets += 1;
            }
            periods += o.periods_posted;
            if o.fully_depreciated {
                fully += 1;
            }
        }
        Ok(DueDepreciationSummary {
            assets_depreciated: assets,
            periods_posted: periods,
            fully_depreciated: fully,
        })
    }
}
