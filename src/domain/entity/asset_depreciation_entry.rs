use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;
use super::AuditMetadata;

/// Strongly-typed ID for AssetDepreciationEntry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetDepreciationEntryId(pub Uuid);

impl AssetDepreciationEntryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AssetDepreciationEntryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AssetDepreciationEntryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AssetDepreciationEntryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AssetDepreciationEntryId> for Uuid {
    fn from(id: AssetDepreciationEntryId) -> Self { id.0 }
}

impl AsRef<Uuid> for AssetDepreciationEntryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AssetDepreciationEntryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AssetDepreciationEntry {
    pub id: Uuid,
    pub company_id: Uuid,
    pub asset_id: Uuid,
    pub period_no: i32,
    pub schedule_date: DateTime<Utc>,
    pub depreciation_amount: Decimal,
    pub accumulated_after: Decimal,
    pub posted: bool,
    pub posted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl AssetDepreciationEntry {
    /// Create a builder for AssetDepreciationEntry
    pub fn builder() -> AssetDepreciationEntryBuilder {
        AssetDepreciationEntryBuilder::default()
    }

    /// Create a new AssetDepreciationEntry with required fields
    pub fn new(company_id: Uuid, asset_id: Uuid, period_no: i32, schedule_date: DateTime<Utc>, depreciation_amount: Decimal, accumulated_after: Decimal, posted: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            asset_id,
            period_no,
            schedule_date,
            depreciation_amount,
            accumulated_after,
            posted,
            posted_at: None,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AssetDepreciationEntryId {
        AssetDepreciationEntryId(self.id)
    }

    /// Get when this entity was created
    pub fn created_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.created_at.as_ref()
    }

    /// Get when this entity was last updated
    pub fn updated_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.updated_at.as_ref()
    }

    /// Check if this entity is soft deleted
    pub fn is_deleted(&self) -> bool {
        self.metadata.deleted_at.is_some()
    }

    /// Check if this entity is active (not deleted)
    pub fn is_active(&self) -> bool {
        self.metadata.deleted_at.is_none()
    }

    /// Get when this entity was deleted
    pub fn deleted_at(&self) -> Option<&DateTime<Utc>> {
        self.metadata.deleted_at.as_ref()
    }

    /// Get who created this entity
    pub fn created_by(&self) -> Option<&Uuid> {
        self.metadata.created_by.as_ref()
    }

    /// Get who last updated this entity
    pub fn updated_by(&self) -> Option<&Uuid> {
        self.metadata.updated_by.as_ref()
    }

    /// Get who deleted this entity
    pub fn deleted_by(&self) -> Option<&Uuid> {
        self.metadata.deleted_by.as_ref()
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the posted_at field (chainable)
    pub fn with_posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    // ==========================================================
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "asset_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.asset_id = v; }
                }
                "period_no" => {
                    if let Ok(v) = serde_json::from_value(value) { self.period_no = v; }
                }
                "schedule_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.schedule_date = v; }
                }
                "depreciation_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.depreciation_amount = v; }
                }
                "accumulated_after" => {
                    if let Ok(v) = serde_json::from_value(value) { self.accumulated_after = v; }
                }
                "posted" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted = v; }
                }
                "posted_at" => {
                    if let Ok(v) = serde_json::from_value(value) { self.posted_at = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for AssetDepreciationEntry {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "AssetDepreciationEntry"
    }
}

impl backbone_core::PersistentEntity for AssetDepreciationEntry {
    fn entity_id(&self) -> String {
        self.id.to_string()
    }
    fn set_entity_id(&mut self, id: String) {
        if let Ok(uuid) = uuid::Uuid::parse_str(&id) {
            self.id = uuid;
        }
    }
    fn created_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.created_at
    }
    fn set_created_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.created_at = Some(ts);
    }
    fn updated_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.updated_at
    }
    fn set_updated_at(&mut self, ts: chrono::DateTime<chrono::Utc>) {
        self.metadata.updated_at = Some(ts);
    }
    fn deleted_at(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.metadata.deleted_at
    }
    fn set_deleted_at(&mut self, ts: Option<chrono::DateTime<chrono::Utc>>) {
        self.metadata.deleted_at = ts;
    }
}

impl backbone_orm::EntityRepoMeta for AssetDepreciationEntry {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("asset_id".to_string(), "uuid".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &[]
    }
}

/// Builder for AssetDepreciationEntry entity
///
/// Provides a fluent API for constructing AssetDepreciationEntry instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AssetDepreciationEntryBuilder {
    company_id: Option<Uuid>,
    asset_id: Option<Uuid>,
    period_no: Option<i32>,
    schedule_date: Option<DateTime<Utc>>,
    depreciation_amount: Option<Decimal>,
    accumulated_after: Option<Decimal>,
    posted: Option<bool>,
    posted_at: Option<DateTime<Utc>>,
}

impl AssetDepreciationEntryBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the asset_id field (required)
    pub fn asset_id(mut self, value: Uuid) -> Self {
        self.asset_id = Some(value);
        self
    }

    /// Set the period_no field (required)
    pub fn period_no(mut self, value: i32) -> Self {
        self.period_no = Some(value);
        self
    }

    /// Set the schedule_date field (required)
    pub fn schedule_date(mut self, value: DateTime<Utc>) -> Self {
        self.schedule_date = Some(value);
        self
    }

    /// Set the depreciation_amount field (required)
    pub fn depreciation_amount(mut self, value: Decimal) -> Self {
        self.depreciation_amount = Some(value);
        self
    }

    /// Set the accumulated_after field (required)
    pub fn accumulated_after(mut self, value: Decimal) -> Self {
        self.accumulated_after = Some(value);
        self
    }

    /// Set the posted field (default: `false`)
    pub fn posted(mut self, value: bool) -> Self {
        self.posted = Some(value);
        self
    }

    /// Set the posted_at field (optional)
    pub fn posted_at(mut self, value: DateTime<Utc>) -> Self {
        self.posted_at = Some(value);
        self
    }

    /// Build the AssetDepreciationEntry entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<AssetDepreciationEntry, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let asset_id = self.asset_id.ok_or_else(|| "asset_id is required".to_string())?;
        let period_no = self.period_no.ok_or_else(|| "period_no is required".to_string())?;
        let schedule_date = self.schedule_date.ok_or_else(|| "schedule_date is required".to_string())?;
        let depreciation_amount = self.depreciation_amount.ok_or_else(|| "depreciation_amount is required".to_string())?;
        let accumulated_after = self.accumulated_after.ok_or_else(|| "accumulated_after is required".to_string())?;

        Ok(AssetDepreciationEntry {
            id: Uuid::new_v4(),
            company_id,
            asset_id,
            period_no,
            schedule_date,
            depreciation_amount,
            accumulated_after,
            posted: self.posted.unwrap_or(false),
            posted_at: self.posted_at,
            metadata: AuditMetadata::default(),
        })
    }
}
