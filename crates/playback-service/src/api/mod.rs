pub mod routes;
pub mod webrtc_routes;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use crate::cache::EdgeCache;
use crate::playback::PlaybackManager;
use crate::webrtc::{WebRtcPeerManager, WhepHandler};
use routes::*;

pub fn create_router(manager: Arc<PlaybackManager>, cache: Arc<EdgeCache>) -> Router {
    // Create WebRTC peer manager and WHEP handler
    let peer_manager = Arc::new(WebRtcPeerManager::new());
    let whep_handler = Arc::new(WhepHandler::new(peer_manager.clone()));

    // Create app state tuple for WebRTC routes
    let webrtc_state = (manager.clone(), whep_handler);

    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/v1/playback/start", post(start_playback))
        .route("/v1/playback/stop", post(stop_playback))
        .route("/v1/playback/seek", post(seek_playback))
        .route("/v1/playback/control", post(control_playback))
        .route("/v1/playback/sessions", get(list_playback_sessions))
        .route("/ll-hls/streams/:stream_id/playlist.m3u8", get(serve_ll_hls_playlist))
        // DVR endpoints
        .route("/v1/dvr/window", post(get_dvr_window))
        .route("/v1/dvr/seek", post(dvr_seek))
        .route("/v1/dvr/jump_to_live", post(jump_to_live))
        // Time-axis preview endpoint
        .route("/v1/preview/time_axis", post(get_time_axis_preview))
        .with_state(manager)
        // WebRTC WHEP endpoints (with separate state)
        .nest("/whep",
            Router::new()
                .route("/stream/:stream_id", post(webrtc_routes::whep_stream))
                .route("/recording/:recording_id", post(webrtc_routes::whep_recording))
                .route("/session/:session_id", delete(webrtc_routes::whep_delete_session))
                .with_state(webrtc_state)
        )
        // Cache metrics endpoint
        .route("/metrics/cache", get(crate::cache::cache_metrics))
        .with_state(cache)
}
