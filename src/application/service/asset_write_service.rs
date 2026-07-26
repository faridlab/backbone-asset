//! The hand-authored asset write path — HUB (user-owned; survives regen).
//!
//! Book-basis fixed-asset lifecycle, all emitted through `GlPostSink` (assets owns no ledger):
//!   activate    Dr Fixed Asset · Cr Funding            (direct-buy capitalization) + generate schedule
//!   depreciate  Dr Depreciation Expense · Cr Accum Dep (each due straight-line period)
//!   dispose     Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset
//! so an asset acquired → fully depreciated → disposed nets its Fixed-Asset + Accumulated-Depreciation
//! accounts back to ZERO (removed from the books), and Σ depreciation posts == the depreciable base
//! (gross − salvage). Every verb does its idempotent GL post FIRST, then commits a status/posted gate
//! (the manufacturing lesson), so a retry never double-posts. Money is IDR, 2dp, half-away-from-zero.
//!
//! **This file is the hub:** it holds the module's vocabulary (input structs, outcomes, errors, internal
//! helpers) and the asset/category registration path. The lifecycle verbs are chunked into focused
//! siblings, each an `impl AssetWriteService` block over these same types:
//!
//! - [`super::asset_activate`] — capitalize + generate schedule + flip draft→active (`activate_asset`).
//! - [`super::asset_depreciate`] — post every due schedule period (`run_depreciation`).
//! - [`super::asset_dispose`] — remove from the books and recognise gain/loss (`dispose_asset`).

use backbone_orm::company_scope;
use rust_decimal::{Decimal, RoundingStrategy};
use sqlx::PgPool;
use uuid::Uuid;

use crate::infrastructure::persistence::{
    AssetCategoryRepository, AssetDepreciationEntryRepository, AssetRepository,
    NewAssetCategoryRow, NewAssetRow,
};

use super::asset_gl::{AccountingPostEnvelope, GlPostSink};

pub(super) fn money(v: Decimal) -> Decimal {
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
    pub(super) pool: PgPool,
    pub(super) assets: AssetRepository,
    pub(super) categories: AssetCategoryRepository,
    pub(super) schedule: AssetDepreciationEntryRepository,
}

pub(super) struct Cat {
    pub(super) method: String,
    pub(super) useful_life_months: i32,
    pub(super) fixed_asset: Uuid,
    pub(super) accum_dep: Uuid,
    pub(super) dep_expense: Uuid,
    pub(super) gain_loss: Uuid,
}

pub(super) struct AssetRow {
    pub(super) company_id: Uuid,
    pub(super) category_id: Uuid,
    pub(super) asset_code: String,
    pub(super) gross: Decimal,
    pub(super) salvage: Decimal,
    pub(super) useful_life_months: i32,
    pub(super) purchase_date: chrono::DateTime<chrono::Utc>,
    pub(super) available: Option<chrono::DateTime<chrono::Utc>>,
    pub(super) accumulated: Decimal,
    pub(super) opening: Decimal,
    pub(super) status: String,
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
        )
        .await?;
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
        )
        .await;
        if let Err(e) = r {
            return Err(if is_dup(&e) { AssetError::DuplicateNumber(a.asset_code) } else { e.into() });
        }
        Ok(id)
    }

    // ---- shared helpers (pub(super) — used by sibling impl blocks) ----------------------------

    pub(super) async fn post(&self, gl: &dyn GlPostSink, env: &AccountingPostEnvelope) -> Result<(), AssetError> {
        if !env.is_balanced() {
            return Err(AssetError::Invalid("unbalanced posting".into()));
        }
        gl.post(env).await.map_err(|r| AssetError::Gl(r.code))?;
        Ok(())
    }

    pub(super) async fn load_category(&self, company_id: Uuid, id: Uuid) -> Result<Cat, AssetError> {
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

    pub(super) async fn load_asset(&self, id: Uuid) -> Result<AssetRow, AssetError> {
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

pub(super) fn is_dup(e: &sqlx::Error) -> bool {
    matches!(e, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505"))
}
