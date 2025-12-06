use crate::state::AiServiceState;
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
