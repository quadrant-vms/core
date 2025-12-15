use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::incident::{Incident, IncidentSeverity};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateIncidentRequest {
    pub title: String,
    pub description: String,
    pub severity: IncidentSeverity,
    pub source: String,
    pub device_id: Option<String>,
    pub alert_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateIncidentRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub severity: Option<IncidentSeverity>,
}

#[derive(Debug, Deserialize)]
pub struct AddNoteRequest {
    pub author: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct IncidentResponse {
    pub incident: Incident,
}

pub async fn list_incidents(
    State(state): State<AppState>,
) -> Result<Json<Vec<Incident>>, (StatusCode, Json<Value>)> {
    let store = state.incident_store.read().await;
    let incidents = store.list().into_iter().cloned().collect();
    Ok(Json(incidents))
}

pub async fn create_incident(
    State(state): State<AppState>,
    Json(req): Json<CreateIncidentRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let mut incident = Incident::new(req.title, req.description, req.severity, req.source);
    incident.device_id = req.device_id;
    incident.alert_id = req.alert_id;

    let mut store = state.incident_store.write().await;
    let created = store.create(incident);

    Ok(Json(IncidentResponse { incident: created }))
}

pub async fn get_incident(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let store = state.incident_store.read().await;

    match store.get(&id) {
        Some(incident) => Ok(Json(IncidentResponse {
            incident: incident.clone(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Incident not found"})),
        )),
    }
}

pub async fn update_incident(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIncidentRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let mut store = state.incident_store.write().await;

    match store.get_mut(&id) {
        Some(incident) => {
            if let Some(title) = req.title {
                incident.title = title;
            }
            if let Some(description) = req.description {
                incident.description = description;
            }
            if let Some(severity) = req.severity {
                incident.severity = severity;
            }
            incident.updated_at = chrono::Utc::now();

            Ok(Json(IncidentResponse {
                incident: incident.clone(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Incident not found"})),
        )),
    }
}

pub async fn acknowledge_incident(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let mut store = state.incident_store.write().await;

    match store.get_mut(&id) {
        Some(incident) => {
            incident.acknowledge("system".to_string());

            Ok(Json(IncidentResponse {
                incident: incident.clone(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Incident not found"})),
        )),
    }
}

pub async fn resolve_incident(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let mut store = state.incident_store.write().await;

    match store.get_mut(&id) {
        Some(incident) => {
            incident.resolve("system".to_string());

            Ok(Json(IncidentResponse {
                incident: incident.clone(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Incident not found"})),
        )),
    }
}

pub async fn add_note(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddNoteRequest>,
) -> Result<Json<IncidentResponse>, (StatusCode, Json<Value>)> {
    let mut store = state.incident_store.write().await;

    match store.get_mut(&id) {
        Some(incident) => {
            incident.add_note(req.author, req.content);

            Ok(Json(IncidentResponse {
                incident: incident.clone(),
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Incident not found"})),
        )),
    }
}
