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

// PTZ Control Types

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PtzDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PtzZoomDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PtzFocusMode {
    Auto,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzMoveRequest {
    pub direction: PtzDirection,
    pub speed: f32, // 0.0 to 1.0
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzStopRequest {
    pub stop_pan_tilt: bool,
    pub stop_zoom: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzZoomRequest {
    pub direction: PtzZoomDirection,
    pub speed: f32, // 0.0 to 1.0
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzAbsolutePositionRequest {
    pub pan: f32,  // -1.0 (left) to 1.0 (right)
    pub tilt: f32, // -1.0 (down) to 1.0 (up)
    pub zoom: f32, // 0.0 (wide) to 1.0 (tele)
    pub speed: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzRelativePositionRequest {
    pub pan: f32,  // Relative movement in degrees
    pub tilt: f32, // Relative movement in degrees
    pub zoom: f32, // Relative zoom change
    pub speed: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzFocusRequest {
    pub mode: PtzFocusMode,
    pub value: Option<f32>, // For manual mode: 0.0 (near) to 1.0 (far)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzIrisRequest {
    pub value: f32, // 0.0 (closed) to 1.0 (open)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzPosition {
    pub pan: f32,
    pub tilt: f32,
    pub zoom: f32,
}

// Custom sqlx encoding/decoding for PtzPosition
impl sqlx::Type<sqlx::Postgres> for PtzPosition {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("jsonb")
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for PtzPosition {
    fn decode(
        value: sqlx::postgres::PgValueRef<'r>,
    ) -> Result<Self, sqlx::error::BoxDynError> {
        let json_value = <serde_json::Value as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Ok(serde_json::from_value(json_value)?)
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Postgres> for PtzPosition {
    fn encode_by_ref(&self, buf: &mut sqlx::postgres::PgArgumentBuffer) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        let json_value = serde_json::to_value(self)?;
        <serde_json::Value as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&json_value, buf)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzStatus {
    pub device_id: String,
    pub position: PtzPosition,
    pub is_moving: bool,
    pub last_updated: DateTime<Utc>,
}

// PTZ Preset Types

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PtzPreset {
    pub preset_id: String,
    pub device_id: String,
    pub name: String,
    pub position: PtzPosition,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePtzPresetRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePtzPresetRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub position: Option<PtzPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GotoPresetRequest {
    pub preset_id: String,
    pub speed: Option<f32>,
}

// PTZ Tour Types

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "tour_state", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TourState {
    Stopped,
    Running,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PtzTour {
    pub tour_id: String,
    pub device_id: String,
    pub name: String,
    pub description: Option<String>,
    pub state: TourState,
    pub loop_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PtzTourStep {
    pub step_id: String,
    pub tour_id: String,
    pub sequence_order: i32,
    pub preset_id: Option<String>,
    pub position: Option<PtzPosition>,
    pub dwell_time_ms: i64,
    pub speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePtzTourRequest {
    pub name: String,
    pub description: Option<String>,
    pub loop_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePtzTourRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub loop_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddTourStepRequest {
    pub preset_id: Option<String>,
    pub position: Option<PtzPosition>,
    pub dwell_time_ms: i64,
    pub speed: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PtzCapabilities {
    pub pan_tilt: bool,
    pub zoom: bool,
    pub focus: bool,
    pub iris: bool,
    pub presets: bool,
    pub tours: bool,
    pub absolute_movement: bool,
    pub relative_movement: bool,
    pub continuous_movement: bool,
    pub home_position: bool,
    pub pan_range: Option<(f32, f32)>,
    pub tilt_range: Option<(f32, f32)>,
    pub zoom_range: Option<(f32, f32)>,
    pub max_presets: Option<u32>,
}

// Camera Configuration Types

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "configuration_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum ConfigurationStatus {
    Pending,
    Applied,
    Failed,
    PartiallyApplied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfigurationRequest {
    // Video encoding settings
    pub video_codec: Option<String>, // "h264", "h265", "mjpeg"
    pub resolution: Option<String>,  // "1920x1080", "1280x720", etc.
    pub framerate: Option<u32>,      // fps
    pub bitrate: Option<u32>,        // kbps
    pub gop_size: Option<u32>,       // keyframe interval (Group of Pictures)
    pub quality: Option<String>,     // "low", "medium", "high"

    // Image settings
    pub brightness: Option<f32>,     // 0.0-1.0
    pub contrast: Option<f32>,       // 0.0-1.0
    pub saturation: Option<f32>,     // 0.0-1.0
    pub sharpness: Option<f32>,      // 0.0-1.0
    pub hue: Option<f32>,            // 0.0-1.0

    // Audio settings
    pub audio_enabled: Option<bool>,
    pub audio_codec: Option<String>, // "aac", "pcm", "g711"
    pub audio_bitrate: Option<u32>,  // kbps

    // Network settings
    pub multicast_enabled: Option<bool>,
    pub multicast_address: Option<String>,
    pub rtsp_port: Option<u32>,

    // Other settings
    pub ir_mode: Option<String>,     // "auto", "on", "off"
    pub wdr_enabled: Option<bool>,   // Wide Dynamic Range
    pub metadata: Option<JsonValue>,  // Additional custom settings
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfigurationResponse {
    pub config_id: String,
    pub device_id: String,
    pub status: ConfigurationStatus,
    pub applied_settings: HashMap<String, JsonValue>,
    pub failed_settings: Option<HashMap<String, String>>, // setting_name -> error_message
    pub error_message: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DeviceConfiguration {
    pub config_id: String,
    pub device_id: String,
    pub requested_config: JsonValue,
    pub applied_config: Option<JsonValue>,
    pub status: ConfigurationStatus,
    pub error_message: Option<String>,
    pub applied_by: Option<String>, // user_id
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurationHistoryQuery {
    pub device_id: String,
    pub status: Option<ConfigurationStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// Firmware Update Types

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "lowercase")]
pub enum FirmwareUpdateStatus {
    Pending,
    Uploading,
    Uploaded,
    Installing,
    Rebooting,
    Verifying,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for FirmwareUpdateStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirmwareUpdateStatus::Pending => write!(f, "pending"),
            FirmwareUpdateStatus::Uploading => write!(f, "uploading"),
            FirmwareUpdateStatus::Uploaded => write!(f, "uploaded"),
            FirmwareUpdateStatus::Installing => write!(f, "installing"),
            FirmwareUpdateStatus::Rebooting => write!(f, "rebooting"),
            FirmwareUpdateStatus::Verifying => write!(f, "verifying"),
            FirmwareUpdateStatus::Completed => write!(f, "completed"),
            FirmwareUpdateStatus::Failed => write!(f, "failed"),
            FirmwareUpdateStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FirmwareUpdate {
    pub update_id: String,
    pub device_id: String,
    pub firmware_version: String,
    pub firmware_file_path: String,
    pub firmware_file_size: i64,
    pub firmware_checksum: String,

    // Status tracking
    pub status: FirmwareUpdateStatus,
    pub progress_percent: i32,

    // Error handling
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,

    // Metadata
    pub previous_firmware_version: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub release_notes: Option<String>,
    pub release_date: Option<DateTime<Utc>>,

    // Rollback support
    pub can_rollback: bool,
    pub rollback_data: Option<JsonValue>,

    // Audit
    pub initiated_by: Option<String>,
    pub initiated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FirmwareUpdateHistory {
    pub history_id: i64,
    pub update_id: String,
    pub status: String,
    pub progress_percent: i32,
    pub message: Option<String>,
    pub metadata: Option<JsonValue>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FirmwareFile {
    pub file_id: String,
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,

    // File information
    pub file_path: String,
    pub file_size: i64,
    pub checksum: String,
    pub mime_type: Option<String>,

    // Metadata
    pub release_notes: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub min_device_version: Option<String>,
    pub compatible_models: Vec<String>,
    pub metadata: Option<JsonValue>,

    // Validation
    pub is_verified: bool,
    pub is_deprecated: bool,

    // Timestamps
    pub uploaded_by: Option<String>,
    pub uploaded_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateFirmwareUpdateRequest {
    pub firmware_file_id: Option<String>, // Use existing file from catalog
    pub firmware_file: Option<Vec<u8>>,   // Or upload new file
    pub firmware_version: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub release_notes: Option<String>,
    pub max_retries: Option<i32>,
    pub force: bool, // Force update even if same/older version
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFirmwareFileRequest {
    pub manufacturer: String,
    pub model: String,
    pub firmware_version: String,
    pub firmware_file_base64: Option<String>, // Base64-encoded firmware file data
    pub release_notes: Option<String>,
    pub release_date: Option<DateTime<Utc>>,
    pub min_device_version: Option<String>,
    pub compatible_models: Option<Vec<String>>,
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareUpdateProgressReport {
    pub update_id: String,
    pub status: FirmwareUpdateStatus,
    pub progress_percent: i32,
    pub message: Option<String>,
    pub estimated_time_remaining_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareUpdateListQuery {
    pub device_id: Option<String>,
    pub status: Option<FirmwareUpdateStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareFileListQuery {
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub is_verified: Option<bool>,
    pub is_deprecated: Option<bool>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
