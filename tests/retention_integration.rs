use common::retention::*;
use std::collections::HashMap;

/// Basic test to verify retention types can be serialized/deserialized
#[tokio::test]
async fn test_retention_policy_serialization() {
  let mut conditions = HashMap::new();
  conditions.insert("device_id".to_string(), serde_json::json!("device-123"));

  let policy = RetentionPolicy {
    id: "policy-123".to_string(),
    tenant_id: Some("tenant-1".to_string()),
    name: "Delete old recordings".to_string(),
    description: Some("Delete recordings older than 30 days".to_string()),
    enabled: true,
    policy_type: PolicyType::TimeBased,
    retention_days: Some(30),
    max_storage_bytes: None,
    conditions,
    enable_tiered_storage: false,
    cold_storage_after_days: None,
    cold_storage_path: None,
    priority: 0,
    dry_run: false,
    created_at: Some(1704067200),
    updated_at: Some(1704067200),
    created_by: None,
  };

  // Test serialization
  let json = serde_json::to_string(&policy).unwrap();
  assert!(json.contains("Delete old recordings"));

  // Test deserialization
  let deserialized: RetentionPolicy = serde_json::from_str(&json).unwrap();
  assert_eq!(deserialized.name, policy.name);
  assert_eq!(deserialized.policy_type, policy.policy_type);
  assert_eq!(deserialized.retention_days, Some(30));
}

#[tokio::test]
async fn test_retention_execution_serialization() {
  let execution = RetentionExecution {
    id: "exec-123".to_string(),
    policy_id: "policy-123".to_string(),
    status: ExecutionStatus::Completed,
    recordings_scanned: 100,
    recordings_deleted: 25,
    recordings_moved_to_cold: 10,
    bytes_freed: 1024 * 1024 * 500, // 500 MB
    bytes_moved: 1024 * 1024 * 200, // 200 MB
    started_at: 1704067200,
    completed_at: Some(1704067260),
    duration_secs: Some(60),
    error_message: None,
    created_at: Some(1704067200),
  };

  // Test serialization
  let json = serde_json::to_string(&execution).unwrap();
  assert!(json.contains("exec-123"));

  // Test deserialization
  let deserialized: RetentionExecution = serde_json::from_str(&json).unwrap();
  assert_eq!(deserialized.id, execution.id);
  assert_eq!(deserialized.status, ExecutionStatus::Completed);
  assert_eq!(deserialized.recordings_deleted, 25);
}

#[tokio::test]
async fn test_retention_action_types() {
  let delete_action = RetentionAction {
    id: "action-1".to_string(),
    execution_id: "exec-1".to_string(),
    recording_id: "rec-1".to_string(),
    action_type: ActionType::Delete,
    status: ActionStatus::Completed,
    recording_path: Some("/data/recordings/rec-1.mp4".to_string()),
    recording_size_bytes: Some(1024 * 1024 * 100),
    recording_duration_secs: Some(3600),
    recording_created_at: Some(1704000000),
    performed_at: Some(1704067200),
    error_message: None,
    created_at: Some(1704067200),
  };

  // Test serialization
  let json = serde_json::to_string(&delete_action).unwrap();
  assert!(json.contains("delete"));

  // Test deserialization
  let deserialized: RetentionAction = serde_json::from_str(&json).unwrap();
  assert_eq!(deserialized.action_type, ActionType::Delete);
  assert_eq!(deserialized.status, ActionStatus::Completed);
}

#[tokio::test]
async fn test_create_retention_policy_request() {
  let mut conditions = HashMap::new();
  conditions.insert("min_duration_secs".to_string(), serde_json::json!(30));

  let req = CreateRetentionPolicyRequest {
    name: "Test Policy".to_string(),
    description: Some("Test policy description".to_string()),
    policy_type: PolicyType::StorageQuota,
    retention_days: None,
    max_storage_bytes: Some(1024 * 1024 * 1024 * 100), // 100 GB
    conditions,
    enable_tiered_storage: true,
    cold_storage_after_days: Some(7),
    cold_storage_path: Some("/mnt/cold-storage".to_string()),
    priority: 10,
    dry_run: true,
    tenant_id: Some("tenant-1".to_string()),
  };

  // Test serialization
  let json = serde_json::to_string(&req).unwrap();
  assert!(json.contains("Test Policy"));
  assert!(json.contains("storage_quota"));

  // Test deserialization
  let deserialized: CreateRetentionPolicyRequest = serde_json::from_str(&json).unwrap();
  assert_eq!(deserialized.name, req.name);
  assert_eq!(deserialized.policy_type, PolicyType::StorageQuota);
  assert_eq!(deserialized.enable_tiered_storage, true);
}
