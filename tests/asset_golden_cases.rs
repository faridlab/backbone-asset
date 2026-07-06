//! Golden cases — the numeric oracle for the straight-line schedule + validation. Money is IDR (2dp,
//! half-away-from-zero). Mirrors docs/business-flows/golden-cases.md.

mod common;

use backbone_asset::application::service::asset_events::LoggingSink;
use backbone_asset::application::service::asset_write_service::{
    AssetError, AssetWriteService, NewAsset, NewAssetCategory,
};
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

async fn category(svc: &AssetWriteService, company: Uuid, a: &AssetAccounts, life: i32) -> Uuid {
    svc.create_category(NewAssetCategory {
        company_id: company,
        category_name: "Machinery".into(),
        useful_life_months: life,
        fixed_asset_account_id: a.fixed_asset,
        accumulated_depreciation_account_id: a.accum_dep,
        depreciation_expense_account_id: a.dep_expense,
        disposal_gain_loss_account_id: a.gain_loss,
    })
    .await
    .unwrap()
}

async fn asset(svc: &AssetWriteService, company: Uuid, cat: Uuid, gross: &str, salvage: &str, life: i32) -> Uuid {
    svc.create_asset(NewAsset {
        company_id: company,
        asset_category_id: cat,
        asset_name: "M1".into(),
        asset_code: format!("A-{}", &Uuid::new_v4().to_string()[..8]),
        item_id: None,
        branch_id: None,
        gross_purchase_amount: dec(gross),
        salvage_value: dec(salvage),
        opening_accumulated_depreciation: dec("0"),
        useful_life_months: life,
        purchase_date: now(),
        available_for_use_date: None,
    })
    .await
    .unwrap()
}

async fn schedule(pool: &sqlx::PgPool, asset_id: Uuid) -> Vec<(i32, Decimal, Decimal)> {
    sqlx::query_as::<_, (i32, Decimal, Decimal)>(
        "SELECT period_no, depreciation_amount, accumulated_after FROM asset.asset_depreciation_entries
         WHERE asset_id=$1 ORDER BY period_no",
    )
    .bind(asset_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// AGC-1 — a divisible straight-line schedule: 12,000 over 12 months = 1,000 each; Σ = depreciable.
#[tokio::test]
async fn agc1_divisible_schedule() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = CountingGl::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = category(&svc, company, &acc, 12).await;
    let a = asset(&svc, company, cat, "12000", "0", 0).await;
    svc.activate_asset(a, acc.funding, today(), &gl, &sink).await.unwrap();

    let s = schedule(&pool, a).await;
    assert_eq!(s.len(), 12);
    assert!(s.iter().all(|(_, amt, _)| *amt == dec("1000.00")));
    assert_eq!(s.last().unwrap().2, dec("12000.00"), "accumulated after last = depreciable");
    let sum: Decimal = s.iter().map(|(_, amt, _)| *amt).sum();
    assert_eq!(sum, dec("12000.00"), "Σ depreciation == depreciable base");
}

/// AGC-2 — a non-divisible schedule ties out exactly: the last period absorbs the rounding residue.
#[tokio::test]
async fn agc2_non_divisible_last_absorbs_residue() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = CountingGl::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = category(&svc, company, &acc, 3).await;
    // 10,000 over 3 → 3333.33, 3333.33, 3333.34 (Σ = 10,000).
    let a = asset(&svc, company, cat, "10000", "0", 0).await;
    svc.activate_asset(a, acc.funding, today(), &gl, &sink).await.unwrap();

    let s = schedule(&pool, a).await;
    let amts: Vec<Decimal> = s.iter().map(|(_, amt, _)| *amt).collect();
    assert_eq!(amts, vec![dec("3333.33"), dec("3333.33"), dec("3333.34")]);
    assert_eq!(amts.iter().copied().sum::<Decimal>(), dec("10000.00"));
}

