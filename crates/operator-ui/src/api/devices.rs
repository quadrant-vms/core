use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;

use crate::state::AppState;

pub async fn list_devices(
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let url = format!("{}/devices", state.config.device_manager_url);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Vec<Value>>().await {
                Ok(devices) => Ok(Json(devices)),
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
                Json(serde_json::json!({"error": "Device manager error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Device manager unavailable"})),
        )),
    }
}

pub async fn get_device(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/devices/{}", state.config.device_manager_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(device) => Ok(Json(device)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Device not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Device manager error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Device manager unavailable"})),
        )),
    }
}

pub async fn get_device_health(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/devices/{}/health", state.config.device_manager_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(health) => Ok(Json(health)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Device not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Device manager error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Device manager unavailable"})),
        )),
    }
}
