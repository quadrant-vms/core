use crate::prober::DeviceProber;
use crate::state::DeviceManagerState;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use common::auth_middleware::{AuthContext, RequireAuth};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info};

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
        .route("/v1/devices/:device_id/events", get(get_device_events))
        .route("/v1/devices/batch", put(batch_update_devices))
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn readyz(State(state): State<DeviceManagerState>) -> impl IntoResponse {
    // Check database connectivity
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
    let metric_families = telemetry::metrics_registry().gather();
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
    RequireAuth(auth_ctx): RequireAuth,
    Json(req): Json<CreateDeviceRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:create") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate name
    if let Err(e) = common::validation::validate_name(&req.name, "name") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid name: {}", e)})),
        )
            .into_response();
    }

    // Validate primary_uri
    if let Err(e) = common::validation::validate_uri(&req.primary_uri, "primary_uri") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid primary_uri: {}", e)})),
        )
            .into_response();
    }

    let tenant_id = &auth_ctx.tenant_id;

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
    RequireAuth(auth_ctx): RequireAuth,
    Query(mut query): Query<DeviceListQuery>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Filter by tenant unless system admin
    if !auth_ctx.is_system_admin {
        query.tenant_id = Some(auth_ctx.tenant_id.clone());
    }

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
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            // Check tenant access
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(device)).into_response()
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

async fn update_device(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<UpdateDeviceRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // Check device exists and user has access
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "device not found"})),
            )
                .into_response()
        }
        Err(e) => {
            error!("failed to check device: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }

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
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:delete") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // Check device exists and user has access
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "device not found"})),
            )
                .into_response()
        }
        Err(e) => {
            error!("failed to check device: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }

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
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // Get device
    let device = match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }
            device
        }
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

    // Probe device
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
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // Get device
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }

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
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // Check device exists and user has access
    match state.store.get_device(&device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "device not found"})),
            )
                .into_response()
        }
        Err(e) => {
            error!("failed to check device: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    }

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

async fn get_device_events(
    State(_state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Validate device_id
    if let Err(e) = common::validation::validate_id(&device_id, "device_id") {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid device_id: {}", e)})),
        )
            .into_response();
    }

    // TODO: Implement event retrieval
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "not implemented"})),
    )
        .into_response()
}

async fn batch_update_devices(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Json(req): Json<BatchUpdateRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let mut succeeded = Vec::new();
    let mut failed = HashMap::new();

    for device_id in req.device_ids {
        // Check device exists and user has access
        match state.store.get_device(&device_id).await {
            Ok(Some(device)) => {
                if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                    failed.insert(device_id.clone(), "access denied".to_string());
                    continue;
                }
            }
            Ok(None) => {
                failed.insert(device_id.clone(), "device not found".to_string());
                continue;
            }
            Err(e) => {
                failed.insert(device_id.clone(), e.to_string());
                continue;
            }
        }

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
