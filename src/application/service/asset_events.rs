//! Asset domain events (hand-authored, user-owned) — the public extension surface.
//!
//! The lifecycle publishes these as the asset is capitalized, depreciated, and retired. A consumer
//! (a fixed-asset register report, the tax-depreciation overlay) subscribes without calling back.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An asset was activated (capitalized): `Dr Fixed Asset · Cr Funding`, and its straight-line
/// schedule was generated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetActivated {
    pub asset_id: Uuid,
    pub company_id: Uuid,
    pub gross_purchase_amount: Decimal,
    pub periods: i32,
}

/// A depreciation period posted: `Dr Depreciation Expense · Cr Accumulated Depreciation`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DepreciationPosted {
    pub asset_id: Uuid,
    pub entry_id: Uuid,
    pub company_id: Uuid,
    pub period_no: i32,
    pub amount: Decimal,
    pub accumulated_after: Decimal,
    pub fully_depreciated: bool,
}

/// An asset was disposed — removed from the books, with the gain/loss recognised.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssetDisposed {
    pub asset_id: Uuid,
    pub company_id: Uuid,
    pub proceeds: Decimal,
    pub net_book_value: Decimal,
    /// proceeds − net_book_value (positive = gain, negative = loss).
    pub gain_loss: Decimal,
}

/// The asset domain-event union.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AssetEvent {
    AssetActivated(AssetActivated),
    DepreciationPosted(DepreciationPosted),
    AssetDisposed(AssetDisposed),
}

/// Sink the write path publishes to. A consuming service supplies its own (bus, outbox, …).
pub trait AssetEventSink: Send + Sync {
    fn publish(&self, event: &AssetEvent);
}

/// A no-op/logging sink for tests and single-process composition.
#[derive(Debug, Default, Clone)]
pub struct LoggingSink;

impl AssetEventSink for LoggingSink {
    fn publish(&self, event: &AssetEvent) {
        tracing::info!(?event, "asset event");
    }
}
