use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::state::AppState;

pub async fn list_streams(
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let url = format!("{}/streams", state.config.admin_gateway_url);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Vec<Value>>().await {
                Ok(streams) => Ok(Json(streams)),
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
                Json(serde_json::json!({"error": "Admin gateway error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Admin gateway unavailable"})),
        )),
    }
}

pub async fn get_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/streams/{}", state.config.admin_gateway_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(stream) => Ok(Json(stream)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Stream not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Admin gateway error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Admin gateway unavailable"})),
        )),
    }
}

pub async fn stop_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/streams/{}/stop", state.config.admin_gateway_url, id);

    match state.http_client.post(&url).send().await {
        Ok(response) if response.status().is_success() => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Stream stopped"
        }))),
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Stream not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Failed to stop stream"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Admin gateway unavailable"})),
        )),
    }
}
