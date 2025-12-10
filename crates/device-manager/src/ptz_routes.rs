use crate::ptz_client::create_ptz_client;
use crate::state::DeviceManagerState;
use crate::types::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use common::auth_middleware::{AuthContext, RequireAuth};
use serde_json::json;
use tracing::{error, info};

/// PTZ control - move camera
pub async fn ptz_move(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<PtzMoveRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

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

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.move_camera(&req).await {
            Ok(_) => {
                info!(device_id = %device_id, direction = ?req.direction, "PTZ move command sent");
                (StatusCode::OK, Json(json!({"status": "ok"}))).into_response()
            }
            Err(e) => {
                error!("PTZ move failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response()
            }
        },
        Err(e) => {
            error!("failed to create PTZ client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// PTZ control - stop
pub async fn ptz_stop(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<PtzStopRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.stop(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => {
                error!("PTZ stop failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response()
            }
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// PTZ control - zoom
pub async fn ptz_zoom(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<PtzZoomRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.zoom(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// PTZ control - goto absolute position
pub async fn ptz_goto_absolute(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<PtzAbsolutePositionRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.goto_absolute_position(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// PTZ control - goto relative position
pub async fn ptz_goto_relative(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<PtzRelativePositionRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.goto_relative_position(&req).await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// PTZ control - goto home position
pub async fn ptz_goto_home(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.goto_home().await {
            Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get PTZ status
pub async fn ptz_get_status(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.get_status().await {
            Ok(status) => (StatusCode::OK, Json(status)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get PTZ capabilities
pub async fn ptz_get_capabilities(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.get_capabilities().await {
            Ok(capabilities) => (StatusCode::OK, Json(capabilities)).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// PTZ Preset handlers

/// Create PTZ preset
pub async fn create_ptz_preset(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<CreatePtzPresetRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    // Get current position
    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    let position = match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => match client.get_status().await {
            Ok(status) => status.position,
            Err(e) => {
                error!("failed to get PTZ status: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "failed to get current position"})),
                )
                    .into_response();
            }
        },
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    match state.store.create_ptz_preset(&device_id, req, position).await {
        Ok(preset) => {
            info!(preset_id = %preset.preset_id, device_id = %device_id, "PTZ preset created");
            (StatusCode::CREATED, Json(preset)).into_response()
        }
        Err(e) => {
            error!("failed to create PTZ preset: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// List PTZ presets
pub async fn list_ptz_presets(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(_) => {}
        Err(response) => return response,
    };

    match state.store.list_ptz_presets(&device_id).await {
        Ok(presets) => (StatusCode::OK, Json(presets)).into_response(),
        Err(e) => {
            error!("failed to list PTZ presets: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Get PTZ preset
pub async fn get_ptz_preset(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, preset_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.get_ptz_preset(&preset_id).await {
        Ok(Some(preset)) => (StatusCode::OK, Json(preset)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "preset not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Update PTZ preset
pub async fn update_ptz_preset(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, preset_id)): Path<(String, String)>,
    Json(req): Json<UpdatePtzPresetRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.update_ptz_preset(&preset_id, req).await {
        Ok(preset) => (StatusCode::OK, Json(preset)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete PTZ preset
pub async fn delete_ptz_preset(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, preset_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:delete") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.delete_ptz_preset(&preset_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Goto PTZ preset
pub async fn goto_ptz_preset(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((device_id, preset_id)): Path<(String, String)>,
    Json(req): Json<GotoPresetRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let device = match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(device) => device,
        Err(response) => return response,
    };

    let preset = match state.store.get_ptz_preset(&preset_id).await {
        Ok(Some(preset)) => preset,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "preset not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let username = device.username.clone();
    let password = device
        .password_encrypted
        .as_ref()
        .and_then(|enc| state.store.decrypt_password(enc).ok());

    match create_ptz_client(&device.protocol, &device.primary_uri, username, password) {
        Ok(client) => {
            let absolute_req = PtzAbsolutePositionRequest {
                pan: preset.position.pan,
                tilt: preset.position.tilt,
                zoom: preset.position.zoom,
                speed: req.speed,
            };
            match client.goto_absolute_position(&absolute_req).await {
                Ok(_) => (StatusCode::OK, Json(json!({"status": "ok"}))).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": e.to_string()})),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// PTZ Tour handlers

/// Create PTZ tour
pub async fn create_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
    Json(req): Json<CreatePtzTourRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(_) => {}
        Err(response) => return response,
    };

    match state.store.create_ptz_tour(&device_id, req).await {
        Ok(tour) => (StatusCode::CREATED, Json(tour)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// List PTZ tours
pub async fn list_ptz_tours(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match get_device_with_auth(&state, &device_id, &auth_ctx).await {
        Ok(_) => {}
        Err(response) => return response,
    };

    match state.store.list_ptz_tours(&device_id).await {
        Ok(tours) => (StatusCode::OK, Json(tours)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Get PTZ tour (with steps)
pub async fn get_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:read") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    let tour = match state.store.get_ptz_tour(&tour_id).await {
        Ok(Some(tour)) => tour,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "tour not found"})),
            )
                .into_response()
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let steps = match state.store.get_ptz_tour_steps(&tour_id).await {
        Ok(steps) => steps,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "tour": tour,
            "steps": steps
        })),
    )
        .into_response()
}

/// Update PTZ tour
pub async fn update_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
    Json(req): Json<UpdatePtzTourRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.update_ptz_tour(&tour_id, req).await {
        Ok(tour) => (StatusCode::OK, Json(tour)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete PTZ tour
pub async fn delete_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:delete") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.delete_ptz_tour(&tour_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Add step to PTZ tour
pub async fn add_ptz_tour_step(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
    Json(req): Json<AddTourStepRequest>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.add_ptz_tour_step(&tour_id, req).await {
        Ok(step) => (StatusCode::CREATED, Json(step)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete PTZ tour step
pub async fn delete_ptz_tour_step(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, _tour_id, step_id)): Path<(String, String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:delete") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.store.delete_ptz_tour_step(&step_id).await {
        Ok(_) => (StatusCode::NO_CONTENT, Json(json!({}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Start PTZ tour
pub async fn start_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    // Start tour execution
    match state.tour_executor.start_tour(tour_id.clone()).await {
        Ok(_) => {
            info!(tour_id = %tour_id, "PTZ tour started");
            (StatusCode::OK, Json(json!({"status": "started"}))).into_response()
        }
        Err(e) => {
            error!(tour_id = %tour_id, error = %e, "failed to start tour");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Stop PTZ tour
pub async fn stop_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.tour_executor.stop_tour(&tour_id).await {
        Ok(_) => {
            info!(tour_id = %tour_id, "PTZ tour stopped");
            (StatusCode::OK, Json(json!({"status": "stopped"}))).into_response()
        }
        Err(e) => {
            error!(tour_id = %tour_id, error = %e, "failed to stop tour");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Pause PTZ tour
pub async fn pause_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.tour_executor.pause_tour(&tour_id).await {
        Ok(_) => {
            info!(tour_id = %tour_id, "PTZ tour paused");
            (StatusCode::OK, Json(json!({"status": "paused"}))).into_response()
        }
        Err(e) => {
            error!(tour_id = %tour_id, error = %e, "failed to pause tour");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Resume PTZ tour
pub async fn resume_ptz_tour(
    State(state): State<DeviceManagerState>,
    RequireAuth(auth_ctx): RequireAuth,
    Path((_device_id, tour_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if !auth_ctx.has_permission("device:update") {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "permission denied"})),
        )
            .into_response();
    }

    match state.tour_executor.resume_tour(&tour_id).await {
        Ok(_) => {
            info!(tour_id = %tour_id, "PTZ tour resumed");
            (StatusCode::OK, Json(json!({"status": "resumed"}))).into_response()
        }
        Err(e) => {
            error!(tour_id = %tour_id, error = %e, "failed to resume tour");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

// Helper function to get device with auth check
async fn get_device_with_auth(
    state: &DeviceManagerState,
    device_id: &str,
    auth_ctx: &AuthContext,
) -> Result<Device, axum::response::Response> {
    match state.store.get_device(device_id).await {
        Ok(Some(device)) => {
            if !auth_ctx.is_system_admin && device.tenant_id != auth_ctx.tenant_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({"error": "access denied"})),
                )
                    .into_response());
            }
            Ok(device)
        }
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "device not found"})),
        )
            .into_response()),
        Err(e) => {
            error!("failed to get device: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
                .into_response())
        }
    }
}
