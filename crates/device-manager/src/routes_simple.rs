// Simplified routes with JWT authentication
use crate::imaging_client::create_imaging_client;
use crate::ptz_client::create_ptz_client;
use crate::state::DeviceManagerState;
use crate::types::*;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use chrono::Utc;
use common::auth_middleware::RequireAuth;
use serde_json::json;
use std::collections::HashMap;
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
        .route("/v1/devices/batch", put(batch_update_devices))
        // Discovery routes
        .route("/v1/discovery/scan", post(start_discovery_scan))
        .route("/v1/discovery/scans", get(list_discovery_scans))
        .route("/v1/discovery/scans/:scan_id", get(get_discovery_scan))
        .route("/v1/discovery/scans/:scan_id/devices", get(get_discovered_devices))
        .route("/v1/discovery/scans/:scan_id/cancel", post(cancel_discovery_scan))
        // PTZ Control routes
        .route("/v1/devices/:device_id/ptz/move", post(ptz_move))
        .route("/v1/devices/:device_id/ptz/stop", post(ptz_stop))
        .route("/v1/devices/:device_id/ptz/zoom", post(ptz_zoom))
        .route("/v1/devices/:device_id/ptz/absolute", post(ptz_goto_absolute))
        .route("/v1/devices/:device_id/ptz/home", post(ptz_goto_home))
        .route("/v1/devices/:device_id/ptz/status", get(ptz_get_status))
        .route("/v1/devices/:device_id/ptz/capabilities", get(ptz_get_capabilities))
        // PTZ Preset routes
        .route("/v1/devices/:device_id/ptz/presets", post(create_ptz_preset))
        .route("/v1/devices/:device_id/ptz/presets", get(list_ptz_presets))
        .route("/v1/devices/:device_id/ptz/presets/:preset_id", get(get_ptz_preset))
        .route("/v1/devices/:device_id/ptz/presets/:preset_id", put(update_ptz_preset))
        .route("/v1/devices/:device_id/ptz/presets/:preset_id", delete(delete_ptz_preset))
        .route("/v1/devices/:device_id/ptz/presets/:preset_id/goto", post(goto_ptz_preset))
        // PTZ Tour routes
        .route("/v1/devices/:device_id/ptz/tours", post(create_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours", get(list_ptz_tours))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id", get(get_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id", put(update_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id", delete(delete_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/steps", post(add_ptz_tour_step))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/steps/:step_id", delete(delete_ptz_tour_step))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/start", post(start_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/stop", post(stop_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/pause", post(pause_ptz_tour))
        .route("/v1/devices/:device_id/ptz/tours/:tour_id/resume", post(resume_ptz_tour))
        // Camera Configuration routes
        .route("/v1/devices/:device_id/configuration", post(configure_camera))
        .route("/v1/devices/:device_id/configuration", get(get_current_configuration))
        .route("/v1/devices/:device_id/configuration/history", get(get_configuration_history))
        .route("/v1/devices/:device_id/configuration/:config_id", get(get_configuration_by_id))
        // Firmware Management routes
        .route("/v1/firmware/files", post(crate::firmware_routes::upload_firmware_file))
        .route("/v1/firmware/files", get(crate::firmware_routes::list_firmware_files))
        .route("/v1/firmware/files/:file_id", get(crate::firmware_routes::get_firmware_file))
        .route("/v1/firmware/files/:file_id/verify", post(crate::firmware_routes::verify_firmware_file))
        .route("/v1/firmware/files/:file_id", delete(crate::firmware_routes::delete_firmware_file))
        .route("/v1/firmware/updates", get(crate::firmware_routes::list_firmware_updates))
        .route("/v1/firmware/updates/:update_id", get(crate::firmware_routes::get_firmware_update))
        .route("/v1/firmware/updates/:update_id/history", get(crate::firmware_routes::get_firmware_update_history))
        .route("/v1/firmware/updates/:update_id/cancel", post(crate::firmware_routes::cancel_firmware_update))
        .route("/v1/devices/:device_id/firmware/update", post(crate::firmware_routes::initiate_firmware_update))
        .route("/v1/devices/:device_id/firmware/updates", get(crate::firmware_routes::list_device_firmware_updates))
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

    // Extract tenant_id from auth context
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
    let password_decrypted = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());
    let password = password_decrypted.as_deref();

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

// PTZ Control Handlers

async fn ptz_move(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<PtzMoveRequest>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.move_camera(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_stop(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<PtzStopRequest>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.stop(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_zoom(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<PtzZoomRequest>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.zoom(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_goto_absolute(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<PtzAbsolutePositionRequest>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.goto_absolute_position(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_goto_home(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.goto_home().await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_get_status(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.get_status().await {
            Ok(status) => (StatusCode::OK, Json(status)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

async fn ptz_get_capabilities(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.get_capabilities().await {
            Ok(capabilities) => (StatusCode::OK, Json(capabilities)).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => response,
    }
}

// PTZ Preset Handlers

async fn create_ptz_preset(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<CreatePtzPresetRequest>,
) -> impl IntoResponse {
    let position = match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => match client.get_status().await {
            Ok(status) => status.position,
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        },
        Err(response) => return response,
    };

    match state.store.create_ptz_preset(&device_id, req, position).await {
        Ok(preset) => (StatusCode::CREATED, Json(preset)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn list_ptz_presets(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match state.store.list_ptz_presets(&device_id).await {
        Ok(presets) => (StatusCode::OK, Json(presets)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn get_ptz_preset(
    State(state): State<DeviceManagerState>,
    Path((_device_id, preset_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.store.get_ptz_preset(&preset_id).await {
        Ok(Some(preset)) => (StatusCode::OK, Json(preset)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, Json(json!({"error": "preset not found"}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn update_ptz_preset(
    State(state): State<DeviceManagerState>,
    Path((_device_id, preset_id)): Path<(String, String)>,
    Json(req): Json<UpdatePtzPresetRequest>,
) -> impl IntoResponse {
    match state.store.update_ptz_preset(&preset_id, req).await {
        Ok(preset) => (StatusCode::OK, Json(preset)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn delete_ptz_preset(
    State(state): State<DeviceManagerState>,
    Path((_device_id, preset_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.store.delete_ptz_preset(&preset_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn goto_ptz_preset(
    State(state): State<DeviceManagerState>,
    Path((device_id, preset_id)): Path<(String, String)>,
    Json(req): Json<GotoPresetRequest>,
) -> impl IntoResponse {
    let preset = match state.store.get_ptz_preset(&preset_id).await {
        Ok(Some(preset)) => preset,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "preset not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    match get_device_and_create_client(&state, &device_id).await {
        Ok(client) => {
            let absolute_req = PtzAbsolutePositionRequest {
                pan: preset.position.pan,
                tilt: preset.position.tilt,
                zoom: preset.position.zoom,
                speed: req.speed,
            };
            match client.goto_absolute_position(&absolute_req).await {
                Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
                Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
            }
        }
        Err(response) => response,
    }
}

// PTZ Tour Handlers

async fn create_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Json(req): Json<CreatePtzTourRequest>,
) -> impl IntoResponse {
    match state.store.create_ptz_tour(&device_id, req).await {
        Ok(tour) => (StatusCode::CREATED, Json(tour)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn list_ptz_tours(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    match state.store.list_ptz_tours(&device_id).await {
        Ok(tours) => (StatusCode::OK, Json(tours)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn get_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let tour = match state.store.get_ptz_tour(&tour_id).await {
        Ok(Some(tour)) => tour,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({"error": "tour not found"}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    let steps = match state.store.get_ptz_tour_steps(&tour_id).await {
        Ok(steps) => steps,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    (StatusCode::OK, Json(json!({"tour": tour, "steps": steps}))).into_response()
}

async fn update_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
    Json(req): Json<UpdatePtzTourRequest>,
) -> impl IntoResponse {
    match state.store.update_ptz_tour(&tour_id, req).await {
        Ok(tour) => (StatusCode::OK, Json(tour)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn delete_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.store.delete_ptz_tour(&tour_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn add_ptz_tour_step(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
    Json(req): Json<AddTourStepRequest>,
) -> impl IntoResponse {
    match state.store.add_ptz_tour_step(&tour_id, req).await {
        Ok(step) => (StatusCode::CREATED, Json(step)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn delete_ptz_tour_step(
    State(state): State<DeviceManagerState>,
    Path((_device_id, _tour_id, step_id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    match state.store.delete_ptz_tour_step(&step_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

async fn start_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.tour_executor.start_tour(tour_id).await {
        Ok(_) => {
            info!("PTZ tour started");
            (StatusCode::OK, Json(json!({"status": "started"}))).into_response()
        }
        Err(e) => {
            error!("failed to start tour: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn stop_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.tour_executor.stop_tour(&tour_id).await {
        Ok(_) => {
            info!("PTZ tour stopped");
            (StatusCode::OK, Json(json!({"status": "stopped"}))).into_response()
        }
        Err(e) => {
            error!("failed to stop tour: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn pause_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.tour_executor.pause_tour(&tour_id).await {
        Ok(_) => {
            info!("PTZ tour paused");
            (StatusCode::OK, Json(json!({"status": "paused"}))).into_response()
        }
        Err(e) => {
            error!("failed to pause tour: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

async fn resume_ptz_tour(
    State(state): State<DeviceManagerState>,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.tour_executor.resume_tour(&tour_id).await {
        Ok(_) => {
            info!("PTZ tour resumed");
            (StatusCode::OK, Json(json!({"status": "resumed"}))).into_response()
        }
        Err(e) => {
            error!("failed to resume tour: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()
        }
    }
}

// Discovery endpoints

async fn start_discovery_scan(
    State(state): State<DeviceManagerState>,
) -> impl IntoResponse {
    info!("starting ONVIF device discovery scan");

    // Start scan
    let scan_id = match state.discovery_client.start_scan().await {
        Ok(id) => id,
        Err(e) => {
            error!("failed to start discovery scan: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Spawn background task to perform discovery
    let discovery_client = state.discovery_client.clone();
    let store = state.store.clone();
    let scan_id_clone = scan_id.clone();

    tokio::spawn(async move {
        match discovery_client.discover_devices(&scan_id_clone).await {
            Ok(result) => {
                info!(
                    scan_id = %scan_id_clone,
                    devices_found = result.total_found,
                    "discovery scan completed"
                );

                // Save scan to database
                if let Some(scan) = discovery_client.get_scan_status(&scan_id_clone).await {
                    if let Err(e) = store.save_discovery_scan(&scan).await {
                        error!("failed to save discovery scan: {}", e);
                    }
                }

                // Save discovered devices to database
                for device in result.devices {
                    if let Err(e) = store.save_discovered_device(&scan_id_clone, &device).await {
                        error!("failed to save discovered device: {}", e);
                    }
                }
            }
            Err(e) => {
                error!(scan_id = %scan_id_clone, error = %e, "discovery scan failed");
            }
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "scan_id": scan_id,
            "status": "running",
            "message": "Discovery scan started"
        })),
    )
        .into_response()
}

async fn list_discovery_scans(
    State(state): State<DeviceManagerState>,
) -> impl IntoResponse {
    match state.discovery_client.list_scans().await {
        scans => {
            info!(count = scans.len(), "listed discovery scans");
            (StatusCode::OK, Json(json!({"scans": scans}))).into_response()
        }
    }
}

async fn get_discovery_scan(
    State(state): State<DeviceManagerState>,
    Path(scan_id): Path<String>,
) -> impl IntoResponse {
    match state.discovery_client.get_scan_status(&scan_id).await {
        Some(scan) => {
            info!(scan_id = %scan_id, "retrieved discovery scan");
            (StatusCode::OK, Json(scan)).into_response()
        }
        None => {
            error!(scan_id = %scan_id, "discovery scan not found");
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "scan not found"})),
            )
                .into_response()
        }
    }
}

async fn get_discovered_devices(
    State(state): State<DeviceManagerState>,
    Path(scan_id): Path<String>,
) -> impl IntoResponse {
    match state.store.list_discovered_devices(&scan_id).await {
        Ok(devices) => {
            info!(scan_id = %scan_id, count = devices.len(), "listed discovered devices");
            (StatusCode::OK, Json(json!({"devices": devices}))).into_response()
        }
        Err(e) => {
            error!(scan_id = %scan_id, error = %e, "failed to list discovered devices");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

async fn cancel_discovery_scan(
    State(state): State<DeviceManagerState>,
    Path(scan_id): Path<String>,
) -> impl IntoResponse {
    match state.discovery_client.cancel_scan(&scan_id).await {
        Ok(_) => {
            info!(scan_id = %scan_id, "discovery scan cancelled");
            (StatusCode::OK, Json(json!({"status": "cancelled"}))).into_response()
        }
        Err(e) => {
            error!(scan_id = %scan_id, error = %e, "failed to cancel scan");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

// Helper function
async fn get_device_and_create_client(
    state: &DeviceManagerState,
    device_id: &str,
) -> Result<std::sync::Arc<dyn crate::ptz_client::PtzClient>, axum::response::Response> {
    let device = match state.store.get_device(device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "device not found"}))).into_response()),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()),
    };

    let username = device.username.clone();
    let password = device.password_encrypted.as_ref().and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => Ok(client),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()),
    }
}

// Helper function for creating imaging clients
async fn get_device_and_create_imaging_client(
    state: &DeviceManagerState,
    device_id: &str,
) -> Result<std::sync::Arc<dyn crate::imaging_client::ImagingClient>, axum::response::Response> {
    let device = match state.store.get_device(device_id).await {
        Ok(Some(device)) => device,
        Ok(None) => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "device not found"}))).into_response()),
        Err(e) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()),
    };

    let username = device.username.clone();
    let password = device.password_encrypted.as_ref().and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_imaging_client(&device.protocol, &device.primary_uri, username, password, device_id) {
        Ok(client) => Ok(client),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response()),
    }
}

// Camera Configuration Handlers

/// Configure camera settings
async fn configure_camera(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(config_request): Json<CameraConfigurationRequest>,
) -> impl IntoResponse {
    // Check permission
    if !auth_ctx.has_permission("device:configure") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    info!(device_id = %device_id, user = %auth_ctx.username, "configuring camera");

    // Get device and create imaging client
    let imaging_client = match get_device_and_create_imaging_client(&state, &device_id).await {
        Ok(client) => client,
        Err(response) => return response,
    };

    // Apply configuration to device
    let response = match imaging_client.configure_camera(&config_request).await {
        Ok(resp) => resp,
        Err(e) => {
            error!(device_id = %device_id, error = %e, "failed to configure camera");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to configure camera: {}", e)})),
            )
                .into_response();
        }
    };

    // Save configuration to database
    let config = DeviceConfiguration {
        config_id: response.config_id.clone(),
        device_id: device_id.clone(),
        requested_config: serde_json::to_value(&config_request).unwrap_or_default(),
        applied_config: Some(serde_json::to_value(&response.applied_settings).unwrap_or_default()),
        status: response.status.clone(),
        error_message: response.error_message.clone(),
        applied_by: Some(auth_ctx.username.clone()), // Get from auth context
        created_at: Utc::now(),
        applied_at: response.applied_at,
    };

    match state.store.save_device_configuration(config).await {
        Ok(_) => {
            info!(device_id = %device_id, config_id = %response.config_id, "camera configuration saved");
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!(device_id = %device_id, error = %e, "failed to save configuration");
            // Return the response even if saving failed
            (StatusCode::OK, Json(response)).into_response()
        }
    }
}

/// Get current camera configuration (from device)
async fn get_current_configuration(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    info!(device_id = %device_id, "getting current camera configuration");

    let imaging_client = match get_device_and_create_imaging_client(&state, &device_id).await {
        Ok(client) => client,
        Err(response) => return response,
    };

    match imaging_client.get_camera_configuration().await {
        Ok(config) => (StatusCode::OK, Json(config)).into_response(),
        Err(e) => {
            error!(device_id = %device_id, error = %e, "failed to get camera configuration");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Get configuration history for a device
async fn get_configuration_history(
    State(state): State<DeviceManagerState>,
    Path(device_id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    info!(device_id = %device_id, "getting configuration history");

    let status = params.get("status").and_then(|s| {
        serde_json::from_value::<ConfigurationStatus>(serde_json::json!(s)).ok()
    });

    let limit = params
        .get("limit")
        .and_then(|l| l.parse::<i64>().ok());

    let offset = params
        .get("offset")
        .and_then(|o| o.parse::<i64>().ok());

    let query = ConfigurationHistoryQuery {
        device_id: device_id.clone(),
        status,
        limit,
        offset,
    };

    match state.store.list_device_configuration_history(query).await {
        Ok(history) => (StatusCode::OK, Json(history)).into_response(),
        Err(e) => {
            error!(device_id = %device_id, error = %e, "failed to get configuration history");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Get specific configuration by ID
async fn get_configuration_by_id(
    State(state): State<DeviceManagerState>,
    Path((device_id, config_id)): Path<(String, String)>,
) -> impl IntoResponse {
    info!(device_id = %device_id, config_id = %config_id, "getting configuration by id");

    match state.store.get_device_configuration(&config_id).await {
        Ok(config) => {
            if config.device_id != device_id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "configuration not found for this device"})),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(config)).into_response()
        }
        Err(e) => {
            error!(device_id = %device_id, config_id = %config_id, error = %e, "failed to get configuration");
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "configuration not found"})),
            )
                .into_response()
        }
    }
}
