use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "device_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Camera,
    Nvr,
    Encoder,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "device_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum DeviceStatus {
    Online,
    Offline,
    Error,
    Maintenance,
    Provisioning,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "connection_protocol", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ConnectionProtocol {
    Rtsp,
    Onvif,
    Http,
    Rtmp,
    WebRtc,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Device {
    pub device_id: String,
    pub tenant_id: String,
    pub name: String,
    pub device_type: DeviceType,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,

    // Connection information
    pub primary_uri: String,
    pub secondary_uri: Option<String>,
    pub protocol: ConnectionProtocol,
    pub username: Option<String>,
    #[serde(skip_serializing)]
    pub password_encrypted: Option<String>,

    // Location and grouping
    pub location: Option<String>,
    pub zone: Option<String>,
    pub tags: Vec<String>,

    // Status and health
    pub status: DeviceStatus,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub last_health_check_at: Option<DateTime<Utc>>,
    pub health_check_interval_secs: i32,
    pub consecutive_failures: i32,

    // Device capabilities
    pub capabilities: Option<JsonValue>,
    pub video_codecs: Vec<String>,
    pub audio_codecs: Vec<String>,
    pub resolutions: Vec<String>,

    // Metadata
    pub description: Option<String>,
    pub notes: Option<String>,
    pub metadata: Option<JsonValue>,

    // Configuration
    pub auto_start: bool,
    pub recording_enabled: bool,
    pub ai_enabled: bool,

    // Timestamps
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDeviceRequest {
    pub name: String,
    pub device_type: DeviceType,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub primary_uri: String,
    pub secondary_uri: Option<String>,
    pub protocol: ConnectionProtocol,
    pub username: Option<String>,
    pub password: Option<String>,
    pub location: Option<String>,
    pub zone: Option<String>,
    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub health_check_interval_secs: Option<i32>,
    pub auto_start: Option<bool>,
    pub recording_enabled: Option<bool>,
    pub ai_enabled: Option<bool>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub primary_uri: Option<String>,
    pub secondary_uri: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub location: Option<String>,
    pub zone: Option<String>,
    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub notes: Option<String>,
    pub health_check_interval_secs: Option<i32>,
    pub auto_start: Option<bool>,
    pub recording_enabled: Option<bool>,
    pub ai_enabled: Option<bool>,
    pub status: Option<DeviceStatus>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceHealthHistory {
    pub history_id: i64,
    pub device_id: String,
    pub status: DeviceStatus,
    pub response_time_ms: Option<i32>,
    pub error_message: Option<String>,
    pub metadata: Option<JsonValue>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceEvent {
    pub event_id: i64,
    pub device_id: String,
    pub event_type: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub user_id: Option<String>,
    pub metadata: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeResult {
    pub success: bool,
    pub response_time_ms: u64,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub firmware_version: Option<String>,
    pub capabilities: HashMap<String, bool>,
    pub video_codecs: Vec<String>,
    pub audio_codecs: Vec<String>,
    pub resolutions: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckResult {
    pub device_id: String,
    pub status: DeviceStatus,
    pub response_time_ms: u64,
    pub timestamp: DateTime<Utc>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateRequest {
    pub device_ids: Vec<String>,
    pub update: UpdateDeviceRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchUpdateResponse {
    pub succeeded: Vec<String>,
    pub failed: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListQuery {
    pub tenant_id: Option<String>,
    pub status: Option<DeviceStatus>,
    pub device_type: Option<DeviceType>,
    pub zone: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
