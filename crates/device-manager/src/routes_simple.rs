// Simplified routes without auth for initial implementation
// TODO: Add proper authentication using auth-service integration

use crate::state::DeviceManagerState;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde_json::json;
use std::collections::HashMap;
use tracing::{error, info, warn};

pub fn router(state: DeviceManagerState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/readyz", get(readyz))
        .route("/metrics", get(metrics))
        .route("/v1/devices", post(create_device))
        .route("/v1/devices", get(list_devices))
        .route("/v1/devices/:device_id", get(get_device))
        .route("/v1/devices/:device_id", put(update_device))
        .route("/v1/devices/:device_id", delete(delete_device))
        .route("/v1/devices/:device_id/probe", post(probe_device))
        .route("/v1/devices/:device_id/health", get(get_device_health))
        .route("/v1/devices/:device_id/health/history", get(get_health_history))
        .route("/v1/devices/batch", put(batch_update_devices))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn readyz(State(state): State<DeviceManagerState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").fetch_one(state.store.pool()).await {
        Ok(_) => (StatusCode::OK, Json(json!({"status": "ready"}))),
        Err(e) => {
            error!("readyz check failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"status": "not ready", "error": e.to_string()})),
            )
        }
    }
}

async fn metrics() -> impl IntoResponse {
    use prometheus::{Encoder, TextEncoder};
    let encoder = TextEncoder::new();
    let metric_families = telemetry::metrics::REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        buffer,
    )
}

async fn create_device(
    State(state): State<DeviceManagerState>,
    Json(req): Json<CreateDeviceRequest>,
) -> impl IntoResponse {
    // TODO: Extract tenant_id from auth context
    let tenant_id = "system"; // Default tenant for now

    match state.store.create_device(tenant_id, req).await {
        Ok(device) => {
            info!(
                device_id = %device.device_id,
                device_name = %device.name,
                tenant_id = %tenant_id,
                "device created"
            );
            (StatusCode::CREATED, Json(device)).into_response()
        }
        Err(e) => {
            error!("failed to create device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn list_devices(
    State(state): State<DeviceManagerState>,
    Query(query): Query<DeviceListQuery>,
) -> impl IntoResponse {
    match state.store.list_devices(query).await {
        Ok(devices) => (StatusCode::OK, Json(devices)).into_response(),
        Err(e) => {
            error!("failed to list devices: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn get_device(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => (StatusCode::OK, Json(device)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "device not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("failed to get device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn update_device(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<UpdateDeviceRequest>,
) -> impl IntoResponse {
    match state.store.update_device(&device_id, req).await {
        Ok(device) => {
            info!(
                device_id = %device.device_id,
                device_name = %device.name,
                "device updated"
            );
            (StatusCode::OK, Json(device)).into_response()
        }
        Err(e) => {
            error!("failed to update device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn delete_device(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match state.store.delete_device(&device_id).await {
        Ok(_) => {
            info!(device_id = %device_id, "device deleted");
            (StatusCode::NO_CONTENT, Json(json!({}))).into_response()
        }
        Err(e) => {
            error!("failed to delete device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn probe_device(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let device = match state.store.get_device(&device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "device not found"})),
            )
                .into_response()
        }
        Err(e) => {
            error!("failed to get device: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let username = device.username.as_deref();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok())
        .as_deref();

    match state
        .prober
        .probe_device(&device.primary_uri, &device.protocol, username, password)
        .await
    {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => {
            error!("failed to probe device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn get_device_health(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            let health = json!({
                "device_id": device.device_id,
                "status": device.status,
                "last_seen_at": device.last_seen_at,
                "last_health_check_at": device.last_health_check_at,
                "consecutive_failures": device.consecutive_failures,
            });
            (StatusCode::OK, Json(health)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "device not found"})),
        )
            .into_response(),
        Err(e) => {
            error!("failed to get device: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn get_health_history(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let limit = query
        .get("limit")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(100);

    match state.store.get_health_history(&device_id, limit).await {
        Ok(history) => (StatusCode::OK, Json(history)).into_response(),
        Err(e) => {
            error!("failed to get health history: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn batch_update_devices(
    State(state): State<DeviceManagerState>,
    Json(req): Json<BatchUpdateRequest>,
) -> impl IntoResponse {
    let mut succeeded = Vec::new();
    let mut failed = HashMap::new();

    for device_id in req.device_ids {
        match state.store.update_device(&device_id, req.update.clone()).await {
            Ok(_) => succeeded.push(device_id),
            Err(e) => {
                failed.insert(device_id, e.to_string());
            }
        }
    }

    info!(
        succeeded = succeeded.len(),
        failed = failed.len(),
        "batch device update completed"
    );

    let response = BatchUpdateResponse { succeeded, failed };
    (StatusCode::OK, Json(response)).into_response()
}
