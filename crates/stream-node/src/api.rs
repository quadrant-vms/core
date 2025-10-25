use axum::{Json, extract::Query, response::IntoResponse};
use serde::{Serialize, Deserialize};
use crate::rtsp::{start_stream, StreamSpec, Codec};
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use tracing::info;

#[derive(Serialize, Clone)]
pub struct StreamInfo {
    id: String,
    uri: String,
    codec: String,
    status: String,
}

lazy_static! {
    static ref ACTIVE_STREAMS: Arc<Mutex<Vec<StreamInfo>>> = Arc::new(Mutex::new(vec![]));
}

#[derive(Deserialize)]
pub struct StartQuery {
    id: String,
    uri: String,
    #[serde(default = "default_codec")]
    codec: String, // "h264" | "h265"
}
fn default_codec() -> String { "h264".into() }

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn list_streams() -> impl IntoResponse {
    Json(ACTIVE_STREAMS.lock().unwrap().clone())
}

pub async fn start_stream_api(Query(q): Query<StartQuery>) -> impl IntoResponse {
    let codec = match q.codec.to_lowercase().as_str() {
        "h265" | "hevc" | "h265+" => Codec::H265,
        _ => Codec::H264,
    };
    let spec = StreamSpec { id: q.id.clone(), uri: q.uri.clone(), codec };

    match start_stream(&spec) {
        Ok(_) => {
            info!(id = %q.id, codec = %q.codec, "stream started");
            let mut s = ACTIVE_STREAMS.lock().unwrap();
            s.push(StreamInfo {
                id: q.id, uri: q.uri, codec: q.codec, status: "running".into()
            });
            "started"
        }
        Err(e) => {
            tracing::error!(?e, "failed to start stream");
            "error"
        }
    }
}