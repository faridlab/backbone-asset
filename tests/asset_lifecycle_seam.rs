//! The asset-lifecycle seam, end-to-end: **backbone-asset → the REAL backbone-accounting ledger**.
//! An asset is capitalized (Dr Fixed Asset · Cr Funding), depreciated period-by-period (Dr Depreciation
//! Expense · Cr Accumulated Depreciation), then disposed (Dr Accum Dep + Dr Proceeds ± gain/loss · Cr
//! Fixed Asset). Two invariants hold against the real ledger:
//!   (1) Σ depreciation posts == the depreciable base (gross − salvage);
//!   (2) on disposal the asset's Fixed-Asset AND Accumulated-Depreciation accounts net back to ZERO —
//!       it is removed from the books, gain/loss the plug.
//! Assets owns no ledger — it emits through `GlPostSink`; ZERO normal Cargo edge to accounting.

mod common;

use backbone_asset::application::service::asset_events::LoggingSink;
use backbone_asset::application::service::asset_write_service::{
    AssetWriteService, NewAsset, NewAssetCategory,
};
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

async fn make(pool: &sqlx::PgPool, svc: &AssetWriteService, company: Uuid, acc: &AssetAccounts, gross: &str, salvage: &str, life: i32) -> Uuid {
    let cat = svc.create_category(NewAssetCategory {
        company_id: company, category_name: "Machinery".into(), useful_life_months: life,
        fixed_asset_account_id: acc.fixed_asset, accumulated_depreciation_account_id: acc.accum_dep,
        depreciation_expense_account_id: acc.dep_expense, disposal_gain_loss_account_id: acc.gain_loss,
    }).await.unwrap();
    let _ = pool;
    svc.create_asset(NewAsset {
        company_id: company, asset_category_id: cat, asset_name: "M".into(),
        asset_code: format!("A-{}", &Uuid::new_v4().to_string()[..8]),
        item_id: None, branch_id: None, gross_purchase_amount: dec(gross), salvage_value: dec(salvage),
        opening_accumulated_depreciation: dec("0"),
        useful_life_months: life, purchase_date: now(), available_for_use_date: None,
    }).await.unwrap()
}

fn far() -> chrono::DateTime<chrono::Utc> {
    now() + chrono::Duration::days(3650)
}

/// ALSEAM-1 — full life then disposal at a gain: Σ depreciation == depreciable, and the asset nets off
/// the books (Fixed Asset AND Accumulated Depreciation both back to zero).
#[tokio::test]
async fn alseam1_full_life_then_disposal_nets_off_books() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = GlAdapter::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let asset = make(&pool, &svc, company, &acc, "12000", "0", 12).await;

    // Capitalize: Dr Fixed Asset 12,000 · Cr Funding 12,000.
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();
    assert_eq!(balance(&pool, acc.fixed_asset).await, dec("12000.00"));
    assert_eq!(balance(&pool, acc.funding).await, dec("-12000.00"));

    // Depreciate the whole life: Σ = depreciable 12,000.
    let r = svc.run_depreciation(asset, far(), &gl, &sink).await.unwrap();
    assert_eq!(r.periods_posted, 12);
    assert_eq!(r.total_posted, dec("12000.00"), "Σ depreciation == depreciable base");
    assert!(r.fully_depreciated);
    assert_eq!(balance(&pool, acc.dep_expense).await, dec("12000.00"));
    assert_eq!(balance(&pool, acc.accum_dep).await, dec("-12000.00"));

    // Dispose at a gain (NBV = 0, proceeds 3,000 → gain 3,000).
    let d = svc.dispose_asset(asset, dec("3000"), acc.proceeds, today(), &gl, &sink).await.unwrap();
    assert_eq!(d.net_book_value, dec("0"));
    assert_eq!(d.gain_loss, dec("3000"));

    // The asset is REMOVED from the books: both its accounts net to zero.
    assert_eq!(balance(&pool, acc.fixed_asset).await, dec("0.00"), "Fixed Asset removed");
    assert_eq!(balance(&pool, acc.accum_dep).await, dec("0.00"), "Accumulated Depreciation removed");
    assert_eq!(balance(&pool, acc.gain_loss).await, dec("-3000.00"), "gain recognised (credit)");
    assert_eq!(balance(&pool, acc.proceeds).await, dec("3000.00"), "proceeds banked");

    let status: String = sqlx::query_scalar("SELECT status::text FROM asset.assets WHERE id=$1")
        .bind(asset).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "disposed");
}

/// ALSEAM-2 — early disposal at a LOSS: after partial depreciation the asset still nets off the books,
/// with the loss recognised as the plug.
#[tokio::test]
async fn alseam2_early_disposal_at_a_loss() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = GlAdapter::new(pool.clone());
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let asset = make(&pool, &svc, company, &acc, "1200", "0", 12).await;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();

    // ~3 months → 3 × 100 = 300 depreciated; NBV = 900.
    let r = svc.run_depreciation(asset, now() + chrono::Duration::days(95), &gl, &sink).await.unwrap();
    assert_eq!(r.periods_posted, 3);
    assert_eq!(r.total_posted, dec("300.00"));

    // Dispose for 500 → loss = 500 − 900 = −400.
    let d = svc.dispose_asset(asset, dec("500"), acc.proceeds, today(), &gl, &sink).await.unwrap();
    assert_eq!(d.net_book_value, dec("900"));
    assert_eq!(d.gain_loss, dec("-400"));

    assert_eq!(balance(&pool, acc.fixed_asset).await, dec("0.00"), "Fixed Asset removed");
    assert_eq!(balance(&pool, acc.accum_dep).await, dec("0.00"), "Accumulated Depreciation removed");
    assert_eq!(balance(&pool, acc.gain_loss).await, dec("400.00"), "loss recognised (debit)");
    assert_eq!(balance(&pool, acc.proceeds).await, dec("500.00"));
}
