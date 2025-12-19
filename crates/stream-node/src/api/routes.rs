use axum::http::StatusCode;
use axum::{extract::Query, response::IntoResponse, Json};
use tracing::info;

use super::{StartQuery, StartRequest, StopQuery, StopRequest, StreamDto};
use crate::stream::{self, Codec, Container};
use common::validation;

pub async fn healthz() -> impl IntoResponse {
  (StatusCode::OK, "ok")
}

pub async fn readyz() -> impl IntoResponse {
  (StatusCode::OK, "ready")
}

pub async fn list_streams() -> impl IntoResponse {
  let list = stream::list_streams().await;
  let out: Vec<StreamDto> = list
    .into_iter()
    .map(|s| StreamDto {
      id: s.id,
      uri: s.uri,
      codec: s.codec,
      container: s.container,
      running: s.running,
      playlist: s.playlist.to_string_lossy().to_string(),
      output_dir: s.output_dir.to_string_lossy().to_string(),
    })
    .collect();
  (StatusCode::OK, Json(out))
}

/// POST /start - Start a stream (recommended)
pub async fn start_stream(Json(req): Json<StartRequest>) -> impl IntoResponse {
  // Validate inputs
  if let Err(e) = validation::validate_id(&req.id, "stream_id") {
    return (StatusCode::BAD_REQUEST, format!("invalid stream_id: {e}"));
  }
  if let Err(e) = validation::validate_uri(&req.uri, "source_uri") {
    return (StatusCode::BAD_REQUEST, format!("invalid source_uri: {e}"));
  }

  let codec = match req.codec.to_lowercase().as_str() {
    "h265" | "hevc" | "h265+" => Codec::H265,
    _ => Codec::H264,
  };
  let container = match req.container.to_lowercase().as_str() {
    "fmp4" | "mp4" => Container::Fmp4,
    _ => Container::Ts,
  };
  let spec = stream::StreamSpec {
    id: req.id.clone(),
    uri: req.uri.clone(),
    codec,
    container,
  };

  match stream::start_stream(&spec).await {
    Ok(_) => {
      info!(id=%req.id, "stream started");
      (StatusCode::OK, "started".to_string())
    }
    Err(e) => {
      tracing::error!(?e, "start failed");
      (StatusCode::INTERNAL_SERVER_ERROR, format!("error: {e}"))
    }
  }
}

/// GET /start (deprecated, use POST /start)
pub async fn start_stream_api(Query(q): Query<StartQuery>) -> impl IntoResponse {
  // Validate inputs
  if let Err(e) = validation::validate_id(&q.id, "stream_id") {
    return (StatusCode::BAD_REQUEST, format!("invalid stream_id: {e}"));
  }
  if let Err(e) = validation::validate_uri(&q.uri, "source_uri") {
    return (StatusCode::BAD_REQUEST, format!("invalid source_uri: {e}"));
  }

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

/// DELETE /stop - Stop a stream (recommended)
pub async fn stop_stream(Json(req): Json<StopRequest>) -> impl IntoResponse {
  // Validate input
  if let Err(e) = validation::validate_id(&req.id, "stream_id") {
    return (StatusCode::BAD_REQUEST, format!("invalid stream_id: {e}"));
  }

  match stream::stop_stream(&req.id).await {
    Ok(_) => {
      info!(id=%req.id, "stream stopped");
      (StatusCode::OK, "stopped".to_string())
    }
    Err(e) => {
      tracing::error!(?e, "stop failed");
      (StatusCode::NOT_FOUND, format!("error: {e}"))
    }
  }
}

/// GET /stop (deprecated, use DELETE /stop)
pub async fn stop_stream_api(Query(q): Query<StopQuery>) -> impl IntoResponse {
  // Validate input
  if let Err(e) = validation::validate_id(&q.id, "stream_id") {
    return (StatusCode::BAD_REQUEST, format!("invalid stream_id: {e}"));
  }

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
