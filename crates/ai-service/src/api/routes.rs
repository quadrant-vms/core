use crate::state::AiServiceState;
use crate::plugin::facial_recognition::FacialRecognitionPlugin;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use common::ai_tasks::{
    AiTaskStartRequest, AiTaskStartResponse, AiTaskStopResponse, PluginListResponse,
    VideoFrame,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Start a new AI task
pub async fn start_task(
    State(state): State<AiServiceState>,
    Json(request): Json<AiTaskStartRequest>,
) -> impl IntoResponse {
    match state
        .start_task(request.config, request.lease_ttl_secs)
        .await
    {
        Ok(task_id) => {
            let response = AiTaskStartResponse {
                accepted: true,
                lease_id: Some(task_id.clone()),
                message: Some(format!("AI task '{}' started successfully", task_id)),
            };
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to start AI task: {}", e);
            let response = AiTaskStartResponse {
                accepted: false,
                lease_id: None,
                message: Some(format!("Failed to start task: {}", e)),
            };
            (StatusCode::BAD_REQUEST, Json(response))
        }
    }
}

/// Stop an AI task
pub async fn stop_task(
    State(state): State<AiServiceState>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    match state.stop_task(&task_id).await {
        Ok(_) => {
            let response = AiTaskStopResponse {
                success: true,
                message: Some(format!("AI task '{}' stopped successfully", task_id)),
            };
            (StatusCode::OK, Json(response))
        }
        Err(e) => {
            tracing::error!("Failed to stop AI task {}: {}", task_id, e);
            let response = AiTaskStopResponse {
                success: false,
                message: Some(format!("Failed to stop task: {}", e)),
            };
            (StatusCode::NOT_FOUND, Json(response))
        }
    }
}

/// Get information about a specific task
pub async fn get_task(
    State(state): State<AiServiceState>,
    Path(task_id): Path<String>,
) -> impl IntoResponse {
    match state.get_task(&task_id).await {
        Some(task_info) => (StatusCode::OK, Json(task_info)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("Task '{}' not found", task_id)
            })),
        )
            .into_response(),
    }
}

/// List all AI tasks
pub async fn list_tasks(State(state): State<AiServiceState>) -> impl IntoResponse {
    let tasks = state.list_tasks().await;
    (StatusCode::OK, Json(json!({ "tasks": tasks })))
}

/// List all available plugins
pub async fn list_plugins(State(state): State<AiServiceState>) -> impl IntoResponse {
    let plugins = state.plugins().list().await;
    let response = PluginListResponse { plugins };
    (StatusCode::OK, Json(response))
}

/// Get information about a specific plugin
pub async fn get_plugin(
    State(state): State<AiServiceState>,
    Path(plugin_id): Path<String>,
) -> impl IntoResponse {
    match state.plugins().get(&plugin_id).await {
        Ok(plugin) => {
            let plugin_read = plugin.read().await;
            let info = plugin_read.info();
            drop(plugin_read);
            (StatusCode::OK, Json(info)).into_response()
        }
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "error": format!("Plugin '{}' not found: {}", plugin_id, e)
            })),
        )
            .into_response(),
    }
}

/// Health check endpoint
pub async fn healthz() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "healthy",
            "service": "ai-service"
        })),
    )
}

/// Readiness check endpoint
pub async fn readyz(State(state): State<AiServiceState>) -> impl IntoResponse {
    // Check if plugins are healthy
    let plugin_health = state.plugins().health_check_all().await;
    let all_healthy = plugin_health.values().all(|&h| h);

    if all_healthy {
        (
            StatusCode::OK,
            Json(json!({
                "status": "ready",
                "plugins": plugin_health
            })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "status": "not ready",
                "plugins": plugin_health
            })),
        )
    }
}

/// Submit a video frame for processing by a specific task
pub async fn submit_frame(
    State(state): State<AiServiceState>,
    Path(task_id): Path<String>,
    Json(frame): Json<VideoFrame>,
) -> impl IntoResponse {
    match state.process_frame(&task_id, frame).await {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => {
            tracing::error!("Failed to process frame for task {}: {}", task_id, e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Failed to process frame: {}", e)
                })),
            )
                .into_response()
        }
    }
}

/// Metrics endpoint (Prometheus format)
pub async fn metrics() -> impl IntoResponse {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = telemetry::metrics::REGISTRY.gather();
    let mut buffer = Vec::new();

    if let Err(e) = encoder.encode(&metric_families, &mut buffer) {
        tracing::error!("Failed to encode metrics: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to encode metrics",
        )
            .into_response();
    }

    match String::from_utf8(buffer) {
        Ok(s) => s.into_response(),
        Err(e) => {
            tracing::error!("Failed to convert metrics to UTF-8: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to convert metrics",
            )
                .into_response()
        }
    }
}

