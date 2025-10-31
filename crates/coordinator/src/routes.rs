use crate::{error::ApiError, state::CoordinatorState};
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use common::leases::{
    LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest,
    LeaseReleaseResponse, LeaseRenewRequest, LeaseRenewResponse,
};
use serde::Deserialize;

pub fn router(state: CoordinatorState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/leases", get(list_leases))
        .route("/v1/leases/acquire", post(acquire_lease))
        .route("/v1/leases/renew", post(renew_lease))
        .route("/v1/leases/release", post(release_lease))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct ListLeasesQuery {
    kind: Option<String>,
}

async fn list_leases(
    State(state): State<CoordinatorState>,
    Query(query): Query<ListLeasesQuery>,
) -> Result<Json<Vec<LeaseRecord>>, ApiError> {
    let maybe_kind = if let Some(kind_str) = query.kind {
        if kind_str.is_empty() {
            None
        } else {
            Some(kind_str.parse::<LeaseKind>().map_err(|_| {
                ApiError::bad_request(format!("unknown lease kind '{}'", kind_str))
            })?)
        }
    } else {
        None
    };

    let store = state.store();
    let records = store.list(maybe_kind).await?;
    Ok(Json(records))
}

async fn acquire_lease(
    State(state): State<CoordinatorState>,
    Json(request): Json<LeaseAcquireRequest>,
) -> Result<Json<LeaseAcquireResponse>, ApiError> {
    let store = state.store();
    let resp = store.acquire(request).await?;
    Ok(Json(resp))
}

async fn renew_lease(
    State(state): State<CoordinatorState>,
    Json(request): Json<LeaseRenewRequest>,
) -> Result<Json<LeaseRenewResponse>, ApiError> {
    let store = state.store();
    let resp = store.renew(request).await?;
    Ok(Json(resp))
}

async fn release_lease(
    State(state): State<CoordinatorState>,
    Json(request): Json<LeaseReleaseRequest>,
) -> Result<Json<LeaseReleaseResponse>, ApiError> {
    let store = state.store();
    let resp = store.release(request).await?;
    Ok(Json(resp))
}
