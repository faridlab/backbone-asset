use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use rust_decimal::Decimal;

use super::AssetStatus;
use super::AuditMetadata;

/// Strongly-typed ID for Asset
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub Uuid);

impl AssetId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AssetId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AssetId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AssetId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AssetId> for Uuid {
    fn from(id: AssetId) -> Self { id.0 }
}

impl AsRef<Uuid> for AssetId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AssetId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Asset {
    pub id: Uuid,
    pub company_id: Uuid,
    pub asset_category_id: Uuid,
    pub asset_name: String,
    pub asset_code: String,
    pub item_id: Option<Uuid>,
    pub branch_id: Option<Uuid>,
    pub gross_purchase_amount: Decimal,
    pub salvage_value: Decimal,
    pub useful_life_months: i32,
    pub opening_accumulated_depreciation: Decimal,
    pub purchase_date: DateTime<Utc>,
    pub available_for_use_date: Option<DateTime<Utc>>,
    pub accumulated_depreciation: Decimal,
    pub net_book_value: Decimal,
    pub status: AssetStatus,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl Asset {
    /// Create a builder for Asset
    pub fn builder() -> AssetBuilder {
        AssetBuilder::default()
    }

    /// Create a new Asset with required fields
    pub fn new(company_id: Uuid, asset_category_id: Uuid, asset_name: String, asset_code: String, gross_purchase_amount: Decimal, salvage_value: Decimal, useful_life_months: i32, opening_accumulated_depreciation: Decimal, purchase_date: DateTime<Utc>, accumulated_depreciation: Decimal, net_book_value: Decimal, status: AssetStatus) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            asset_category_id,
            asset_name,
            asset_code,
            item_id: None,
            branch_id: None,
            gross_purchase_amount,
            salvage_value,
            useful_life_months,
            opening_accumulated_depreciation,
            purchase_date,
            available_for_use_date: None,
            accumulated_depreciation,
            net_book_value,
            status,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AssetId {
        AssetId(self.id)
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

    /// Get the current status
    pub fn status(&self) -> &AssetStatus {
        &self.status
    }


    // ==========================================================
    // Fluent Setters (with_* for optional fields)
    // ==========================================================

    /// Set the item_id field (chainable)
    pub fn with_item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the branch_id field (chainable)
    pub fn with_branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the available_for_use_date field (chainable)
    pub fn with_available_for_use_date(mut self, value: DateTime<Utc>) -> Self {
        self.available_for_use_date = Some(value);
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
                "asset_category_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.asset_category_id = v; }
                }
                "asset_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.asset_name = v; }
                }
                "asset_code" => {
                    if let Ok(v) = serde_json::from_value(value) { self.asset_code = v; }
                }
                "item_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.item_id = v; }
                }
                "branch_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.branch_id = v; }
                }
                "gross_purchase_amount" => {
                    if let Ok(v) = serde_json::from_value(value) { self.gross_purchase_amount = v; }
                }
                "salvage_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.salvage_value = v; }
                }
                "useful_life_months" => {
                    if let Ok(v) = serde_json::from_value(value) { self.useful_life_months = v; }
                }
                "opening_accumulated_depreciation" => {
                    if let Ok(v) = serde_json::from_value(value) { self.opening_accumulated_depreciation = v; }
                }
                "purchase_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.purchase_date = v; }
                }
                "available_for_use_date" => {
                    if let Ok(v) = serde_json::from_value(value) { self.available_for_use_date = v; }
                }
                "accumulated_depreciation" => {
                    if let Ok(v) = serde_json::from_value(value) { self.accumulated_depreciation = v; }
                }
                "net_book_value" => {
                    if let Ok(v) = serde_json::from_value(value) { self.net_book_value = v; }
                }
                "status" => {
                    if let Ok(v) = serde_json::from_value(value) { self.status = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for Asset {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "Asset"
    }
}

impl backbone_core::PersistentEntity for Asset {
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

impl backbone_orm::EntityRepoMeta for Asset {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("asset_category_id".to_string(), "uuid".to_string());
        m.insert("item_id".to_string(), "uuid".to_string());
        m.insert("branch_id".to_string(), "uuid".to_string());
        m.insert("status".to_string(), "asset_status".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["asset_name", "asset_code"]
    }
}

/// Builder for Asset entity
///
/// Provides a fluent API for constructing Asset instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AssetBuilder {
    company_id: Option<Uuid>,
    asset_category_id: Option<Uuid>,
    asset_name: Option<String>,
    asset_code: Option<String>,
    item_id: Option<Uuid>,
    branch_id: Option<Uuid>,
    gross_purchase_amount: Option<Decimal>,
    salvage_value: Option<Decimal>,
    useful_life_months: Option<i32>,
    opening_accumulated_depreciation: Option<Decimal>,
    purchase_date: Option<DateTime<Utc>>,
    available_for_use_date: Option<DateTime<Utc>>,
    accumulated_depreciation: Option<Decimal>,
    net_book_value: Option<Decimal>,
    status: Option<AssetStatus>,
}

impl AssetBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the asset_category_id field (required)
    pub fn asset_category_id(mut self, value: Uuid) -> Self {
        self.asset_category_id = Some(value);
        self
    }

    /// Set the asset_name field (required)
    pub fn asset_name(mut self, value: String) -> Self {
        self.asset_name = Some(value);
        self
    }

    /// Set the asset_code field (required)
    pub fn asset_code(mut self, value: String) -> Self {
        self.asset_code = Some(value);
        self
    }

    /// Set the item_id field (optional)
    pub fn item_id(mut self, value: Uuid) -> Self {
        self.item_id = Some(value);
        self
    }

    /// Set the branch_id field (optional)
    pub fn branch_id(mut self, value: Uuid) -> Self {
        self.branch_id = Some(value);
        self
    }

    /// Set the gross_purchase_amount field (required)
    pub fn gross_purchase_amount(mut self, value: Decimal) -> Self {
        self.gross_purchase_amount = Some(value);
        self
    }

    /// Set the salvage_value field (default: `Decimal::from(0)`)
    pub fn salvage_value(mut self, value: Decimal) -> Self {
        self.salvage_value = Some(value);
        self
    }

    /// Set the useful_life_months field (required)
    pub fn useful_life_months(mut self, value: i32) -> Self {
        self.useful_life_months = Some(value);
        self
    }

    /// Set the opening_accumulated_depreciation field (default: `Decimal::from(0)`)
    pub fn opening_accumulated_depreciation(mut self, value: Decimal) -> Self {
        self.opening_accumulated_depreciation = Some(value);
        self
    }

    /// Set the purchase_date field (required)
    pub fn purchase_date(mut self, value: DateTime<Utc>) -> Self {
        self.purchase_date = Some(value);
        self
    }

    /// Set the available_for_use_date field (optional)
    pub fn available_for_use_date(mut self, value: DateTime<Utc>) -> Self {
        self.available_for_use_date = Some(value);
        self
    }

    /// Set the accumulated_depreciation field (default: `Decimal::from(0)`)
    pub fn accumulated_depreciation(mut self, value: Decimal) -> Self {
        self.accumulated_depreciation = Some(value);
        self
    }

    /// Set the net_book_value field (default: `Decimal::from(0)`)
    pub fn net_book_value(mut self, value: Decimal) -> Self {
        self.net_book_value = Some(value);
        self
    }

    /// Set the status field (default: `AssetStatus::default()`)
    pub fn status(mut self, value: AssetStatus) -> Self {
        self.status = Some(value);
        self
    }

    /// Build the Asset entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<Asset, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let asset_category_id = self.asset_category_id.ok_or_else(|| "asset_category_id is required".to_string())?;
        let asset_name = self.asset_name.ok_or_else(|| "asset_name is required".to_string())?;
        let asset_code = self.asset_code.ok_or_else(|| "asset_code is required".to_string())?;
        let gross_purchase_amount = self.gross_purchase_amount.ok_or_else(|| "gross_purchase_amount is required".to_string())?;
        let useful_life_months = self.useful_life_months.ok_or_else(|| "useful_life_months is required".to_string())?;
        let purchase_date = self.purchase_date.ok_or_else(|| "purchase_date is required".to_string())?;

        Ok(Asset {
            id: Uuid::new_v4(),
            company_id,
            asset_category_id,
            asset_name,
            asset_code,
            item_id: self.item_id,
            branch_id: self.branch_id,
            gross_purchase_amount,
            salvage_value: self.salvage_value.unwrap_or(Decimal::from(0)),
            useful_life_months,
            opening_accumulated_depreciation: self.opening_accumulated_depreciation.unwrap_or(Decimal::from(0)),
            purchase_date,
            available_for_use_date: self.available_for_use_date,
            accumulated_depreciation: self.accumulated_depreciation.unwrap_or(Decimal::from(0)),
            net_book_value: self.net_book_value.unwrap_or(Decimal::from(0)),
            status: self.status.unwrap_or(AssetStatus::default()),
            metadata: AuditMetadata::default(),
        })
    }
}
