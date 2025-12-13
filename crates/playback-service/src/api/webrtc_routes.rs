use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use std::sync::Arc;
use tracing::{error, info};

use crate::playback::PlaybackManager;
use crate::webrtc::{WhepHandler, WhepOffer, WhepAnswer};

type AppState = (Arc<PlaybackManager>, Arc<WhepHandler>);

/// WHEP endpoint for live streams
/// POST /api/whep/stream/{stream_id}
/// Body: { "sdp": "..." }
/// Returns: { "sdp": "...", "session_id": "...", "session_url": "..." }
pub async fn whep_stream(
    State((manager, whep)): State<AppState>,
    Path(stream_id): Path<String>,
    Json(offer): Json<WhepOffer>,
) -> Result<(StatusCode, HeaderMap, Json<WhepAnswer>), StatusCode> {
    info!(stream_id = %stream_id, "WHEP request for stream");

    // Get base URL from environment
    let base_url = std::env::var("PLAYBACK_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:8087".to_string());

    // Handle the WHEP offer
    match whep.handle_offer(&stream_id, offer, &base_url).await {
        Ok(answer) => {
            // Build Location header with session URL
            let mut headers = HeaderMap::new();
            headers.insert(
                "Location",
                answer.session_url.parse().unwrap(),
            );
            headers.insert(
                "Content-Type",
                "application/json".parse().unwrap(),
            );

            info!(stream_id = %stream_id, session_id = %answer.session_id, "WHEP session created for stream");
            Ok((StatusCode::CREATED, headers, Json(answer)))
        }
        Err(e) => {
            error!(stream_id = %stream_id, error = %e, "failed to handle WHEP offer for stream");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// WHEP endpoint for recordings
/// POST /api/whep/recording/{recording_id}
/// Body: { "sdp": "..." }
/// Returns: { "sdp": "...", "session_id": "...", "session_url": "..." }
pub async fn whep_recording(
    State((manager, whep)): State<AppState>,
    Path(recording_id): Path<String>,
    Json(offer): Json<WhepOffer>,
) -> Result<(StatusCode, HeaderMap, Json<WhepAnswer>), StatusCode> {
    info!(recording_id = %recording_id, "WHEP request for recording");

    // Get base URL from environment
    let base_url = std::env::var("PLAYBACK_SERVICE_URL")
        .unwrap_or_else(|_| "http://localhost:8087".to_string());

    // Handle the WHEP offer
    match whep.handle_offer(&recording_id, offer, &base_url).await {
        Ok(answer) => {
            // Build Location header with session URL
            let mut headers = HeaderMap::new();
            headers.insert(
                "Location",
                answer.session_url.parse().unwrap(),
            );
            headers.insert(
                "Content-Type",
                "application/json".parse().unwrap(),
            );

            info!(recording_id = %recording_id, session_id = %answer.session_id, "WHEP session created for recording");
            Ok((StatusCode::CREATED, headers, Json(answer)))
        }
        Err(e) => {
            error!(recording_id = %recording_id, error = %e, "failed to handle WHEP offer for recording");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Delete WHEP session
/// DELETE /api/whep/session/{session_id}
pub async fn whep_delete_session(
    State((manager, whep)): State<AppState>,
    Path(session_id): Path<String>,
) -> StatusCode {
    info!(session_id = %session_id, "deleting WHEP session");

    match whep.delete_session(&session_id).await {
        Ok(_) => {
            info!(session_id = %session_id, "WHEP session deleted");
            StatusCode::NO_CONTENT
        }
        Err(e) => {
            error!(session_id = %session_id, error = %e, "failed to delete WHEP session");
            StatusCode::NOT_FOUND
        }
    }
}
