//! HTTP-level test of the delivered bounded context:
//!   1. the generic write surface on the two engine-owned financial tables is CLOSED
//!      (read-only by default), and
//!   2. the validated lifecycle verbs (register -> activate -> depreciate -> dispose) work
//!      over the merged router and post through the GlPostSink with exactly the idempotency
//!      keys the engine emits (acquire: / depr: / dispose:).
//!
//! Needs a live Postgres at DATABASE_URL (see `common::dburl`) — like the other lifecycle tests.

mod common;

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, StatusCode};
use backbone_asset::application::service::{AssetWriteService, NewAssetCategory};
use backbone_asset::AssetsModule;
// The lifecycle handlers source `company_id` from a verified `CompanyContext` (set by the consumer's
// `company_auth` middleware). The test bypasses the JWT and injects the context directly into the
// request extensions — the same place the extractor reads it from.
use backbone_auth::company::CompanyContext;
use common::{pool, CountingGl};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn req(method: Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// Like `req` but injects a verified `CompanyContext` for `company` into the request extensions,
/// standing in for the `company_auth` middleware on the HTTP path.
fn req_as(company: Uuid, method: Method, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .extension(CompanyContext {
            company_id: company,
            branch_id: None,
            user_id: "test".into(),
        })
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

#[tokio::test]
async fn financial_write_surface_is_read_only_by_default() {
    // Default surface only — no lifecycle routes mounted.
    let module = AssetsModule::builder().with_database(pool().await).build().unwrap();
    let router = module.read_only_routes();

    // Generic create on the financial tables must be refused.
    let r = router.clone().oneshot(req(Method::POST, "/assets", json!({}))).await.unwrap();
    assert!(
        r.status() == StatusCode::METHOD_NOT_ALLOWED || r.status() == StatusCode::NOT_FOUND,
        "generic POST /assets should be closed, got {}",
        r.status()
    );

    let r = router
        .clone()
        .oneshot(req(Method::POST, "/asset_depreciation_entries", json!({})))
        .await
        .unwrap();
    assert!(
        r.status() == StatusCode::METHOD_NOT_ALLOWED || r.status() == StatusCode::NOT_FOUND,
        "generic POST /asset_depreciation_entries should be closed, got {}",
        r.status()
    );
}

#[tokio::test]
async fn lifecycle_verbs_post_through_the_sink() {
    let pool = pool().await;
    let company = Uuid::new_v4();
    let gl = std::sync::Arc::new(CountingGl::new());

    // Category setup via a standalone write service (the module's field is pub(crate)).
    let setup = AssetWriteService::new(pool.clone());
    let cat = setup
        .create_category(NewAssetCategory {
            company_id: company,
            category_name: "Machinery".into(),
            useful_life_months: 12,
            // CountingGl never posts, so the account ids are never exercised — arbitrary uuids are
            // fine (the schema stores them as logical FKs with no DB constraint).
            fixed_asset_account_id: Uuid::new_v4(),
            accumulated_depreciation_account_id: Uuid::new_v4(),
            depreciation_expense_account_id: Uuid::new_v4(),
            disposal_gain_loss_account_id: Uuid::new_v4(),
        })
        .await
        .unwrap();

    let module = AssetsModule::builder()
        .with_database(pool.clone())
        .with_gl_sink(gl.clone())
        .build()
        .unwrap();
    let router = module.read_only_routes().merge(module.lifecycle_routes());

    // 1) register a draft asset (validated path). company_id comes from the injected CompanyContext.
    let r = router
        .clone()
        .oneshot(req_as(
            company,
            Method::POST,
            "/assets/register",
            json!({
                "asset_category_id": cat,
                "asset_name": "Lathe",
                "asset_code": format!("AST-{}", &Uuid::new_v4().to_string()[..8]),
                "gross_purchase_amount": "1200.00",
                "salvage_value": "0",
                "useful_life_months": 12,
                "purchase_date": "2025-01-01T00:00:00Z",
            }),
        ))
        .await
        .unwrap();
    let (status, body) = (r.status(), body_json(r).await);
    assert_eq!(status, StatusCode::CREATED, "register failed: {body:?}");
    let id = body["data"]["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // 2) activate (capitalize + generate the 12-period schedule).
    let r = router
        .clone()
        .oneshot(req_as(
            company,
            Method::POST,
            &format!("/assets/{id}/activate"),
            json!({ "funding_account_id": Uuid::new_v4(), "at": "2025-01-01" }),
        ))
        .await
        .unwrap();
    let (status, body) = (r.status(), body_json(r).await);
    assert_eq!(status, StatusCode::OK, "activate failed: {body:?}");

    // 3) depreciate every due period (up_to well past the schedule).
    let r = router
        .clone()
        .oneshot(req_as(
            company,
            Method::POST,
            &format!("/assets/{id}/depreciate"),
            json!({ "up_to": "2030-01-01T00:00:00Z" }),
        ))
        .await
        .unwrap();
    let (status, body) = (r.status(), body_json(r).await);
    assert_eq!(status, StatusCode::OK, "depreciate failed: {body:?}");
    assert_eq!(body["data"]["periods_posted"].as_i64(), Some(12));
    assert_eq!(body["data"]["fully_depreciated"].as_bool(), Some(true));

    // 4) dispose (proceeds 0 — asset is fully depreciated, so NBV is 0).
    let r = router
        .clone()
        .oneshot(req_as(
            company,
            Method::POST,
            &format!("/assets/{id}/dispose"),
            json!({
                "proceeds": "0",
                "proceeds_account_id": Uuid::new_v4(),
                "at": "2030-01-01",
            }),
        ))
        .await
        .unwrap();
    let (status, body) = (r.status(), body_json(r).await);
    assert_eq!(status, StatusCode::OK, "dispose failed: {body:?}");

    // The GL sink saw exactly the posts the engine emits.
    assert_eq!(gl.count("acquire:"), 1);
    assert_eq!(gl.count("depr:"), 12);
    assert_eq!(gl.count("dispose:"), 1);
}
