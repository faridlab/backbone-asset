//! The hand-authored asset write path (user-owned; survives regen).
//!
//! Book-basis fixed-asset lifecycle, all emitted through `GlPostSink` (assets owns no ledger):
//!   activate    Dr Fixed Asset · Cr Funding            (direct-buy capitalization) + generate schedule
//!   depreciate  Dr Depreciation Expense · Cr Accum Dep (each due straight-line period)
//!   dispose     Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset
//! so an asset acquired → fully depreciated → disposed nets its Fixed-Asset + Accumulated-Depreciation
//! accounts back to ZERO (removed from the books), and Σ depreciation posts == the depreciable base
//! (gross − salvage). Every verb does its idempotent GL post FIRST, then commits a status/posted gate
//! (the manufacturing lesson), so a retry never double-posts. Money is IDR, 2dp, half-away-from-zero.

use backbone_orm::company_scope;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    AssetCategoryRepository, AssetDepreciationEntryRepository, AssetRepository,
    NewAssetCategoryRow, NewAssetRow, NewDepreciationEntryRow,
};

use super::asset_events::*;
use super::asset_gl::{AccountingPostEnvelope, GlPostLine, GlPostSink};

fn money(v: Decimal) -> Decimal {
    v.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("db: {0}")]
    Db(#[from] sqlx::Error),
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("invalid state: {0}")]
    InvalidState(&'static str),
    #[error("unsupported depreciation method (only straight_line is wired)")]
    UnsupportedMethod,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("gl rejected: {0}")]
    Gl(String),
    #[error("duplicate asset code: {0}")]
    DuplicateNumber(String),
}

pub struct NewAssetCategory {
    pub company_id: Uuid,
    pub category_name: String,
    pub useful_life_months: i32,
    pub fixed_asset_account_id: Uuid,
    pub accumulated_depreciation_account_id: Uuid,
    pub depreciation_expense_account_id: Uuid,
    pub disposal_gain_loss_account_id: Uuid,
}

