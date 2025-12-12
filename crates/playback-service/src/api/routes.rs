use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use common::playback::*;
use std::sync::Arc;
use tracing::{error, info};

use crate::playback::PlaybackManager;

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn start_playback(
    State(manager): State<Arc<PlaybackManager>>,
    Json(req): Json<PlaybackStartRequest>,
) -> Result<Json<PlaybackStartResponse>, StatusCode> {
    info!(session_id = %req.config.session_id, source = %req.config.source_id, "start playback request");

    match manager.start(req.config.clone()).await {
        Ok(info) => Ok(Json(PlaybackStartResponse {
            accepted: true,
            session_id: info.config.session_id,
            lease_id: info.lease_id,
            playback_url: info.playback_url,
            message: Some("Playback session started".to_string()),
        })),
        Err(e) => {
            error!("failed to start playback: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn stop_playback(
    State(manager): State<Arc<PlaybackManager>>,
    Json(req): Json<PlaybackStopRequest>,
) -> Result<Json<PlaybackStopResponse>, StatusCode> {
    info!(session_id = %req.session_id, "stop playback request");

    match manager.stop(&req.session_id).await {
        Ok(stopped) => Ok(Json(PlaybackStopResponse {
            stopped,
            message: if stopped { Some("Playback session stopped".to_string()) } else { Some("Session not found".to_string()) },
        })),
        Err(e) => {
            error!("failed to stop playback: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn seek_playback(
    State(manager): State<Arc<PlaybackManager>>,
    Json(req): Json<PlaybackSeekRequest>,
) -> Result<Json<PlaybackSeekResponse>, StatusCode> {
    info!(session_id = %req.session_id, position = %req.position_secs, "seek playback request");

    match manager.seek(&req.session_id, req.position_secs).await {
        Ok(position) => Ok(Json(PlaybackSeekResponse {
            success: true,
            current_position_secs: Some(position),
            message: Some("Seek successful".to_string()),
        })),
        Err(e) => {
            error!("failed to seek playback: {}", e);
            Ok(Json(PlaybackSeekResponse {
                success: false,
                current_position_secs: None,
                message: Some(format!("Seek failed: {}", e)),
            }))
        }
    }
}

pub async fn control_playback(
    State(manager): State<Arc<PlaybackManager>>,
    Json(req): Json<PlaybackControlRequest>,
) -> Result<Json<PlaybackControlResponse>, StatusCode> {
    info!(session_id = %req.session_id, action = ?req.action, "control playback request");

    let result = match req.action {
        PlaybackAction::Pause => manager.pause(&req.session_id).await,
        PlaybackAction::Resume => manager.resume(&req.session_id).await,
        PlaybackAction::Stop => manager.stop(&req.session_id).await.map(|_| ()),
    };

    match result {
        Ok(_) => Ok(Json(PlaybackControlResponse {
            success: true,
            message: Some(format!("Action {:?} successful", req.action)),
        })),
        Err(e) => {
            error!("failed to control playback: {}", e);
            Ok(Json(PlaybackControlResponse {
                success: false,
                message: Some(format!("Action failed: {}", e)),
            }))
        }
    }
}

pub async fn list_playback_sessions(
    State(manager): State<Arc<PlaybackManager>>,
) -> Json<PlaybackListResponse> {
    let sessions = manager.list().await;
    Json(PlaybackListResponse { sessions })
}
