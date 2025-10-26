use axum::{extract::Query, response::IntoResponse, Json};
use axum::http::StatusCode;
use tracing::info;

use crate::stream::{self, Codec, Container};
use super::{StartQuery, StopQuery, StreamDto};

pub async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

pub async fn list_streams() -> impl IntoResponse {
    let list = stream::list_streams().await;
    let out: Vec<StreamDto> = list.into_iter().map(|s| StreamDto {
        id: s.id,
        uri: s.uri,
        codec: s.codec,
        container: s.container,
        running: s.running,
        playlist: s.playlist.to_string_lossy().to_string(),
        output_dir: s.output_dir.to_string_lossy().to_string(),
    }).collect();
    (StatusCode::OK, Json(out))
}

pub async fn start_stream_api(Query(q): Query<StartQuery>) -> impl IntoResponse {
    let codec = match q.codec.to_lowercase().as_str() {
        "h265" | "hevc" | "h265+" => Codec::H265,
        _ => Codec::H264,
    };
    let container = match q.container.to_lowercase().as_str() {
        "fmp4" | "mp4" => Container::Fmp4,
        _ => Container::Ts,
    };
    let spec = stream::StreamSpec {
        id: q.id.clone(),
        uri: q.uri.clone(),
        codec,
        container,
    };

    match stream::start_stream(&spec).await {
        Ok(_) => {
            info!(id=%q.id, "stream started");
            (StatusCode::OK, "started".to_string())
        }
        Err(e) => {
            tracing::error!(?e, "start failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("error: {e}"))
        }
    }
}

pub async fn stop_stream_api(Query(q): Query<StopQuery>) -> impl IntoResponse {
    match stream::stop_stream(&q.id).await {
        Ok(_) => {
            info!(id=%q.id, "stream stopped");
            (StatusCode::OK, "stopped".to_string())
        }
        Err(e) => {
            tracing::error!(?e, "stop failed");
            (StatusCode::NOT_FOUND, format!("error: {e}"))
        }
    }
}