/// AGC-3 — a salvage value depreciates only (gross − salvage); the asset never drops below salvage.
#[tokio::test]
async fn agc3_salvage_reduces_depreciable() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = CountingGl::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = category(&svc, company, &acc, 4).await;
    // gross 10,000, salvage 2,000 → depreciable 8,000 / 4 = 2,000 each.
    let a = asset(&svc, company, cat, "10000", "2000", 0).await;
    svc.activate_asset(a, acc.funding, today(), &gl, &sink).await.unwrap();

    let s = schedule(&pool, a).await;
    assert_eq!(s.iter().map(|(_, amt, _)| *amt).sum::<Decimal>(), dec("8000.00"));
    assert_eq!(s.last().unwrap().2, dec("8000.00")); // never depreciates the 2,000 salvage
}

/// AGC-4 — validation: salvage must be in [0, gross); life inherited from the category when 0.
#[tokio::test]
async fn agc4_validation_and_life_inheritance() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let sink = LoggingSink;
    let gl = CountingGl::new();
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = category(&svc, company, &acc, 6).await;

    // salvage >= gross → rejected.
    let bad = svc.create_asset(NewAsset {
        company_id: company, asset_category_id: cat, asset_name: "x".into(), asset_code: "X".into(),
        item_id: None, branch_id: None, gross_purchase_amount: dec("1000"), salvage_value: dec("1000"),
        opening_accumulated_depreciation: dec("0"),
        useful_life_months: 0, purchase_date: now(), available_for_use_date: None,
    }).await;
    assert!(matches!(bad, Err(AssetError::Invalid(_))));

    // life 0 → inherit the category's 6 months.
    let a = asset(&svc, company, cat, "6000", "0", 0).await;
    svc.activate_asset(a, acc.funding, today(), &gl, &sink).await.unwrap();
    assert_eq!(schedule(&pool, a).await.len(), 6, "useful life inherited from the category");
}

/// AGC-5 (completeness council 2026-07-06) — ONBOARD an existing, part-depreciated asset. A machine
/// bought earlier: gross 120,000, life 120mo, already 30,000 depreciated on the legacy books. Activation
/// must (a) post NO capitalization (it's already on the opening trial balance) and (b) schedule only the
/// REMAINING 90 periods summing to 90,000, with accumulated starting at 30,000.
#[tokio::test]
async fn agc5_onboard_existing_part_depreciated_asset() {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let gl = CountingGl::new();
    let sink = LoggingSink;
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = category(&svc, company, &acc, 120).await;

    let a = svc.create_asset(NewAsset {
        company_id: company, asset_category_id: cat, asset_name: "M".into(),
        asset_code: format!("A-{}", &Uuid::new_v4().to_string()[..8]),
        item_id: None, branch_id: None, gross_purchase_amount: dec("120000"), salvage_value: dec("0"),
        opening_accumulated_depreciation: dec("30000"), // already 30k depreciated
        useful_life_months: 120, purchase_date: now(), available_for_use_date: None,
    }).await.unwrap();

    // Register carries the opening: accumulated 30,000, NBV 90,000.
    let (accd, nbv): (Decimal, Decimal) = sqlx::query_as(
        "SELECT accumulated_depreciation, net_book_value FROM asset.assets WHERE id=$1")
        .bind(a).fetch_one(&pool).await.unwrap();
    assert_eq!(accd, dec("30000.00"));
    assert_eq!(nbv, dec("90000.00"));

    svc.activate_asset(a, acc.funding, today(), &gl, &sink).await.unwrap();
    // NO capitalization post — the asset is already on the opening trial balance.
    assert_eq!(gl.count("acquire"), 0, "an onboarded asset is NOT re-capitalized");

    let s = schedule(&pool, a).await;
    assert_eq!(s.len(), 90, "only the REMAINING 90 periods are scheduled");
    assert_eq!(s.iter().map(|(_, amt, _)| *amt).sum::<Decimal>(), dec("90000.00"), "remaining base");
    assert_eq!(s.first().unwrap().2, dec("31000.00"), "accumulated continues from the opening 30,000");
    assert_eq!(s.last().unwrap().2, dec("120000.00"), "ends at the full depreciable base");
}
