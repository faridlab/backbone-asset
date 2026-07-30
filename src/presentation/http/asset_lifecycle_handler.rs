//! The validated, GL-backed asset lifecycle write surface (hand-authored, user-owned).
//!
//! These four verbs are the ONLY way financial state may change — generic CRUD on the
//! financial tables is read-only by default (see `AssetsModule::all_crud_routes`). Each
//! handler delegates to `AssetWriteService`, which posts through the injected `GlPostSink`
//! and publishes through the `AssetEventSink`, so the books and the GL can never diverge.
//!
//! Mounted via `AssetsModule::lifecycle_routes()`, which composes as
//! `module.all_crud_routes().merge(module.lifecycle_routes())`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::application::service::{
    AssetEventSink, AssetWriteService, GlPostSink, NewAsset,
};
// The engine error — NOT the presentation-layer `AssetError` in asset_handler.rs.
use crate::application::service::AssetError as WriteError;
// The verified tenant — proven by the consumer's `company_auth` middleware from the signed JWT.
// This is the ONLY source of `company_id` for the lifecycle verbs; the request body never carries it.
use backbone_auth::company::CompanyContext;

/// Shared state for the lifecycle routes: the write service plus the two sinks the verbs
/// post/publish through. `Clone` (everything is behind an `Arc`), so axum can hand a copy
/// to each request.
#[derive(Clone)]
pub struct AssetLifecycleState {
    pub write_svc: Arc<AssetWriteService>,
    pub gl: Arc<dyn GlPostSink>,
    pub event_sink: Arc<dyn AssetEventSink>,
}

/// Local wrapper around the engine `AssetError` so we can impl axum's `IntoResponse`
/// (foreign trait + foreign type would hit the orphan rule). Maps each domain failure to a
/// stable HTTP code + contract error string.
pub struct LifecycleApiError(pub WriteError);

impl IntoResponse for LifecycleApiError {
    fn into_response(self) -> axum::response::Response {
        let msg = self.0.to_string();
        let (status, code) = match self.0 {
            WriteError::NotFound(_) => (StatusCode::NOT_FOUND, "ASSET_NOT_FOUND"),
            WriteError::Invalid(_)
            | WriteError::InvalidState(_)
            | WriteError::UnsupportedMethod => (StatusCode::BAD_REQUEST, "ASSET_INVALID"),
            WriteError::DuplicateNumber(_) => (StatusCode::CONFLICT, "ASSET_DUPLICATE_CODE"),
            // The GL rejected the posting (e.g. unbalanced / account closed) — the asset did not move.
            WriteError::Gl(_) => (StatusCode::FAILED_DEPENDENCY, "ASSET_GL_REJECTED"),
            WriteError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "ASSET_DATABASE_ERROR"),
        };
        (status, Json(json!({ "success": false, "error": code, "message": msg }))).into_response()
    }
}

