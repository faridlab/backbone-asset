//! Capitalize + schedule + flip draft→active (hand-authored, user-owned).
//!
//! An `impl AssetWriteService` chunk over the vocabulary in [`super::asset_write_service`]: post
//! `Dr Fixed Asset · Cr Funding` (skipped for an onboarded part-depreciated asset whose gross +
//! accumulated are already on the opening trial balance), build the straight-line schedule for the
//! REMAINING life only (periods absorbed by `opening_accumulated_depreciation` are dropped), and gate
//! draft→active. Idempotent (the acquisition post dedupes by `acquire:{asset_id}`, and the draft→active
//! gate is a compare-and-set on status).
//!
//! Per the module's 4-layer rule this file holds no SQL — the statements live on
//! `AssetRepository` / `AssetCategoryRepository` / `AssetDepreciationEntryRepository`, whose custom
//! methods take this service's transaction so the status flip + schedule inserts commit as one unit.

use backbone_orm::company_scope;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::infrastructure::persistence::NewDepreciationEntryRow;

use super::asset_events::{AssetActivated, AssetEvent, AssetEventSink};
use super::asset_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};

use super::asset_write_service::{money, AssetError, AssetWriteService};

impl AssetWriteService {
    /// Capitalize + schedule: post `Dr Fixed Asset · Cr Funding`, generate the straight-line schedule,
    /// draft → active. Idempotent (the acquisition post + the draft→active gate).
    pub async fn activate_asset(
        &self,
        asset_id: Uuid,
        company_id: Uuid,
        funding_account_id: Uuid,
        at: chrono::NaiveDate,
        gl: &dyn GlPostSink,
        sink: &dyn AssetEventSink,
    ) -> Result<(), AssetError> {
        let a = self.load_asset(company_id, asset_id).await?;
        if a.status != "draft" {
            return Ok(()); // already activated — idempotent no-op (the acquisition post was made once)
        }
        let cat = self.load_category(a.company_id, a.category_id).await?;
        if cat.method != "straight_line" {
            return Err(AssetError::UnsupportedMethod);
        }

        // 1) Acquisition post — ONLY for a brand-new asset. An onboarded existing asset (opening > 0)
        //    already sits on the opening trial balance (its gross + accumulated), so re-capitalizing it
        //    would double-count assets/equity — skip the post (council 2026-07-06).
        if a.opening == Decimal::ZERO {
            let env = AccountingPostEnvelope {
                idempotency_key: format!("acquire:{asset_id}"),
                company_id: a.company_id,
                branch_id: None,
                source_type: "asset".into(),
                source_id: Uuid::new_v5(&asset_id, b"asset:acquire"),
                source_reference: Some(a.asset_code.clone()),
                posting_date: at,
                currency: "IDR".into(),
                posting_type: "original".into(),
                description: Some("asset capitalization".into()),
                lines: vec![
                    GlPostLine::debit(cat.fixed_asset, a.gross).with_description("Fixed asset"),
                    GlPostLine::credit(funding_account_id, a.gross).with_description("Funding"),
                ],
            };
            self.post(gl, &env).await?;
        }

        // 2) Build the straight-line schedule for the REMAINING life only. Periods already covered by
        //    `opening` are dropped; the first remaining period is trimmed to the part above `opening`.
        //    `accumulated_after` is the FULL cumulative (ends at `depreciable`), and schedule_date keeps
        //    the ORIGINAL period offset, so a mid-life asset's remaining periods carry correct dates.
        let available = a.available.unwrap_or(a.purchase_date);
        let n = a.useful_life_months;
        let depreciable = a.gross - a.salvage;
        let per = money(depreciable / Decimal::from(n));
        let mut rows: Vec<(i32, chrono::DateTime<chrono::Utc>, Decimal, Decimal)> = Vec::new();
        let mut acc = Decimal::ZERO;
        let mut out_period = 0i32;
        for p in 1..=n {
            let full_amount = if p == n { depreciable - per * Decimal::from(n - 1) } else { per };
            let prev_acc = acc;
            acc += full_amount;
            if acc <= a.opening {
                continue; // this period was already depreciated on the legacy books
            }
            let amount = if prev_acc < a.opening { acc - a.opening } else { full_amount };
            out_period += 1;
            let date = available + chrono::Months::new(p as u32);
            rows.push((out_period, date, amount, acc));
        }

        // 3) Gate draft→active + insert the schedule.
        // RLS scope (ADR-0008): the asset's company was read off its row above — bind it onto this
        // transaction so the status flip and the schedule inserts pass the fence.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_company_on(&mut tx, a.company_id).await?;
        let moved = self.assets.claim_activation(&mut tx, asset_id, available).await?;
        if moved != 1 {
            tx.rollback().await?;
            return Ok(()); // already activated (the acquisition post deduped)
        }
        for (p, date, amount, acc_after) in &rows {
            self.schedule.insert_entry(&mut tx, &NewDepreciationEntryRow {
                id: Uuid::new_v4(),
                company_id: a.company_id,
                asset_id,
                period_no: *p,
                schedule_date: *date,
                depreciation_amount: *amount,
                accumulated_after: *acc_after,
            }).await?;
        }
        tx.commit().await?;
        sink.publish(&AssetEvent::AssetActivated(AssetActivated {
            asset_id,
            company_id: a.company_id,
            gross_purchase_amount: a.gross,
            periods: n,
        }));
        Ok(())
    }
}