pub struct NewAsset {
    pub company_id: Uuid,
    pub asset_category_id: Uuid,
    pub asset_name: String,
    pub asset_code: String,
    pub item_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
    pub gross_purchase_amount: Decimal,
    pub salvage_value: Decimal,
    /// Depreciation already booked on legacy books — set > 0 to ONBOARD an EXISTING (part-depreciated)
    /// asset. Its gross + accumulated are assumed already on the opening trial balance, so activation
    /// posts NO capitalization and schedules only the remaining life. 0 = a brand-new asset.
    pub opening_accumulated_depreciation: Decimal,
    /// 0 = inherit the category's useful life.
    pub useful_life_months: i32,
    pub purchase_date: chrono::DateTime<chrono::Utc>,
    pub available_for_use_date: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DepreciationRunOutcome {
    pub periods_posted: i32,
    pub total_posted: Decimal,
    pub fully_depreciated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DisposalOutcome {
    pub net_book_value: Decimal,
    pub gain_loss: Decimal,
    pub already: bool,
}

pub struct AssetWriteService {
    pool: PgPool,
    assets: AssetRepository,
    categories: AssetCategoryRepository,
    schedule: AssetDepreciationEntryRepository,
}

struct Cat {
    method: String,
    useful_life_months: i32,
    fixed_asset: Uuid,
    accum_dep: Uuid,
    dep_expense: Uuid,
    gain_loss: Uuid,
}

struct AssetRow {
    company_id: Uuid,
    category_id: Uuid,
    asset_code: String,
    gross: Decimal,
    salvage: Decimal,
    useful_life_months: i32,
    purchase_date: chrono::DateTime<chrono::Utc>,
    available: Option<chrono::DateTime<chrono::Utc>>,
    accumulated: Decimal,
    opening: Decimal,
    status: String,
}

impl AssetWriteService {
    pub fn new(pool: PgPool) -> Self {
        let assets = AssetRepository::new(pool.clone());
        let categories = AssetCategoryRepository::new(pool.clone());
        let schedule = AssetDepreciationEntryRepository::new(pool.clone());
        Self { pool, assets, categories, schedule }
    }

    pub async fn create_category(&self, c: NewAssetCategory) -> Result<Uuid, AssetError> {
        if c.useful_life_months <= 0 {
            return Err(AssetError::Invalid("useful_life_months must be positive".into()));
        }
        let id = Uuid::new_v4();
        // RLS scope (ADR-0008): company is on the DTO — scope the insert on it so it passes the
        // WITH CHECK fence. The explicit `company_id` bind stays as defense-in-depth.
        let row = NewAssetCategoryRow {
            id,
            company_id: c.company_id,
            category_name: &c.category_name,
            useful_life_months: c.useful_life_months,
            fixed_asset_account_id: c.fixed_asset_account_id,
            accumulated_depreciation_account_id: c.accumulated_depreciation_account_id,
            depreciation_expense_account_id: c.depreciation_expense_account_id,
            disposal_gain_loss_account_id: c.disposal_gain_loss_account_id,
        };
        company_scope::with_company_scope(
            Some(c.company_id),
            self.categories.insert_category(&self.pool, &row),
        ).await?;
        Ok(id)
    }

    /// Register an asset (draft). Its useful life is snapshotted from the category unless overridden.
    pub async fn create_asset(&self, a: NewAsset) -> Result<Uuid, AssetError> {
        if a.gross_purchase_amount <= Decimal::ZERO {
            return Err(AssetError::Invalid("gross_purchase_amount must be positive".into()));
        }
        if a.salvage_value < Decimal::ZERO || a.salvage_value >= a.gross_purchase_amount {
            return Err(AssetError::Invalid("salvage_value must be in [0, gross)".into()));
        }
        // RLS scope (ADR-0008): company is on the DTO — scope the category lookup on it.
        let cat_life: i32 = company_scope::with_company_scope(
            Some(a.company_id),
            self.categories.find_useful_life(&self.pool, a.asset_category_id, a.company_id),
        )
        .await?
        .ok_or(AssetError::NotFound("asset category"))?;
        let life = if a.useful_life_months > 0 { a.useful_life_months } else { cat_life };
        let depreciable = a.gross_purchase_amount - a.salvage_value;
        // An onboarded existing asset can already be partly (not fully) depreciated.
        if a.opening_accumulated_depreciation < Decimal::ZERO || a.opening_accumulated_depreciation >= depreciable {
            return Err(AssetError::Invalid("opening_accumulated_depreciation must be in [0, depreciable)".into()));
        }
        // Every REMAINING period must depreciate at least one cent, else the residue-absorbing last row
        // can go negative and the schedule can't tie out (council 2026-07-06, steelman).
        if (depreciable - a.opening_accumulated_depreciation) < Decimal::from(life) * Decimal::new(1, 2) {
            return Err(AssetError::Invalid("depreciable base too small for the useful life (< 1 cent/period)".into()));
        }
        let opening = a.opening_accumulated_depreciation;

        let id = Uuid::new_v4();
        let row = NewAssetRow {
            id,
            company_id: a.company_id,
            asset_category_id: a.asset_category_id,
            asset_name: &a.asset_name,
            asset_code: &a.asset_code,
            item_id: a.item_id,
            branch_id: a.branch_id,
            gross_purchase_amount: a.gross_purchase_amount,
            salvage_value: a.salvage_value,
            opening_accumulated_depreciation: opening,
            useful_life_months: life,
            purchase_date: a.purchase_date,
            available_for_use_date: a.available_for_use_date,
        };
        let r = company_scope::with_company_scope(
            Some(a.company_id),
            self.assets.insert_asset(&self.pool, &row),
        ).await;
        if let Err(e) = r {
            return Err(if is_dup(&e) { AssetError::DuplicateNumber(a.asset_code) } else { e.into() });
        }
        Ok(id)
    }

    /// Capitalize + schedule: post `Dr Fixed Asset · Cr Funding`, generate the straight-line schedule,
    /// draft → active. Idempotent (the acquisition post + the draft→active gate).
    pub async fn activate_asset(
        &self,
        asset_id: Uuid,
        funding_account_id: Uuid,
        at: chrono::NaiveDate,
        gl: &dyn GlPostSink,
        sink: &dyn AssetEventSink,
    ) -> Result<(), AssetError> {
        let a = self.load_asset(asset_id).await?;
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

    /// Post every schedule period due on or before `up_to`: `Dr Depreciation Expense · Cr Accum Dep`.
    /// Each period is posted at most once (post first, then the `posted` gate), advancing the asset's
    /// accumulated depreciation / net book value; the last period flips it to `fully_depreciated`.
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
        let cat = self.load_category(a.company_id, a.category_id).await?;
        let depreciable = a.gross - a.salvage;

        // RLS scope (ADR-0008): the asset's company was just read off its row — scope the schedule read
        // on it, so this is correct for non-request callers (the depreciation job) too.
        let entries = company_scope::with_company_scope(
            Some(a.company_id),
            self.schedule.list_due(&self.pool, asset_id, up_to),
        )
        .await?;

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
                company_id: a.company_id,
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
            // Bind the asset's own company (read off its row above) onto this transaction, so the
            // row lock, the posted gate, and the asset advance all pass the RLS fence.
            company_scope::bind_company_on(&mut tx, a.company_id).await?;
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
                company_id: a.company_id,
                period_no,
                amount,
                accumulated_after: acc_after,
                fully_depreciated: is_last,
            }));
        }
        Ok(DepreciationRunOutcome { periods_posted: posted, total_posted: money(total), fully_depreciated: fully })
    }

    /// Dispose the asset: remove it from the books and recognise gain/loss.
    /// `Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset`. Idempotent (post + status gate).
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
        // RLS scope (ADR-0008): this method carries NO company — it is identified by the asset id
        // alone, and the company is only known from the row read UNDER the lock inside this very
        // transaction, so it cannot be bound explicitly up front. Bind the ambient scope instead:
        // under HTTP that is the caller's company. A non-request CALLER (event subscriber / job) MUST
        // wrap this call in `with_company_scope(Some(event.company_id))`, or the locked read below
        // fails closed and the asset reads as not-found.
        let mut tx = self.pool.begin().await?;
        company_scope::bind_current_company(&mut tx).await?;
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
        let company_id: Uuid = row.company_id;
        let category_id: Uuid = row.asset_category_id;
        let asset_code: String = row.asset_code;
        let cat = self.load_category(company_id, category_id).await?;
        let gain_loss = proceeds - nbv; // + gain, − loss

        // Build the balanced disposal envelope from the locked-in accumulated.
        let mut lines = vec![
            GlPostLine::debit(cat.accum_dep, accumulated).with_description("Accumulated depreciation"),
            GlPostLine::credit(cat.fixed_asset, gross).with_description("Fixed asset"),
        ];
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
            company_id,
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
            company_id,
            proceeds,
            net_book_value: nbv,
            gain_loss,
        }));
        Ok(DisposalOutcome { net_book_value: nbv, gain_loss, already: false })
    }

    // ---- helpers ----------------------------------------------------------------------------

    async fn post(&self, gl: &dyn GlPostSink, env: &AccountingPostEnvelope) -> Result<(), AssetError> {
        if !env.is_balanced() {
            return Err(AssetError::Invalid("unbalanced posting".into()));
        }
        gl.post(env).await.map_err(|r| AssetError::Gl(r.code))?;
        Ok(())
    }

    async fn load_category(&self, company_id: Uuid, id: Uuid) -> Result<Cat, AssetError> {
        // RLS scope (ADR-0008): the company is a parameter (read off the asset row by the caller) —
        // scope the lookup on it, so this is correct for non-request callers (jobs) too.
        let r = company_scope::with_company_scope(
            Some(company_id),
            self.categories.find_accounts(&self.pool, id, company_id),
        )
        .await?
        .ok_or(AssetError::NotFound("asset category"))?;
        Ok(Cat {
            method: r.method,
            useful_life_months: r.useful_life_months,
            fixed_asset: r.fixed_asset_account_id,
            accum_dep: r.accumulated_depreciation_account_id,
            dep_expense: r.depreciation_expense_account_id,
            gain_loss: r.disposal_gain_loss_account_id,
        })
    }

    async fn load_asset(&self, id: Uuid) -> Result<AssetRow, AssetError> {
        // RLS scope (ADR-0008), ID-only pattern: identified by the asset id alone — no company to scope
        // from up front, so this read rides the caller's scope (the request-dedicated connection under
        // HTTP, or an event caller's `with_company_scope`). RLS fences it: another company's asset is
        // simply not found. Callers read `company_id` off the returned row to bind their own tx.
        let r = self
            .assets
            .find_snapshot(&self.pool, id)
            .await?
            .ok_or(AssetError::NotFound("asset"))?;
        Ok(AssetRow {
            company_id: r.company_id,
            category_id: r.asset_category_id,
            asset_code: r.asset_code,
            gross: r.gross_purchase_amount,
            salvage: r.salvage_value,
            useful_life_months: r.useful_life_months,
            purchase_date: r.purchase_date,
            available: r.available_for_use_date,
            accumulated: r.accumulated_depreciation,
            opening: r.opening_accumulated_depreciation,
            status: r.status,
        })
    }
}

fn is_dup(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}
