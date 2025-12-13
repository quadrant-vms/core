pub mod routes;

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;

use crate::playback::PlaybackManager;
use routes::*;

pub fn create_router(manager: Arc<PlaybackManager>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/playback/start", post(start_playback))
        .route("/v1/playback/stop", post(stop_playback))
        .route("/v1/playback/seek", post(seek_playback))
        .route("/v1/playback/control", post(control_playback))
        .route("/v1/playback/sessions", get(list_playback_sessions))
        .route("/ll-hls/streams/:stream_id/playlist.m3u8", get(serve_ll_hls_playlist))
        .with_state(manager)
}
