// Simplified routes without auth for initial implementation
// TODO: Add proper authentication using auth-service integration

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
