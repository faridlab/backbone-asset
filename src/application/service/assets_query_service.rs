//! Concrete [`AssetsQueryService`] over the repositories (hand-authored, user-owned).
//!
//! Delivers the published read contract (defined in `exports::services`) that the
//! skeleton shipped as an unimplemented trait. It is placed in the application layer —
//! not inside `exports/` — so `exports/` stays a pure, decoupled contract surface while
//! the realization lives next to the other services that depend on infrastructure.
//!
//! Reads use the repositories' generic `find_by_id` / `exists`. Under RLS
//! (`app.company_id`), a read with no company scope set simply sees no rows; a composing
//! service binds the caller's company onto its connection as usual.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

use crate::exports::{
    AssetCategoryDto, AssetCategoryId, AssetCategorySummary, AssetDepreciationEntryDto,
    AssetDepreciationEntryId, AssetDepreciationEntrySummary, AssetDto, AssetId, AssetSummary,
    AssetsQueryService,
};
use crate::infrastructure::persistence::{
    AssetCategoryRepository, AssetDepreciationEntryRepository, AssetRepository,
};

/// Implemented [`AssetsQueryService`] — one `PgPool`, builds a repo per call (the pool is
/// `Arc`-internal, so cloning is cheap).
pub struct AssetsQueryServiceImpl {
    pool: PgPool,
}

impl AssetsQueryServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AssetsQueryService for AssetsQueryServiceImpl {
    async fn get_asset_category(&self, id: AssetCategoryId) -> Result<Option<AssetCategoryDto>> {
        let c = AssetCategoryRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(c.map(AssetCategoryDto::from))
    }

    async fn get_asset_category_summary(
        &self,
        id: AssetCategoryId,
    ) -> Result<Option<AssetCategorySummary>> {
        let c = AssetCategoryRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(c.map(|c| AssetCategorySummary {
            id: AssetCategoryId(c.id),
            category_name: c.category_name,
            status: c.status,
        }))
    }

    async fn asset_category_exists(&self, id: AssetCategoryId) -> Result<bool> {
        AssetCategoryRepository::new(self.pool.clone())
            .exists(&id.into_inner().to_string())
            .await
    }

    async fn get_asset(&self, id: AssetId) -> Result<Option<AssetDto>> {
        let a = AssetRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(a.map(AssetDto::from))
    }

    async fn get_asset_summary(&self, id: AssetId) -> Result<Option<AssetSummary>> {
        let a = AssetRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(a.map(|a| AssetSummary {
            id: AssetId(a.id),
            asset_name: a.asset_name,
            status: a.status,
        }))
    }

    async fn asset_exists(&self, id: AssetId) -> Result<bool> {
        AssetRepository::new(self.pool.clone())
            .exists(&id.into_inner().to_string())
            .await
    }

    async fn get_asset_depreciation_entry(
        &self,
        id: AssetDepreciationEntryId,
    ) -> Result<Option<AssetDepreciationEntryDto>> {
        let e = AssetDepreciationEntryRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(e.map(AssetDepreciationEntryDto::from))
    }

    async fn get_asset_depreciation_entry_summary(
        &self,
        id: AssetDepreciationEntryId,
    ) -> Result<Option<AssetDepreciationEntrySummary>> {
        let e = AssetDepreciationEntryRepository::new(self.pool.clone())
            .find_by_id(&id.into_inner().to_string())
            .await?;
        Ok(e.map(|e| AssetDepreciationEntrySummary {
            id: AssetDepreciationEntryId(e.id),
        }))
    }

    async fn asset_depreciation_entry_exists(&self, id: AssetDepreciationEntryId) -> Result<bool> {
        AssetDepreciationEntryRepository::new(self.pool.clone())
            .exists(&id.into_inner().to_string())
            .await
    }
}

// Entity → DTO conversions. Regen-safe home: this file is hand-authored (never overwritten by
// `metaphor make`), unlike `exports/types.rs` whose CUSTOM block the generator resets. The query
// methods above rely on these `From` impls to hand siblings the published DTO shapes.
impl From<crate::domain::entity::Asset> for AssetDto {
    fn from(a: crate::domain::entity::Asset) -> Self {
        Self {
            id: AssetId(a.id),
            company_id: a.company_id,
            asset_category_id: a.asset_category_id,
            asset_name: a.asset_name,
            asset_code: a.asset_code,
            item_id: a.item_id,
            branch_id: a.branch_id,
            gross_purchase_amount: a.gross_purchase_amount,
            salvage_value: a.salvage_value,
            useful_life_months: a.useful_life_months,
            opening_accumulated_depreciation: a.opening_accumulated_depreciation,
            purchase_date: a.purchase_date,
            available_for_use_date: a.available_for_use_date,
            accumulated_depreciation: a.accumulated_depreciation,
            net_book_value: a.net_book_value,
            status: a.status,
            metadata: serde_json::to_value(&a.metadata).unwrap_or_default(),
        }
    }
}

impl From<crate::domain::entity::AssetCategory> for AssetCategoryDto {
    fn from(c: crate::domain::entity::AssetCategory) -> Self {
        Self {
            id: AssetCategoryId(c.id),
            company_id: c.company_id,
            category_name: c.category_name,
            depreciation_method: c.depreciation_method,
            useful_life_months: c.useful_life_months,
            fixed_asset_account_id: c.fixed_asset_account_id,
            accumulated_depreciation_account_id: c.accumulated_depreciation_account_id,
            depreciation_expense_account_id: c.depreciation_expense_account_id,
            disposal_gain_loss_account_id: c.disposal_gain_loss_account_id,
            status: c.status,
            metadata: serde_json::to_value(&c.metadata).unwrap_or_default(),
        }
    }
}

impl From<crate::domain::entity::AssetDepreciationEntry> for AssetDepreciationEntryDto {
    fn from(e: crate::domain::entity::AssetDepreciationEntry) -> Self {
        Self {
            id: AssetDepreciationEntryId(e.id),
            company_id: e.company_id,
            asset_id: e.asset_id,
            period_no: e.period_no,
            schedule_date: e.schedule_date,
            depreciation_amount: e.depreciation_amount,
            accumulated_after: e.accumulated_after,
            posted: e.posted,
            posted_at: e.posted_at,
            metadata: serde_json::to_value(&e.metadata).unwrap_or_default(),
        }
    }
}
