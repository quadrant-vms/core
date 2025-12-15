use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

use crate::state::AppState;

pub async fn list_alerts(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let mut url = format!("{}/alerts", state.config.alert_service_url);

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
                Ok(alerts) => Ok(Json(alerts)),
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
                Json(serde_json::json!({"error": "Alert service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}

pub async fn get_alert(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/alerts/{}", state.config.alert_service_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(alert) => Ok(Json(alert)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Alert not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Alert service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}

pub async fn list_rules(
    State(state): State<AppState>,
) -> Result<Json<Vec<Value>>, (StatusCode, Json<Value>)> {
    let url = format!("{}/rules", state.config.alert_service_url);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Vec<Value>>().await {
                Ok(rules) => Ok(Json(rules)),
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
                Json(serde_json::json!({"error": "Alert service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}

pub async fn get_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/rules/{}", state.config.alert_service_url, id);

    match state.http_client.get(&url).send().await {
        Ok(response) if response.status().is_success() => {
            match response.json::<Value>().await {
                Ok(rule) => Ok(Json(rule)),
                Err(_) => Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": "Failed to parse response"})),
                )),
            }
        }
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Rule not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Alert service error"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}

pub async fn enable_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/rules/{}/enable", state.config.alert_service_url, id);

    match state.http_client.post(&url).send().await {
        Ok(response) if response.status().is_success() => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Rule enabled"
        }))),
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Rule not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Failed to enable rule"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}

pub async fn disable_rule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let url = format!("{}/rules/{}/disable", state.config.alert_service_url, id);

    match state.http_client.post(&url).send().await {
        Ok(response) if response.status().is_success() => Ok(Json(serde_json::json!({
            "success": true,
            "message": "Rule disabled"
        }))),
        Ok(response) if response.status() == StatusCode::NOT_FOUND => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Rule not found"})),
        )),
        Ok(response) => {
            let status = response.status();
            Err((
                status,
                Json(serde_json::json!({"error": "Failed to disable rule"})),
            ))
        }
        Err(_) => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "Alert service unavailable"})),
        )),
    }
}
