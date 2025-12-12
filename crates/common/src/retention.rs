use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyType {
  TimeBased,
  StorageQuota,
  Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
  pub id: String,
  pub tenant_id: Option<String>,
  pub name: String,
  pub description: Option<String>,
  pub enabled: bool,
  pub policy_type: PolicyType,

  // Time-based settings
  pub retention_days: Option<i32>,

  // Storage quota settings (in bytes)
  pub max_storage_bytes: Option<i64>,

  // Conditional settings
  #[serde(default)]
  pub conditions: HashMap<String, serde_json::Value>,

  // Tiered storage settings
  pub enable_tiered_storage: bool,
  pub cold_storage_after_days: Option<i32>,
  pub cold_storage_path: Option<String>,

  // Execution settings
  pub priority: i32,
  pub dry_run: bool,

  #[serde(default)]
  pub created_at: Option<i64>,
  #[serde(default)]
  pub updated_at: Option<i64>,
  pub created_by: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
  Running,
  Completed,
  Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionExecution {
  pub id: String,
  pub policy_id: String,
  pub status: ExecutionStatus,
  pub recordings_scanned: i32,
  pub recordings_deleted: i32,
  pub recordings_moved_to_cold: i32,
  pub bytes_freed: i64,
  pub bytes_moved: i64,
  pub started_at: i64,
  pub completed_at: Option<i64>,
  pub duration_secs: Option<i32>,
  pub error_message: Option<String>,
  #[serde(default)]
  pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
  Delete,
  MoveToCold,
  Skip,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
  Pending,
  Completed,
  Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionAction {
  pub id: String,
  pub execution_id: String,
  pub recording_id: String,
  pub action_type: ActionType,
  pub status: ActionStatus,
  pub recording_path: Option<String>,
  pub recording_size_bytes: Option<i64>,
  pub recording_duration_secs: Option<i64>,
  pub recording_created_at: Option<i64>,
  pub performed_at: Option<i64>,
  pub error_message: Option<String>,
  #[serde(default)]
  pub created_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStatistics {
  pub id: String,
  pub tenant_id: Option<String>,
  pub device_id: Option<String>,
  pub zone: Option<String>,
  pub total_recordings: i32,
  pub total_bytes: i64,
  pub oldest_recording_at: Option<i64>,
  pub newest_recording_at: Option<i64>,
  pub calculated_at: i64,
}

// Request/Response types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRetentionPolicyRequest {
  pub name: String,
  pub description: Option<String>,
  pub policy_type: PolicyType,
  pub retention_days: Option<i32>,
  pub max_storage_bytes: Option<i64>,
  #[serde(default)]
  pub conditions: HashMap<String, serde_json::Value>,
  #[serde(default)]
  pub enable_tiered_storage: bool,
  pub cold_storage_after_days: Option<i32>,
  pub cold_storage_path: Option<String>,
  #[serde(default)]
  pub priority: i32,
  #[serde(default)]
  pub dry_run: bool,
  pub tenant_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateRetentionPolicyRequest {
  pub name: Option<String>,
  pub description: Option<String>,
  pub enabled: Option<bool>,
  pub retention_days: Option<i32>,
  pub max_storage_bytes: Option<i64>,
  pub conditions: Option<HashMap<String, serde_json::Value>>,
  pub enable_tiered_storage: Option<bool>,
  pub cold_storage_after_days: Option<i32>,
  pub cold_storage_path: Option<String>,
  pub priority: Option<i32>,
  pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePolicyRequest {
  pub policy_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutePolicyResponse {
  pub execution_id: String,
  pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPoliciesResponse {
  pub policies: Vec<RetentionPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListExecutionsResponse {
  pub executions: Vec<RetentionExecution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListActionsResponse {
  pub actions: Vec<RetentionAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStatsResponse {
  pub statistics: Vec<StorageStatistics>,
}
