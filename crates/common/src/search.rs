use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Recording Search Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingIndexEntry {
  pub id: String,
  pub recording_id: String,
  pub tenant_id: Option<String>,
  pub device_id: Option<String>,
  pub device_name: Option<String>,
  pub zone: Option<String>,
  pub location: Option<String>,
  pub started_at: i64,
  pub stopped_at: Option<i64>,
  pub duration_secs: Option<i32>,
  pub resolution: Option<String>,
  pub video_codec: Option<String>,
  pub audio_codec: Option<String>,
  pub file_size_bytes: Option<i64>,
  pub storage_path: Option<String>,
  #[serde(default)]
  pub tags: Vec<String>,
  #[serde(default)]
  pub labels: HashMap<String, String>,
  pub state: String,
  pub indexed_at: i64,
  pub updated_at: i64,
}

// Event Search Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventIndexEntry {
  pub id: String,
  pub event_id: String,
  pub tenant_id: Option<String>,
  pub event_type: String,
  pub recording_id: Option<String>,
  pub occurred_at: i64,
  pub duration_secs: Option<i32>,
  pub device_id: Option<String>,
  pub device_name: Option<String>,
  pub zone: Option<String>,
  #[serde(default)]
  pub event_data: HashMap<String, serde_json::Value>,
  #[serde(default)]
  pub detected_objects: Vec<String>,
  pub object_count: Option<i32>,
  pub max_confidence: Option<f32>,
  pub snapshot_path: Option<String>,
  pub thumbnail_data: Option<String>,
  pub severity: Option<String>,
  #[serde(default)]
  pub tags: Vec<String>,
  pub indexed_at: i64,
  pub updated_at: i64,
}

// Search Query Types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSearchQuery {
  // Text search
  pub query: Option<String>,

  // Filters
  pub tenant_id: Option<String>,
  pub device_id: Option<String>,
  pub zone: Option<String>,
  pub state: Option<String>,

  // Time range
  pub started_after: Option<i64>,
  pub started_before: Option<i64>,
  pub stopped_after: Option<i64>,
  pub stopped_before: Option<i64>,

  // Duration filter
  pub min_duration_secs: Option<i32>,
  pub max_duration_secs: Option<i32>,

  // Tags and labels
  pub tags: Option<Vec<String>>, // Match ANY of these tags
  pub labels: Option<HashMap<String, String>>, // Match ALL these labels

  // Pagination
  #[serde(default = "default_offset")]
  pub offset: i32,
  #[serde(default = "default_limit")]
  pub limit: i32,

  // Sorting
  #[serde(default = "default_sort_by")]
  pub sort_by: String, // started_at, duration_secs, file_size_bytes
  #[serde(default = "default_sort_order")]
  pub sort_order: String, // asc, desc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSearchQuery {
  // Text search
  pub query: Option<String>,

  // Filters
  pub tenant_id: Option<String>,
  pub event_type: Option<String>,
  pub recording_id: Option<String>,
  pub device_id: Option<String>,
  pub zone: Option<String>,
  pub severity: Option<String>,

  // Time range
  pub occurred_after: Option<i64>,
  pub occurred_before: Option<i64>,

  // Object detection filters
  pub detected_objects: Option<Vec<String>>, // Match ANY of these objects
  pub min_confidence: Option<f32>,
  pub min_object_count: Option<i32>,

  // Tags
  pub tags: Option<Vec<String>>,

  // Pagination
  #[serde(default = "default_offset")]
  pub offset: i32,
  #[serde(default = "default_limit")]
  pub limit: i32,

  // Sorting
  #[serde(default = "default_sort_by")]
  pub sort_by: String, // occurred_at, object_count, max_confidence
  #[serde(default = "default_sort_order")]
  pub sort_order: String, // asc, desc
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSearchQuery {
  // Object type to search for (person, car, dog, etc.)
  pub object_type: String,

  // Filters
  pub tenant_id: Option<String>,
  pub device_id: Option<String>,
  pub zone: Option<String>,

  // Time range
  pub occurred_after: Option<i64>,
  pub occurred_before: Option<i64>,

  // Confidence threshold
  pub min_confidence: Option<f32>,

  // Pagination
  #[serde(default = "default_offset")]
  pub offset: i32,
  #[serde(default = "default_limit")]
  pub limit: i32,
}

// Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingSearchResponse {
  pub recordings: Vec<RecordingIndexEntry>,
  pub total: i64,
  pub offset: i32,
  pub limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSearchResponse {
  pub events: Vec<EventIndexEntry>,
  pub total: i64,
  pub offset: i32,
  pub limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSearchResponse {
  pub events: Vec<EventIndexEntry>,
  pub total: i64,
  pub offset: i32,
  pub limit: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchStatsResponse {
  pub total_recordings: i64,
  pub total_events: i64,
  pub index_size_bytes: i64,
  pub oldest_recording: Option<i64>,
  pub newest_recording: Option<i64>,
}

// Helper functions
fn default_offset() -> i32 {
  0
}

fn default_limit() -> i32 {
  50
}

fn default_sort_by() -> String {
  "started_at".to_string()
}

fn default_sort_order() -> String {
  "desc".to_string()
}
