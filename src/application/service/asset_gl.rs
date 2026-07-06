//! Outbound GL-posting port (hand-authored, user-owned) — assets side of the GL-posting contract
//! (`docs/erp/gl-posting-contract.md`).
//!
//! Assets is a pure emitter of balanced postings, all `source_type = "asset"`, distinguished by
//! `posting_type='original'` + a distinct `source_id` per voucher:
//!   acquire     Dr Fixed Asset · Cr Funding            (direct-buy capitalization)   source_id = asset
//!   depreciate  Dr Depreciation Expense · Cr Accum Dep (one schedule period)         source_id = entry
//!   dispose     Dr Accum Dep + Dr Proceeds ± gain/loss · Cr Fixed Asset               source_id = asset:disposal
//! so an asset acquired then fully depreciated then disposed nets its Fixed-Asset and Accumulated-
//! Depreciation accounts back to ZERO — it is removed from the books. Reached only through a
//! `GlPostSink`; the ACL adapter maps the envelope into accounting's `PostingRequest`. ZERO normal
//! Cargo edge to accounting — the envelope is the wire contract.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One debit/credit line of an emitted posting. Exactly one of `debit`/`credit` is > 0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlPostLine {
    pub account_id: Uuid,
    pub debit: Decimal,
    pub credit: Decimal,
    pub party_type: Option<String>,
    pub party_id: Option<Uuid>,
    pub description: Option<String>,
}

impl GlPostLine {
    pub fn debit(account_id: Uuid, amount: Decimal) -> Self {
        Self { account_id, debit: amount, credit: Decimal::ZERO, party_type: None, party_id: None, description: None }
    }
    pub fn credit(account_id: Uuid, amount: Decimal) -> Self {
        Self { account_id, debit: Decimal::ZERO, credit: amount, party_type: None, party_id: None, description: None }
    }
    pub fn with_description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
}

/// A balanced posting request emitted by assets — the contract envelope.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountingPostEnvelope {
    pub idempotency_key: String,
    pub company_id: Uuid,
    pub branch_id: Option<Uuid>,
    /// Posting source discriminator — assets emits "asset".
    pub source_type: String,
    /// The producer voucher id (asset for acquire/dispose, schedule entry for depreciate).
    pub source_id: Uuid,
    pub source_reference: Option<String>,
    pub posting_date: chrono::NaiveDate,
    pub currency: String,
    /// "original" (assets does not reverse in the MVP).
    pub posting_type: String,
    pub description: Option<String>,
    pub lines: Vec<GlPostLine>,
}

impl AccountingPostEnvelope {
    pub fn totals(&self) -> (Decimal, Decimal) {
        (
            self.lines.iter().map(|l| l.debit).sum(),
            self.lines.iter().map(|l| l.credit).sum(),
        )
    }
    pub fn is_balanced(&self) -> bool {
        let (d, c) = self.totals();
        d == c && !self.lines.is_empty()
    }
}

/// Acknowledgement returned by the GL after a successful post.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlPostAck {
    pub post_id: Uuid,
    pub journal_id: Uuid,
    pub idempotent_reuse: bool,
}

/// Rejection returned by the GL (validation failure). `code` is the stable contract error string.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlPostRejected {
    pub code: String,
    pub message: String,
}

/// The GL-posting seam. A composing service implements this over accounting's `PostingService`.
#[async_trait::async_trait]
pub trait GlPostSink: Send + Sync {
    async fn post(&self, envelope: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected>;
}