// ---------------------------------------------------------------------------
// Request bodies
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterAssetRequest {
    pub asset_category_id: Uuid,
    pub asset_name: String,
    pub asset_code: String,
    pub gross_purchase_amount: Decimal,
    #[serde(default)]
    pub salvage_value: Decimal,
    /// 0 = inherit the category's useful life.
    #[serde(default)]
    pub useful_life_months: i32,
    #[serde(default)]
    pub opening_accumulated_depreciation: Decimal,
    pub purchase_date: DateTime<Utc>,
    #[serde(default)]
    pub available_for_use_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub item_id: Option<Uuid>,
    #[serde(default)]
    pub branch_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ActivateAssetRequest {
    pub funding_account_id: Uuid,
    pub at: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct DepreciateAssetRequest {
    pub up_to: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct DisposeAssetRequest {
    pub proceeds: Decimal,
    pub proceeds_account_id: Uuid,
    pub at: NaiveDate,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// Register a draft asset (validated: gross > 0, salvage in [0, gross), useful-life checks).
#[tracing::instrument(skip_all, fields(company = %tenant.company_id))]
pub async fn register_asset(
    State(st): State<AssetLifecycleState>,
    tenant: CompanyContext,
    Json(req): Json<RegisterAssetRequest>,
) -> Result<(StatusCode, Json<Value>), LifecycleApiError> {
    let id = st
        .write_svc
        .create_asset(NewAsset {
            company_id: tenant.company_id,
            asset_category_id: req.asset_category_id,
            asset_name: req.asset_name,
            asset_code: req.asset_code,
            item_id: req.item_id,
            branch_id: req.branch_id,
            gross_purchase_amount: req.gross_purchase_amount,
            salvage_value: req.salvage_value,
            opening_accumulated_depreciation: req.opening_accumulated_depreciation,
            useful_life_months: req.useful_life_months,
            purchase_date: req.purchase_date,
            available_for_use_date: req.available_for_use_date,
        })
        .await
        .map_err(LifecycleApiError)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": { "id": id } })),
    ))
}

/// Capitalize + generate the straight-line schedule (draft → active).
#[tracing::instrument(skip_all, fields(company = %tenant.company_id, asset = %id))]
pub async fn activate_asset_handler(
    State(st): State<AssetLifecycleState>,
    tenant: CompanyContext,
    Path(id): Path<Uuid>,
    Json(req): Json<ActivateAssetRequest>,
) -> Result<(StatusCode, Json<Value>), LifecycleApiError> {
    st.write_svc
        .activate_asset(id, tenant.company_id, req.funding_account_id, req.at, st.gl.as_ref(), st.event_sink.as_ref())
        .await
        .map_err(LifecycleApiError)?;
    Ok((StatusCode::OK, Json(json!({ "success": true }))))
}

/// Post every depreciation period due on or before `up_to`.
#[tracing::instrument(skip_all, fields(company = %tenant.company_id, asset = %id))]
pub async fn run_depreciation_handler(
    State(st): State<AssetLifecycleState>,
    tenant: CompanyContext,
    Path(id): Path<Uuid>,
    Json(req): Json<DepreciateAssetRequest>,
) -> Result<(StatusCode, Json<Value>), LifecycleApiError> {
    let outcome = st
        .write_svc
        .run_depreciation(id, tenant.company_id, req.up_to, st.gl.as_ref(), st.event_sink.as_ref())
        .await
        .map_err(LifecycleApiError)?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "periods_posted": outcome.periods_posted,
                "total_posted": outcome.total_posted,
                "fully_depreciated": outcome.fully_depreciated,
            }
        })),
    ))
}

/// Remove the asset from the books and recognise gain/loss.
#[tracing::instrument(skip_all, fields(company = %tenant.company_id, asset = %id))]
pub async fn dispose_asset_handler(
    State(st): State<AssetLifecycleState>,
    tenant: CompanyContext,
    Path(id): Path<Uuid>,
    Json(req): Json<DisposeAssetRequest>,
) -> Result<(StatusCode, Json<Value>), LifecycleApiError> {
    let outcome = st
        .write_svc
        .dispose_asset(
            id,
            tenant.company_id,
            req.proceeds,
            req.proceeds_account_id,
            req.at,
            st.gl.as_ref(),
            st.event_sink.as_ref(),
        )
        .await
        .map_err(LifecycleApiError)?;
    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": {
                "net_book_value": outcome.net_book_value,
                "gain_loss": outcome.gain_loss,
                "already": outcome.already,
            }
        })),
    ))
}

/// Mount the four lifecycle verbs. Returns a stateless `Router<()>` ready to merge with
/// `AssetsModule::all_crud_routes()`.
pub fn create_asset_lifecycle_routes(state: AssetLifecycleState) -> Router {
    Router::<AssetLifecycleState>::new()
        .route("/assets/register", post(register_asset))
        .route("/assets/:id/activate", post(activate_asset_handler))
        .route("/assets/:id/depreciate", post(run_depreciation_handler))
        .route("/assets/:id/dispose", post(dispose_asset_handler))
        .with_state(state)
}
