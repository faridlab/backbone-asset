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
