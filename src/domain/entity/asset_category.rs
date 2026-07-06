use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::DepreciationMethod;
use super::AuditMetadata;

/// Strongly-typed ID for AssetCategory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetCategoryId(pub Uuid);

impl AssetCategoryId {
    pub fn new(id: Uuid) -> Self { Self(id) }
    pub fn generate() -> Self { Self(Uuid::new_v4()) }
    pub fn into_inner(self) -> Uuid { self.0 }
}

impl std::fmt::Display for AssetCategoryId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AssetCategoryId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(Uuid::parse_str(s)?))
    }
}

impl From<Uuid> for AssetCategoryId {
    fn from(id: Uuid) -> Self { Self(id) }
}

impl From<AssetCategoryId> for Uuid {
    fn from(id: AssetCategoryId) -> Self { id.0 }
}

impl AsRef<Uuid> for AssetCategoryId {
    fn as_ref(&self) -> &Uuid { &self.0 }
}

impl std::ops::Deref for AssetCategoryId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AssetCategory {
    pub id: Uuid,
    pub company_id: Uuid,
    pub category_name: String,
    pub depreciation_method: DepreciationMethod,
    pub useful_life_months: i32,
    pub fixed_asset_account_id: Uuid,
    pub accumulated_depreciation_account_id: Uuid,
    pub depreciation_expense_account_id: Uuid,
    pub disposal_gain_loss_account_id: Uuid,
    pub is_active: bool,
    #[serde(default)]
    #[sqlx(json)]
    pub metadata: AuditMetadata,
}

impl AssetCategory {
    /// Create a builder for AssetCategory
    pub fn builder() -> AssetCategoryBuilder {
        AssetCategoryBuilder::default()
    }

    /// Create a new AssetCategory with required fields
    pub fn new(company_id: Uuid, category_name: String, depreciation_method: DepreciationMethod, useful_life_months: i32, fixed_asset_account_id: Uuid, accumulated_depreciation_account_id: Uuid, depreciation_expense_account_id: Uuid, disposal_gain_loss_account_id: Uuid, is_active: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            company_id,
            category_name,
            depreciation_method,
            useful_life_months,
            fixed_asset_account_id,
            accumulated_depreciation_account_id,
            depreciation_expense_account_id,
            disposal_gain_loss_account_id,
            is_active,
            metadata: AuditMetadata::default(),
        }
    }

    /// Get the entity's unique identifier
    pub fn id(&self) -> &Uuid {
        &self.id
    }

    /// Get a strongly-typed ID for this entity
    pub fn typed_id(&self) -> AssetCategoryId {
        AssetCategoryId(self.id)
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
    // Partial Update
    // ==========================================================

    /// Apply partial updates from a map of field name to JSON value
    pub fn apply_patch(&mut self, fields: std::collections::HashMap<String, serde_json::Value>) {
        for (key, value) in fields {
            match key.as_str() {
                "company_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.company_id = v; }
                }
                "category_name" => {
                    if let Ok(v) = serde_json::from_value(value) { self.category_name = v; }
                }
                "depreciation_method" => {
                    if let Ok(v) = serde_json::from_value(value) { self.depreciation_method = v; }
                }
                "useful_life_months" => {
                    if let Ok(v) = serde_json::from_value(value) { self.useful_life_months = v; }
                }
                "fixed_asset_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.fixed_asset_account_id = v; }
                }
                "accumulated_depreciation_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.accumulated_depreciation_account_id = v; }
                }
                "depreciation_expense_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.depreciation_expense_account_id = v; }
                }
                "disposal_gain_loss_account_id" => {
                    if let Ok(v) = serde_json::from_value(value) { self.disposal_gain_loss_account_id = v; }
                }
                "is_active" => {
                    if let Ok(v) = serde_json::from_value(value) { self.is_active = v; }
                }
                _ => {} // ignore unknown fields
            }
        }
    }

    // <<< CUSTOM METHODS START >>>
    // <<< CUSTOM METHODS END >>>
}

impl super::Entity for AssetCategory {
    type Id = Uuid;

    fn entity_id(&self) -> &Self::Id {
        &self.id
    }

    fn entity_type() -> &'static str {
        "AssetCategory"
    }
}

