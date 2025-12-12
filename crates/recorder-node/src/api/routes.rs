use axum::{
    extract::Query,
    http::StatusCode,
    Json,
};
use common::recordings::*;
use serde::Deserialize;
use std::path::PathBuf;
use tracing::{error, info};

use crate::recording::manager::RECORDING_MANAGER;
use crate::recording::thumbnail_generator::{
    find_recording_path, generate_recording_thumbnail, generate_recording_thumbnail_grid,
    ThumbnailConfig,
};

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

// Thumbnail generation endpoints
#[derive(Debug, Deserialize)]
pub struct ThumbnailQueryParams {
    recording_id: String,
    timestamp_secs: Option<f64>,
    width: Option<u32>,
    height: Option<u32>,
    quality: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ThumbnailGridQueryParams {
    recording_id: String,
    count: Option<u32>,
    width: Option<u32>,
    height: Option<u32>,
    quality: Option<u32>,
}

pub async fn get_thumbnail(
    Query(params): Query<ThumbnailQueryParams>,
) -> Result<Json<ThumbnailInfo>, StatusCode> {
    info!(
        recording_id = %params.recording_id,
        timestamp = ?params.timestamp_secs,
        "thumbnail request"
    );

    // Get storage root from environment or use default
    let storage_root = std::env::var("RECORDING_STORAGE_ROOT")
        .unwrap_or_else(|_| "./data/recordings".to_string());
    let storage_path = PathBuf::from(storage_root);

    // Find the recording file
    let recording_path = match find_recording_path(&storage_path, &params.recording_id) {
        Ok(path) => path,
        Err(e) => {
            error!("recording not found: {}", e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Build thumbnail config
    let config = ThumbnailConfig {
        width: params.width.unwrap_or(320),
        height: params.height.unwrap_or(180),
        quality: params.quality.unwrap_or(5),
    };

    // Generate thumbnail
    match generate_recording_thumbnail(&recording_path, params.timestamp_secs, &config) {
        Ok((timestamp, image_data)) => Ok(Json(ThumbnailInfo {
            recording_id: params.recording_id,
            timestamp_secs: timestamp,
            width: config.width,
            height: config.height,
            image_data,
        })),
        Err(e) => {
            error!("failed to generate thumbnail: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn get_thumbnail_grid(
    Query(params): Query<ThumbnailGridQueryParams>,
) -> Result<Json<Vec<ThumbnailInfo>>, StatusCode> {
    info!(
        recording_id = %params.recording_id,
        count = ?params.count,
        "thumbnail grid request"
    );

    // Get storage root from environment or use default
    let storage_root = std::env::var("RECORDING_STORAGE_ROOT")
        .unwrap_or_else(|_| "./data/recordings".to_string());
    let storage_path = PathBuf::from(storage_root);

    // Find the recording file
    let recording_path = match find_recording_path(&storage_path, &params.recording_id) {
        Ok(path) => path,
        Err(e) => {
            error!("recording not found: {}", e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Build thumbnail config
    let config = ThumbnailConfig {
        width: params.width.unwrap_or(320),
        height: params.height.unwrap_or(180),
        quality: params.quality.unwrap_or(5),
    };

    let count = params.count.unwrap_or(10);

    // Generate thumbnail grid
    match generate_recording_thumbnail_grid(&recording_path, count, &config) {
        Ok(thumbnails) => {
            let thumbnail_infos: Vec<ThumbnailInfo> = thumbnails
                .into_iter()
                .map(|(timestamp_secs, image_data)| ThumbnailInfo {
                    recording_id: params.recording_id.clone(),
                    timestamp_secs,
                    width: config.width,
                    height: config.height,
                    image_data,
                })
                .collect();
            Ok(Json(thumbnail_infos))
        }
        Err(e) => {
            error!("failed to generate thumbnail grid: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
