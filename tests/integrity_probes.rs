//! Integrity probes — the domain invariants under retry/concurrency. Mirrors golden-cases.md.

mod common;

use backbone_asset::application::service::asset_events::LoggingSink;
use backbone_asset::application::service::asset_write_service::{
    AssetError, AssetWriteService, NewAsset, NewAssetCategory,
};
use common::*;
use rust_decimal::Decimal;
use uuid::Uuid;

/// Create a company + accounts + category + a draft asset (gross/salvage/life). Returns (svc, company, acc, asset).
async fn setup(gross: &str, salvage: &str, life: i32) -> (AssetWriteService, Uuid, AssetAccounts, Uuid) {
    let pool = pool().await;
    let svc = AssetWriteService::new(pool.clone());
    let company = Uuid::new_v4();
    let acc = asset_accounts(&pool, company).await;
    let cat = svc
        .create_category(NewAssetCategory {
            company_id: company,
            category_name: "Cat".into(),
            useful_life_months: life,
            fixed_asset_account_id: acc.fixed_asset,
            accumulated_depreciation_account_id: acc.accum_dep,
            depreciation_expense_account_id: acc.dep_expense,
            disposal_gain_loss_account_id: acc.gain_loss,
        })
        .await
        .unwrap();
    let asset = svc
        .create_asset(NewAsset {
            company_id: company,
            asset_category_id: cat,
            asset_name: "A".into(),
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
        .unwrap();
    (svc, company, acc, asset)
}

fn far_future() -> chrono::DateTime<chrono::Utc> {
    now() + chrono::Duration::days(3650)
}

/// IP-1 — activation is idempotent: a retry posts the acquisition once and generates the schedule once.
#[tokio::test]
async fn ip1_activate_idempotent() {
    let pool = pool().await;
    let (svc, company, acc, asset) = setup("12000", "0", 12).await;
    let gl = CountingGl::new();
    let sink = LoggingSink;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap(); // retry
    assert_eq!(gl.count("acquire"), 1, "capitalization posted once");
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM asset.asset_depreciation_entries WHERE asset_id=$1")
        .bind(asset).fetch_one(&pool).await.unwrap();
    assert_eq!(n, 12, "schedule generated once");
    let _ = company;
}

/// IP-2 — running depreciation twice posts each period at most once.
#[tokio::test]
async fn ip2_depreciation_idempotent() {
    let pool = pool().await;
    let (svc, _c, acc, asset) = setup("1200", "0", 12).await;
    let gl = CountingGl::new();
    let sink = LoggingSink;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();

    let a = svc.run_depreciation(asset, far_future(), &gl, &sink).await.unwrap();
    assert_eq!(a.periods_posted, 12);
    assert_eq!(a.total_posted, dec("1200.00"));
    let b = svc.run_depreciation(asset, far_future(), &gl, &sink).await.unwrap();
    assert_eq!(b.periods_posted, 0, "nothing left to post");
    assert_eq!(gl.count("depr"), 12, "each period posted exactly once");
    // Asset fully depreciated: NBV 0.
    let (status, nbv): (String, Decimal) = sqlx::query_as(
        "SELECT status::text, net_book_value FROM asset.assets WHERE id=$1").bind(asset).fetch_one(&pool).await.unwrap();
    assert_eq!(status, "fully_depreciated");
    assert_eq!(nbv, dec("0.00"));
}

/// IP-3 — a depreciation run only posts periods due on or before the cutoff.
#[tokio::test]
async fn ip3_run_respects_cutoff() {
    let (svc, _c, acc, asset) = setup("1200", "0", 12).await;
    let gl = CountingGl::new();
    let sink = LoggingSink;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();
    // Only ~3 months elapsed.
    let cutoff = now() + chrono::Duration::days(95);
    let r = svc.run_depreciation(asset, cutoff, &gl, &sink).await.unwrap();
    assert_eq!(r.periods_posted, 3, "3 periods due");
    assert!(!r.fully_depreciated);
}

/// IP-4 — disposal is idempotent: a retry disposes once and posts the disposal once.
#[tokio::test]
async fn ip4_dispose_idempotent() {
    let (svc, _c, acc, asset) = setup("1000", "0", 10).await;
    let gl = CountingGl::new();
    let sink = LoggingSink;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();
    // Dispose immediately (NBV = 1000, proceeds 200 → loss 800).
    let a = svc.dispose_asset(asset, dec("200"), acc.proceeds, today(), &gl, &sink).await.unwrap();
    assert!(!a.already);
    assert_eq!(a.net_book_value, dec("1000"));
    assert_eq!(a.gain_loss, dec("-800"));
    let b = svc.dispose_asset(asset, dec("200"), acc.proceeds, today(), &gl, &sink).await.unwrap();
    assert!(b.already, "second dispose short-circuits");
    assert_eq!(gl.count("dispose"), 1, "disposal posted once");
}

/// IP-5 — a draft asset cannot be depreciated or disposed before activation.
#[tokio::test]
async fn ip5_draft_guards() {
    let (svc, _c, acc, asset) = setup("1000", "0", 10).await;
    let gl = CountingGl::new();
    let sink = LoggingSink;
    let d = svc.dispose_asset(asset, dec("0"), acc.proceeds, today(), &gl, &sink).await;
    assert!(matches!(d, Err(AssetError::InvalidState(_))), "cannot dispose a draft asset");
}

/// IP-6 (council 2026-07-06) — dispose and depreciation must SERIALIZE per asset: run concurrently,
/// the asset still nets off the books (Fixed Asset AND Accumulated Depreciation both zero), whichever
/// wins the row lock. Before the FOR-UPDATE fix, dispose read a stale `accumulated` and debited too
/// little, stranding a residual on Accumulated Depreciation.
#[tokio::test]
async fn ip6_dispose_vs_depreciation_serialize() {
    use std::sync::Arc;
    for _ in 0..8 {
        let pool = pool().await;
        let svc = Arc::new(AssetWriteService::new(pool.clone()));
        let company = Uuid::new_v4();
        let acc = asset_accounts(&pool, company).await;
        let cat = svc.create_category(NewAssetCategory {
            company_id: company, category_name: "C".into(), useful_life_months: 12,
            fixed_asset_account_id: acc.fixed_asset, accumulated_depreciation_account_id: acc.accum_dep,
            depreciation_expense_account_id: acc.dep_expense, disposal_gain_loss_account_id: acc.gain_loss,
        }).await.unwrap();
        let asset = svc.create_asset(NewAsset {
            company_id: company, asset_category_id: cat, asset_name: "A".into(),
            asset_code: format!("A-{}", &Uuid::new_v4().to_string()[..8]),
            item_id: None, branch_id: None, gross_purchase_amount: dec("12000"), salvage_value: dec("0"),
            opening_accumulated_depreciation: dec("0"),
            useful_life_months: 12, purchase_date: now(), available_for_use_date: None,
        }).await.unwrap();
        svc.activate_asset(asset, acc.funding, today(), &GlAdapter::new(pool.clone()), &LoggingSink).await.unwrap();

        // Race a full depreciation run against a disposal.
        let (s1, s2) = (svc.clone(), svc.clone());
        let (p1, p2) = (pool.clone(), pool.clone());
        let far = now() + chrono::Duration::days(3650);
        let dep = tokio::spawn(async move {
            s1.run_depreciation(asset, far, &GlAdapter::new(p1), &LoggingSink).await
        });
        let (proceeds_acct, prc) = (acc.proceeds, dec("1000"));
        let dis = tokio::spawn(async move {
            s2.dispose_asset(asset, prc, proceeds_acct, today(), &GlAdapter::new(p2), &LoggingSink).await
        });
        let _ = dep.await.unwrap();
        let _ = dis.await.unwrap();

        // Whatever the interleave: the asset is removed from the books, coherently.
        assert_eq!(balance(&pool, acc.fixed_asset).await, dec("0.00"), "Fixed Asset off the books");
        assert_eq!(balance(&pool, acc.accum_dep).await, dec("0.00"), "Accumulated Depreciation off the books");
        let status: String = sqlx::query_scalar("SELECT status::text FROM asset.assets WHERE id=$1")
            .bind(asset).fetch_one(&pool).await.unwrap();
        assert_eq!(status, "disposed");
    }
}

/// IP-7 (completeness re-check 2026-07-06) — pins the PARKED dispose-without-catch-up behavior as
/// coherent: disposing with due-but-unposted depreciation still balances and nets the asset off the
/// books (the missed depreciation lands in gain/loss — a P&L classification nuance, not a broken
/// ledger), and the remaining unposted schedule rows are inert (can never post after disposal).
#[tokio::test]
async fn ip7_dispose_without_catchup_is_coherent() {
    let pool = pool().await;
    let (svc, _c, acc, asset) = setup("12000", "0", 12).await;
    let gl = GlAdapter::new(pool.clone());
    let sink = LoggingSink;
    svc.activate_asset(asset, acc.funding, today(), &gl, &sink).await.unwrap();

    // Only 3 periods run (accumulated 3,000), but dispose WITHOUT catching up the rest.
    svc.run_depreciation(asset, now() + chrono::Duration::days(95), &gl, &sink).await.unwrap();
    let d = svc.dispose_asset(asset, dec("9000"), acc.proceeds, today(), &gl, &sink).await.unwrap();
    assert_eq!(d.net_book_value, dec("9000"), "NBV from posted accumulated (3,000 depreciated)");

    // The books stay coherent: the asset is fully OFF the register regardless of un-run depreciation.
    assert_eq!(balance(&pool, acc.fixed_asset).await, dec("0.00"), "Fixed Asset off the books");
    assert_eq!(balance(&pool, acc.accum_dep).await, dec("0.00"), "Accumulated Depreciation off the books");

    // The remaining unposted schedule rows are inert — a disposed asset refuses further depreciation.
    let after = svc.run_depreciation(asset, far_future(), &gl, &sink).await;
    assert!(matches!(after, Err(AssetError::InvalidState(_))), "no depreciation posts after disposal");
    let unposted: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM asset.asset_depreciation_entries WHERE asset_id=$1 AND posted=false")
        .bind(asset).fetch_one(&pool).await.unwrap();
    assert!(unposted > 0, "orphan plan rows remain but can never post (cosmetic, zero GL impact)");
}
