use crate::{error::ApiError, state::CoordinatorState};
use axum::{
    extract::{Path, Query, State},
    Json, Router,
    routing::{delete, get, post, put},
};
use common::{
    ai_tasks::AiTaskInfo,
    recordings::RecordingInfo,
    state_store::StateStore,
    streams::StreamInfo,
};
use serde::Deserialize;

pub fn state_router() -> Router<CoordinatorState> {
    Router::new()
        // Stream state endpoints
        .route("/v1/state/streams", post(save_stream))
        .route("/v1/state/streams", get(list_streams))
        .route("/v1/state/streams/:stream_id", get(get_stream))
        .route("/v1/state/streams/:stream_id", delete(delete_stream))
        .route("/v1/state/streams/:stream_id/state", put(update_stream_state))
        // Recording state endpoints
        .route("/v1/state/recordings", post(save_recording))
        .route("/v1/state/recordings", get(list_recordings))
        .route("/v1/state/recordings/:recording_id", get(get_recording))
        .route("/v1/state/recordings/:recording_id", delete(delete_recording))
        .route("/v1/state/recordings/:recording_id/state", put(update_recording_state))
        // AI task state endpoints
        .route("/v1/state/ai-tasks", post(save_ai_task))
        .route("/v1/state/ai-tasks", get(list_ai_tasks))
        .route("/v1/state/ai-tasks/:task_id", get(get_ai_task))
        .route("/v1/state/ai-tasks/:task_id", delete(delete_ai_task))
        .route("/v1/state/ai-tasks/:task_id/state", put(update_ai_task_state))
        .route("/v1/state/ai-tasks/:task_id/stats", put(update_ai_task_stats))
}

// Helper to get state store or return error
fn get_state_store(state: &CoordinatorState) -> Result<std::sync::Arc<dyn StateStore>, ApiError> {
    state
        .state_store()
        .ok_or_else(|| ApiError::bad_request("StateStore not configured (use LEASE_STORE_TYPE=postgres)"))
}

// ========== Stream endpoints ==========

async fn save_stream(
    State(state): State<CoordinatorState>,
    Json(info): Json<StreamInfo>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .save_stream(&info)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to save stream: {}", e)))?;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct NodeIdQuery {
    node_id: Option<String>,
}

async fn list_streams(
    State(state): State<CoordinatorState>,
    Query(query): Query<NodeIdQuery>,
) -> Result<Json<Vec<StreamInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let streams = store
        .list_streams(query.node_id.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to list streams: {}", e)))?;
    Ok(Json(streams))
}

async fn get_stream(
    State(state): State<CoordinatorState>,
    Path(stream_id): Path<String>,
) -> Result<Json<Option<StreamInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let stream = store
        .get_stream(&stream_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get stream: {}", e)))?;
    Ok(Json(stream))
}

async fn delete_stream(
    State(state): State<CoordinatorState>,
    Path(stream_id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .delete_stream(&stream_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to delete stream: {}", e)))?;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct UpdateStateRequest {
    state: String,
    error: Option<String>,
}

async fn update_stream_state(
    State(state): State<CoordinatorState>,
    Path(stream_id): Path<String>,
    Json(req): Json<UpdateStateRequest>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .update_stream_state(&stream_id, &req.state, req.error.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update stream state: {}", e)))?;
    Ok(Json(()))
}

// ========== Recording endpoints ==========

async fn save_recording(
    State(state): State<CoordinatorState>,
    Json(info): Json<RecordingInfo>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .save_recording(&info)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to save recording: {}", e)))?;
    Ok(Json(()))
}

async fn list_recordings(
    State(state): State<CoordinatorState>,
    Query(query): Query<NodeIdQuery>,
) -> Result<Json<Vec<RecordingInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let recordings = store
        .list_recordings(query.node_id.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to list recordings: {}", e)))?;
    Ok(Json(recordings))
}

async fn get_recording(
    State(state): State<CoordinatorState>,
    Path(recording_id): Path<String>,
) -> Result<Json<Option<RecordingInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let recording = store
        .get_recording(&recording_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get recording: {}", e)))?;
    Ok(Json(recording))
}

async fn delete_recording(
    State(state): State<CoordinatorState>,
    Path(recording_id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .delete_recording(&recording_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to delete recording: {}", e)))?;
    Ok(Json(()))
}

async fn update_recording_state(
    State(state): State<CoordinatorState>,
    Path(recording_id): Path<String>,
    Json(req): Json<UpdateStateRequest>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .update_recording_state(&recording_id, &req.state, req.error.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update recording state: {}", e)))?;
    Ok(Json(()))
}

// ========== AI Task endpoints ==========

async fn save_ai_task(
    State(state): State<CoordinatorState>,
    Json(info): Json<AiTaskInfo>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .save_ai_task(&info)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to save AI task: {}", e)))?;
    Ok(Json(()))
}

async fn list_ai_tasks(
    State(state): State<CoordinatorState>,
    Query(query): Query<NodeIdQuery>,
) -> Result<Json<Vec<AiTaskInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let tasks = store
        .list_ai_tasks(query.node_id.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to list AI tasks: {}", e)))?;
    Ok(Json(tasks))
}

async fn get_ai_task(
    State(state): State<CoordinatorState>,
    Path(task_id): Path<String>,
) -> Result<Json<Option<AiTaskInfo>>, ApiError> {
    let store = get_state_store(&state)?;
    let task = store
        .get_ai_task(&task_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get AI task: {}", e)))?;
    Ok(Json(task))
}

async fn delete_ai_task(
    State(state): State<CoordinatorState>,
    Path(task_id): Path<String>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .delete_ai_task(&task_id)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to delete AI task: {}", e)))?;
    Ok(Json(()))
}

async fn update_ai_task_state(
    State(state): State<CoordinatorState>,
    Path(task_id): Path<String>,
    Json(req): Json<UpdateStateRequest>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .update_ai_task_state(&task_id, &req.state, req.error.as_deref())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update AI task state: {}", e)))?;
    Ok(Json(()))
}

#[derive(Deserialize)]
struct UpdateStatsRequest {
    frames_delta: u64,
    detections_delta: u64,
}

async fn update_ai_task_stats(
    State(state): State<CoordinatorState>,
    Path(task_id): Path<String>,
    Json(req): Json<UpdateStatsRequest>,
) -> Result<Json<()>, ApiError> {
    let store = get_state_store(&state)?;
    store
        .update_ai_task_stats(&task_id, req.frames_delta, req.detections_delta)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update AI task stats: {}", e)))?;
    Ok(Json(()))
}
