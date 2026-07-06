//! Shared test helpers: a live pool, a real-accounting GL adapter, account seeding + ledger balances,
//! and a counting GL sink. Fresh random ids per test so rows never collide across parallel runs.

#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use backbone_accounting::application::service::posting_service::{
    PostingLine, PostingRequest, PostingService,
};
use backbone_asset::application::service::asset_gl::{
    AccountingPostEnvelope, GlPostAck, GlPostRejected, GlPostSink,
};
use rust_decimal::Decimal;
use sqlx::PgPool;
use uuid::Uuid;

pub fn dburl() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5433/backbone_asset".into())
}
pub async fn pool() -> PgPool {
    PgPool::connect(&dburl()).await.expect("connect")
}
pub fn dec(s: &str) -> Decimal {
    s.parse().unwrap()
}
pub fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::Utc::now()
}
pub fn today() -> chrono::NaiveDate {
    chrono::Utc::now().date_naive()
}

pub async fn account(pool: &PgPool, company: Uuid, code: &str, atype: &str, subtype: &str, normal: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO accounting.accounts
             (id, company_id, account_number, account_code, name, account_type, account_subtype,
              normal_balance, is_header, is_detail, status)
           VALUES ($1,$2,$3,$4,$5,$6::account_type,$7::account_subtype,$8::normal_balance,
                   false,true,'active'::account_status)"#,
    )
    .bind(id).bind(company).bind(code).bind(code).bind(code).bind(atype).bind(subtype).bind(normal)
    .execute(pool).await.expect("seed account");
    id
}

pub async fn balance(pool: &PgPool, account: Uuid) -> Decimal {
    sqlx::query_scalar(
        "SELECT COALESCE(SUM(debit_amount),0) - COALESCE(SUM(credit_amount),0)
         FROM accounting.ledgers WHERE account_id=$1",
    )
    .bind(account)
    .fetch_one(pool)
    .await
    .expect("balance")
}

/// The GL accounts an asset category needs, plus a funding + proceeds account for the lifecycle.
pub struct AssetAccounts {
    pub fixed_asset: Uuid,
    pub accum_dep: Uuid,
    pub dep_expense: Uuid,
    pub gain_loss: Uuid,
    pub funding: Uuid,
    pub proceeds: Uuid,
}
pub async fn asset_accounts(pool: &PgPool, company: Uuid) -> AssetAccounts {
    AssetAccounts {
        fixed_asset: account(pool, company, "1500-FA", "asset", "fixed_asset", "debit").await,
        accum_dep: account(pool, company, "1590-AD", "asset", "accumulated_depreciation", "credit").await,
        dep_expense: account(pool, company, "6000-DE", "expense", "operating_expense", "debit").await,
        gain_loss: account(pool, company, "7000-GL", "other_income", "operating_revenue", "credit").await,
        // Cash-funded acquisition (a bank account needs no party; an AP funding would require the
        // supplier party, which the MVP activate() does not carry — see the completeness parking lot).
        funding: account(pool, company, "1000-BANK", "asset", "bank", "debit").await,
        proceeds: account(pool, company, "1100-CASH", "asset", "cash", "debit").await,
    }
}

/// ACL: asset's serialized envelope → accounting's PostingRequest against the REAL ledger.
pub struct GlAdapter {
    pub svc: PostingService,
}
impl GlAdapter {
    pub fn new(pool: PgPool) -> Self {
        Self { svc: PostingService::new(pool) }
    }
}
#[async_trait::async_trait]
impl GlPostSink for GlAdapter {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        let mut r = PostingRequest::original(e.company_id, &e.source_type, e.source_id, e.posting_date);
        r.source_reference = e.source_reference.clone();
        r.posting_type = e.posting_type.clone();
        r.lines = e.lines.iter().map(|l| PostingLine {
            account_id: l.account_id, debit: l.debit, credit: l.credit,
            party_type: l.party_type.clone(), party_id: l.party_id,
            cost_center_id: None, project_id: None, department_id: None, description: l.description.clone(),
        }).collect();
        match self.svc.post(r, None).await {
            Ok(x) => Ok(GlPostAck { post_id: x.post_id, journal_id: x.journal_id, idempotent_reuse: x.idempotent_reuse }),
            Err(x) => Err(GlPostRejected { code: x.code().to_string(), message: x.to_string() }),
        }
    }
}

/// A counting GL sink — records each post's idempotency_key so tests can assert how many posts of a
/// given kind (acquire/depr/dispose) reached the ledger.
#[derive(Clone, Default)]
pub struct CountingGl {
    pub keys: Arc<Mutex<Vec<String>>>,
}
impl CountingGl {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn count(&self, kind: &str) -> usize {
        self.keys.lock().unwrap().iter().filter(|k| k.starts_with(kind)).count()
    }
}
#[async_trait::async_trait]
impl GlPostSink for CountingGl {
    async fn post(&self, e: &AccountingPostEnvelope) -> Result<GlPostAck, GlPostRejected> {
        self.keys.lock().unwrap().push(e.idempotency_key.clone());
        Ok(GlPostAck { post_id: Uuid::new_v4(), journal_id: Uuid::new_v4(), idempotent_reuse: false })
    }
}
