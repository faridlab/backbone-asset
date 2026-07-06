use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "asset_status", rename_all = "snake_case")]
pub enum AssetStatus {
    Draft,
    Active,
    FullyDepreciated,
    Disposed,
}

impl std::fmt::Display for AssetStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Active => write!(f, "active"),
            Self::FullyDepreciated => write!(f, "fully_depreciated"),
            Self::Disposed => write!(f, "disposed"),
        }
    }
}

impl FromStr for AssetStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "active" => Ok(Self::Active),
            "fully_depreciated" => Ok(Self::FullyDepreciated),
            "disposed" => Ok(Self::Disposed),
            _ => Err(format!("Unknown AssetStatus variant: {}", s)),
        }
    }
}

impl Default for AssetStatus {
    fn default() -> Self {
        Self::Draft
    }
}
