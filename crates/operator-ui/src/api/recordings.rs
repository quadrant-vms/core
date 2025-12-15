use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct SearchRequest {
    pub query: Option<String>,
    pub filters: HashMap<String, String>,
}

pub async fn list_recordings(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let mut url = format!("{}/recordings", state.config.recorder_node_url);

    if !params.is_empty() {
        let query_string: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        url.push('?');
        url.push_str(&query_string.join("&"));
    }

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Vec<Value>>().await {
                Ok(recordings) => Ok(Json(recordings)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Recorder node error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Recorder node unavailable"})),
        )),
    }
}

pub async fn search_recordings(
    State(state): State<AppState>,
    Json(search_req): Json<SearchRequest>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let url = format!("{}/recordings/search", state.config.recorder_node_url);

    match state
        .http_client
        .post(&url)
        .json(&search_req)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<Vec<Value>>().await {
                Ok(recordings) => Ok(Json(recordings)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Search failed"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Recorder node unavailable"})),
        )),
    }
}

pub async fn get_recording(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/recordings/{}", state.config.recorder_node_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(recording) => Ok(Json(recording)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Recording not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Recorder node error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Recorder node unavailable"})),
        )),
    }
}

pub async fn get_thumbnail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/recordings/{}/thumbnail", state.config.recorder_node_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(thumbnail) => Ok(Json(thumbnail)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Thumbnail not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Recorder node error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Recorder node unavailable"})),
        )),
    }
}
