use serde::{Deserialize, Serialize};
use sqlx::Type;
use std::str::FromStr;
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "depreciation_method", rename_all = "snake_case")]
pub enum DepreciationMethod {
    StraightLine,
    DecliningBalance,
    WrittenDownValue,
}

impl std::fmt::Display for DepreciationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StraightLine => write!(f, "straight_line"),
            Self::DecliningBalance => write!(f, "declining_balance"),
            Self::WrittenDownValue => write!(f, "written_down_value"),
        }
    }
}

impl FromStr for DepreciationMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "straight_line" => Ok(Self::StraightLine),
            "declining_balance" => Ok(Self::DecliningBalance),
            "written_down_value" => Ok(Self::WrittenDownValue),
            _ => Err(format!("Unknown DepreciationMethod variant: {}", s)),
        }
    }
}

impl Default for DepreciationMethod {
    fn default() -> Self {
        Self::StraightLine
    }
}