// ============================================================================
// Facial Recognition Endpoints
// ============================================================================

/// Request to enroll a new face
#[derive(Debug, Serialize, Deserialize)]
pub struct EnrollFaceRequest {
    pub face_id: String,
    pub name: String,
    pub image_data: String, // Base64 encoded image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Response for face enrollment
#[derive(Debug, Serialize, Deserialize)]
pub struct EnrollFaceResponse {
    pub success: bool,
    pub face_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Enroll a new face into the facial recognition database
pub async fn enroll_face(
    State(state): State<AiServiceState>,
    Json(request): Json<EnrollFaceRequest>,
) -> impl IntoResponse {
    // Get the facial recognition plugin
    let plugin_result = state.plugins().get("facial_recognition").await;

    let plugin = match plugin_result {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(EnrollFaceResponse {
                    success: false,
                    face_id: request.face_id,
                    message: Some(format!("Facial recognition plugin not available: {}", e)),
                }),
            )
                .into_response();
        }
    };

    // Decode base64 image
    let image_data = match base64::Engine::decode(&base64::prelude::BASE64_STANDARD, &request.image_data) {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(EnrollFaceResponse {
                    success: false,
                    face_id: request.face_id,
                    message: Some(format!("Invalid base64 image data: {}", e)),
                }),
            )
                .into_response();
        }
    };

    let img = match image::load_from_memory(&image_data) {
        Ok(img) => img,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(EnrollFaceResponse {
                    success: false,
                    face_id: request.face_id,
                    message: Some(format!("Invalid image format: {}", e)),
                }),
            )
                .into_response();
        }
    };

    // Downcast to FacialRecognitionPlugin
    let mut plugin_write = plugin.write().await;
    let face_plugin = match plugin_write.as_any_mut().downcast_mut::<FacialRecognitionPlugin>() {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(EnrollFaceResponse {
                    success: false,
                    face_id: request.face_id,
                    message: Some("Failed to access facial recognition plugin".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Enroll the face
    match face_plugin.enroll_face(request.face_id.clone(), request.name, &img, request.metadata).await {
        Ok(_) => (
            StatusCode::OK,
            Json(EnrollFaceResponse {
                success: true,
                face_id: request.face_id,
                message: Some("Face enrolled successfully".to_string()),
            }),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(EnrollFaceResponse {
                success: false,
                face_id: request.face_id,
                message: Some(format!("Failed to enroll face: {}", e)),
            }),
        )
            .into_response(),
    }
}

/// Remove a face from the facial recognition database
pub async fn remove_face(
    State(state): State<AiServiceState>,
    Path(face_id): Path<String>,
) -> impl IntoResponse {
    let plugin_result = state.plugins().get("facial_recognition").await;

    let plugin = match plugin_result {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "message": format!("Facial recognition plugin not available: {}", e)
                })),
            )
                .into_response();
        }
    };

    let plugin_write = plugin.write().await;
    let face_plugin = match plugin_write.as_any().downcast_ref::<FacialRecognitionPlugin>() {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "message": "Failed to access facial recognition plugin"
                })),
            )
                .into_response();
        }
    };

    match face_plugin.remove_face(&face_id) {
        Ok(removed) => (
            StatusCode::OK,
            Json(json!({
                "success": removed,
                "message": if removed {
                    "Face removed successfully"
                } else {
                    "Face not found"
                }
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Failed to remove face: {}", e)
            })),
        )
            .into_response(),
    }
}

/// List all enrolled faces
pub async fn list_faces(State(state): State<AiServiceState>) -> impl IntoResponse {
    let plugin_result = state.plugins().get("facial_recognition").await;

    let plugin = match plugin_result {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": format!("Facial recognition plugin not available: {}", e)
                })),
            )
                .into_response();
        }
    };

    let plugin_read = plugin.read().await;
    let face_plugin = match plugin_read.as_any().downcast_ref::<FacialRecognitionPlugin>() {
        Some(p) => p,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to access facial recognition plugin"
                })),
            )
                .into_response();
        }
    };

    match face_plugin.list_faces() {
        Ok(faces) => (
            StatusCode::OK,
            Json(json!({
                "faces": faces,
                "count": faces.len()
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to list faces: {}", e)
            })),
        )
            .into_response(),
    }
}
