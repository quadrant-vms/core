use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub devices: DeviceStats,
    pub streams: StreamStats,
    pub recordings: RecordingStats,
    pub ai_tasks: AiTaskStats,
    pub alerts: AlertStats,
    pub incidents: IncidentStats,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeviceStats {
    pub total: usize,
    pub online: usize,
    pub offline: usize,
    pub degraded: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StreamStats {
    pub active: usize,
    pub total: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RecordingStats {
    pub total: usize,
    pub today: usize,
    pub total_size_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AiTaskStats {
    pub active: usize,
    pub total: usize,
    pub detections_today: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertStats {
    pub active_rules: usize,
    pub alerts_today: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IncidentStats {
    pub open: usize,
    pub acknowledged: usize,
    pub total: usize,
}

pub async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<DashboardStats>, (StatusCode, Json<Value>)> {
    // Fetch device stats
    let device_stats = fetch_device_stats(&state).await.unwrap_or(DeviceStats {
        total: 0,
        online: 0,
        offline: 0,
        degraded: 0,
    });

    // Fetch stream stats
    let stream_stats = fetch_stream_stats(&state).await.unwrap_or(StreamStats {
        active: 0,
        total: 0,
    });

    // Fetch recording stats
    let recording_stats = fetch_recording_stats(&state).await.unwrap_or(RecordingStats {
        total: 0,
        today: 0,
        total_size_bytes: 0,
    });

    // Fetch AI task stats
    let ai_task_stats = fetch_ai_task_stats(&state).await.unwrap_or(AiTaskStats {
        active: 0,
        total: 0,
        detections_today: 0,
    });

    // Fetch alert stats
    let alert_stats = fetch_alert_stats(&state).await.unwrap_or(AlertStats {
        active_rules: 0,
        alerts_today: 0,
    });

    // Fetch incident stats
    let incident_store = state.incident_store.read().await;
    let incidents = incident_store.list();
    let incident_stats = IncidentStats {
        open: incidents
            .iter()
            .filter(|i| matches!(i.status, crate::incident::IncidentStatus::Open))
            .count(),
        acknowledged: incidents
            .iter()
            .filter(|i| matches!(i.status, crate::incident::IncidentStatus::Acknowledged))
            .count(),
        total: incidents.len(),
    };

    Ok(Json(DashboardStats {
        devices: device_stats,
        streams: stream_stats,
        recordings: recording_stats,
        ai_tasks: ai_task_stats,
        alerts: alert_stats,
        incidents: incident_stats,
    }))
}

async fn fetch_device_stats(state: &AppState) -> anyhow::Result<DeviceStats> {
    let url = format!("{}/devices", state.config.device_manager_url);
    let response = state.http_client.get(&url).send().await?;

    if response.status().is_success() {
        let devices: Vec<Value> = response.json().await?;
        let total = devices.len();
        let online = devices
            .iter()
            .filter(|d| d["status"] == "online")
            .count();
        let offline = devices
            .iter()
            .filter(|d| d["status"] == "offline")
            .count();
        let degraded = devices
            .iter()
            .filter(|d| d["status"] == "degraded")
            .count();

        Ok(DeviceStats {
            total,
            online,
            offline,
            degraded,
        })
    } else {
        Ok(DeviceStats {
            total: 0,
            online: 0,
            offline: 0,
            degraded: 0,
        })
    }
}

async fn fetch_stream_stats(state: &AppState) -> anyhow::Result<StreamStats> {
    let url = format!("{}/streams", state.config.admin_gateway_url);
    let response = state.http_client.get(&url).send().await?;

    if response.status().is_success() {
        let streams: Vec<Value> = response.json().await?;
        let total = streams.len();
        let active = streams
            .iter()
            .filter(|s| s["status"] == "active")
            .count();

        Ok(StreamStats { active, total })
    } else {
        Ok(StreamStats { active: 0, total: 0 })
    }
}

async fn fetch_recording_stats(state: &AppState) -> anyhow::Result<RecordingStats> {
    let url = format!("{}/recordings", state.config.recorder_node_url);
    let response = state.http_client.get(&url).send().await?;

    if response.status().is_success() {
        let recordings: Vec<Value> = response.json().await?;
        let total = recordings.len();

        // Calculate start of today in Unix timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start_of_today = now - (now % 86400); // Start of day (00:00:00 UTC)

        // Count recordings started today
        let today = recordings
            .iter()
            .filter(|r| {
                if let Some(started_at) = r["started_at"].as_u64() {
                    started_at >= start_of_today
                } else {
                    false
                }
            })
            .count();

        // Sum file sizes from metadata
        let total_size_bytes: u64 = recordings
            .iter()
            .filter_map(|r| {
                r["metadata"]["file_size_bytes"].as_u64()
            })
            .sum();

        Ok(RecordingStats {
            total,
            today,
            total_size_bytes,
        })
    } else {
        Ok(RecordingStats {
            total: 0,
            today: 0,
            total_size_bytes: 0,
        })
    }
}

async fn fetch_ai_task_stats(state: &AppState) -> anyhow::Result<AiTaskStats> {
    let url = format!("{}/tasks", state.config.ai_service_url);
    let response = state.http_client.get(&url).send().await?;

    if response.status().is_success() {
        let tasks_response: Value = response.json().await?;
        let empty_vec = vec![];
        let tasks = tasks_response["tasks"].as_array().unwrap_or(&empty_vec);
        let total = tasks.len();
        let active = tasks
            .iter()
            .filter(|t| t["status"] == "running")
            .count();

        // Note: AI service currently doesn't store historical detection results.
        // Detections are published in real-time but not persisted.
        // To implement this feature, would need to:
        // 1. Add detection results storage to ai-service (with timestamps), OR
        // 2. Store detections in a separate analytics/search service
        let detections_today = 0;

        Ok(AiTaskStats {
            active,
            total,
            detections_today,
        })
    } else {
        Ok(AiTaskStats {
            active: 0,
            total: 0,
            detections_today: 0,
        })
    }
}

async fn fetch_alert_stats(state: &AppState) -> anyhow::Result<AlertStats> {
    let rules_url = format!("{}/rules", state.config.alert_service_url);
    let rules_response = state.http_client.get(&rules_url).send().await?;

    let active_rules = if rules_response.status().is_success() {
        let rules: Vec<Value> = rules_response.json().await?;
        rules.iter().filter(|r| r["enabled"] == true).count()
    } else {
        0
    };

    // Fetch alert events to count today's alerts
    let events_url = format!("{}/events", state.config.alert_service_url);
    let events_response = state.http_client.get(&events_url).send().await?;

    let alerts_today = if events_response.status().is_success() {
        let events: Vec<Value> = events_response.json().await?;

        // Calculate start of today in Unix timestamp
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let start_of_today = now - (now % 86400); // Start of day (00:00:00 UTC)

        // Count alerts fired today (fired_at is ISO 8601 datetime string)
        events
            .iter()
            .filter(|e| {
                if let Some(fired_at_str) = e["fired_at"].as_str() {
                    // Parse ISO 8601 datetime to Unix timestamp
                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(fired_at_str) {
                        let timestamp = dt.timestamp() as u64;
                        timestamp >= start_of_today
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
            .count()
    } else {
        0
    };

    Ok(AlertStats {
        active_rules,
        alerts_today,
    })
}
