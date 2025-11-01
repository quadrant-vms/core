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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::CoordinatorConfig, state::CoordinatorState, store::MemoryLeaseStore};
    use axum::{body::Body, http::{Request, StatusCode}};
    use serde_json::json;
    use std::{net::SocketAddr, sync::Arc};
    use tower::ServiceExt;

    fn test_state() -> CoordinatorState {
        let config = CoordinatorConfig {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            default_ttl_secs: 10,
            max_ttl_secs: 60,
        };
        let store = Arc::new(MemoryLeaseStore::new(10, 60));
        CoordinatorState::new(config, store)
    }

    #[tokio::test]
    async fn acquire_then_list() {
        let app = router(test_state());
        let acquire_req = Request::builder()
            .method("POST")
            .uri("/v1/leases/acquire")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "resource_id": "cam1",
                    "holder_id": "node-a",
                    "kind": "stream",
                    "ttl_secs": 15
                })
                .to_string(),
            ))
            .unwrap();

        let resp = app.clone().oneshot(acquire_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let list_req = Request::builder()
            .method("GET")
            .uri("/v1/leases")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(list_req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let leases: Vec<LeaseRecord> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(leases.len(), 1);
        assert_eq!(leases[0].resource_id, "cam1");
    }

    #[tokio::test]
    async fn release_clears_lease() {
        let app = router(test_state());
        let acquire_body = json!({
            "resource_id": "cam1",
            "holder_id": "node-a",
            "kind": "stream"
        })
        .to_string();
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/leases/acquire")
                    .header("content-type", "application/json")
                    .body(Body::from(acquire_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let acquire_resp: LeaseAcquireResponse = serde_json::from_slice(&bytes).unwrap();
        let lease_id = acquire_resp.record.unwrap().lease_id;

        let release_body = json!({
            "lease_id": lease_id
        })
        .to_string();
        let release_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/leases/release")
                    .header("content-type", "application/json")
                    .body(Body::from(release_body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(release_resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(release_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let release: LeaseReleaseResponse = serde_json::from_slice(&bytes).unwrap();
        assert!(release.released);

        let list_resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/leases")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let leases: Vec<LeaseRecord> = serde_json::from_slice(&bytes).unwrap();
        assert!(leases.is_empty());
    }
}
