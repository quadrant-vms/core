use axum::{http::StatusCode, Json};
use common::recordings::*;
use tracing::info;

use crate::recording::manager::RECORDING_MANAGER;

pub async fn healthz() -> &'static str {
  "ok"
}

pub async fn list_recordings() -> Json<RecordingListResponse> {
  let recordings = RECORDING_MANAGER.list().await;
  Json(RecordingListResponse { recordings })
}

pub async fn start_recording(
  Json(req): Json<RecordingStartRequest>,
) -> Result<Json<RecordingStartResponse>, StatusCode> {
  info!(id = %req.config.id, "start recording request");

  match RECORDING_MANAGER.start(req).await {
    Ok(response) => Ok(Json(response)),
    Err(e) => {
      tracing::error!("failed to start recording: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

pub async fn stop_recording(
  Json(req): Json<RecordingStopRequest>,
) -> Result<Json<RecordingStopResponse>, StatusCode> {
  info!(id = %req.id, "stop recording request");

  match RECORDING_MANAGER.stop(&req.id).await {
    Ok(stopped) => Ok(Json(RecordingStopResponse {
      stopped,
      message: None,
    })),
    Err(e) => {
      tracing::error!("failed to stop recording: {}", e);
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}