impl backbone_core::PersistentEntity for AssetCategory {
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

impl backbone_orm::EntityRepoMeta for AssetCategory {
    fn column_types() -> std::collections::HashMap<String, String> {
        let mut m = std::collections::HashMap::new();
        m.insert("id".to_string(), "uuid".to_string());
        m.insert("company_id".to_string(), "uuid".to_string());
        m.insert("fixed_asset_account_id".to_string(), "uuid".to_string());
        m.insert("accumulated_depreciation_account_id".to_string(), "uuid".to_string());
        m.insert("depreciation_expense_account_id".to_string(), "uuid".to_string());
        m.insert("disposal_gain_loss_account_id".to_string(), "uuid".to_string());
        m.insert("depreciation_method".to_string(), "depreciation_method".to_string());
        m
    }
    fn search_fields() -> &'static [&'static str] {
        &["category_name"]
    }
}

/// Builder for AssetCategory entity
///
/// Provides a fluent API for constructing AssetCategory instances.
/// System fields (id, metadata, timestamps) are auto-initialized.
#[derive(Debug, Clone, Default)]
pub struct AssetCategoryBuilder {
    company_id: Option<Uuid>,
    category_name: Option<String>,
    depreciation_method: Option<DepreciationMethod>,
    useful_life_months: Option<i32>,
    fixed_asset_account_id: Option<Uuid>,
    accumulated_depreciation_account_id: Option<Uuid>,
    depreciation_expense_account_id: Option<Uuid>,
    disposal_gain_loss_account_id: Option<Uuid>,
    is_active: Option<bool>,
}

impl AssetCategoryBuilder {
    /// Set the company_id field (required)
    pub fn company_id(mut self, value: Uuid) -> Self {
        self.company_id = Some(value);
        self
    }

    /// Set the category_name field (required)
    pub fn category_name(mut self, value: String) -> Self {
        self.category_name = Some(value);
        self
    }

    /// Set the depreciation_method field (default: `DepreciationMethod::default()`)
    pub fn depreciation_method(mut self, value: DepreciationMethod) -> Self {
        self.depreciation_method = Some(value);
        self
    }

    /// Set the useful_life_months field (required)
    pub fn useful_life_months(mut self, value: i32) -> Self {
        self.useful_life_months = Some(value);
        self
    }

    /// Set the fixed_asset_account_id field (required)
    pub fn fixed_asset_account_id(mut self, value: Uuid) -> Self {
        self.fixed_asset_account_id = Some(value);
        self
    }

    /// Set the accumulated_depreciation_account_id field (required)
    pub fn accumulated_depreciation_account_id(mut self, value: Uuid) -> Self {
        self.accumulated_depreciation_account_id = Some(value);
        self
    }

    /// Set the depreciation_expense_account_id field (required)
    pub fn depreciation_expense_account_id(mut self, value: Uuid) -> Self {
        self.depreciation_expense_account_id = Some(value);
        self
    }

    /// Set the disposal_gain_loss_account_id field (required)
    pub fn disposal_gain_loss_account_id(mut self, value: Uuid) -> Self {
        self.disposal_gain_loss_account_id = Some(value);
        self
    }

    /// Set the is_active field (default: `true`)
    pub fn is_active(mut self, value: bool) -> Self {
        self.is_active = Some(value);
        self
    }

    /// Build the AssetCategory entity
    ///
    /// Returns Err if any required field without a default is missing.
    pub fn build(self) -> Result<AssetCategory, String> {
        let company_id = self.company_id.ok_or_else(|| "company_id is required".to_string())?;
        let category_name = self.category_name.ok_or_else(|| "category_name is required".to_string())?;
        let useful_life_months = self.useful_life_months.ok_or_else(|| "useful_life_months is required".to_string())?;
        let fixed_asset_account_id = self.fixed_asset_account_id.ok_or_else(|| "fixed_asset_account_id is required".to_string())?;
        let accumulated_depreciation_account_id = self.accumulated_depreciation_account_id.ok_or_else(|| "accumulated_depreciation_account_id is required".to_string())?;
        let depreciation_expense_account_id = self.depreciation_expense_account_id.ok_or_else(|| "depreciation_expense_account_id is required".to_string())?;
        let disposal_gain_loss_account_id = self.disposal_gain_loss_account_id.ok_or_else(|| "disposal_gain_loss_account_id is required".to_string())?;

        Ok(AssetCategory {
            id: Uuid::new_v4(),
            company_id,
            category_name,
            depreciation_method: self.depreciation_method.unwrap_or(DepreciationMethod::default()),
            useful_life_months,
            fixed_asset_account_id,
            accumulated_depreciation_account_id,
            depreciation_expense_account_id,
            disposal_gain_loss_account_id,
            is_active: self.is_active.unwrap_or(true),
            metadata: AuditMetadata::default(),
        })
    }
}
