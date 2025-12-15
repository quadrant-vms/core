use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let mut url = format!("{}/tasks", state.config.ai_service_url);

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
                Ok(tasks) => Ok(Json(tasks)),
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
                Json(serde_json::json!({"error": "AI service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "AI service unavailable"})),
        )),
    }
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/tasks/{}", state.config.ai_service_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(task) => Ok(Json(task)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Task not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "AI service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "AI service unavailable"})),
        )),
    }
}

pub async fn list_detections(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let mut url = format!("{}/detections", state.config.ai_service_url);

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
                Ok(detections) => Ok(Json(detections)),
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
                Json(serde_json::json!({"error": "AI service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "AI service unavailable"})),
        )),
    }
}
