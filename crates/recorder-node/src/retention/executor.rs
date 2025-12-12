use anyhow::Result;
use common::retention::*;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::recording::manager::RECORDING_MANAGER;
use super::store::RetentionStore;

pub struct RetentionExecutor {
  store: Arc<dyn RetentionStore>,
  recording_storage_root: String,
}

impl RetentionExecutor {
  pub fn new(store: Arc<dyn RetentionStore>, recording_storage_root: String) -> Self {
    Self {
      store,
      recording_storage_root,
    }
  }

  /// Execute a specific retention policy
  pub async fn execute_policy(&self, policy_id: &str) -> Result<RetentionExecution> {
    let policy = self
      .store
      .get_policy(policy_id)
      .await?
      .ok_or_else(|| anyhow::anyhow!("policy not found"))?;

    if !policy.enabled {
      return Err(anyhow::anyhow!("policy is disabled"));
    }

    info!(
      policy_id = %policy.id,
      policy_name = %policy.name,
      policy_type = ?policy.policy_type,
      "starting retention policy execution"
    );

    let mut execution = self.store.create_execution(&policy.id).await?;

    // Get all recordings
    let all_recordings = RECORDING_MANAGER.list().await;
    execution.recordings_scanned = all_recordings.len() as i32;

    info!(
      execution_id = %execution.id,
      recordings_scanned = all_recordings.len(),
      "scanned recordings for retention policy"
    );

    // Filter recordings based on policy conditions
    let matching_recordings = self.filter_recordings(&all_recordings, &policy);

    info!(
      execution_id = %execution.id,
      matching_count = matching_recordings.len(),
      "filtered recordings matching policy conditions"
    );

    // Determine actions for each recording
    let actions = self.determine_actions(&matching_recordings, &policy);

    info!(
      execution_id = %execution.id,
      action_count = actions.len(),
      "determined retention actions"
    );

    // Execute actions
    for mut action in actions {
      // Save action to database
      if let Err(e) = self.store.create_action(&action).await {
        warn!(
          execution_id = %execution.id,
          recording_id = %action.recording_id,
          error = %e,
          "failed to create retention action record"
        );
        continue;
      }

      // Perform the action
      if !policy.dry_run {
        match self.perform_action(&action).await {
          Ok(bytes_affected) => {
            action.status = ActionStatus::Completed;
            action.performed_at = Some(
              SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            );

            match action.action_type {
              ActionType::Delete => {
                execution.recordings_deleted += 1;
                execution.bytes_freed += bytes_affected;
              }
              ActionType::MoveToCold => {
                execution.recordings_moved_to_cold += 1;
                execution.bytes_moved += bytes_affected;
              }
              ActionType::Skip => {}
            }

            info!(
              execution_id = %execution.id,
              recording_id = %action.recording_id,
              action_type = ?action.action_type,
              bytes_affected = bytes_affected,
              "retention action completed"
            );
          }
          Err(e) => {
            action.status = ActionStatus::Failed;
            action.error_message = Some(e.to_string());
            error!(
              execution_id = %execution.id,
              recording_id = %action.recording_id,
              action_type = ?action.action_type,
              error = %e,
              "retention action failed"
            );
          }
        }
      } else {
        // Dry run mode
        action.status = ActionStatus::Completed;
        action.performed_at = Some(
          SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
        );
        info!(
          execution_id = %execution.id,
          recording_id = %action.recording_id,
          action_type = ?action.action_type,
          "retention action (dry run, not performed)"
        );
      }

      // Update action in database
      if let Err(e) = self.store.update_action(&action).await {
        warn!(
          execution_id = %execution.id,
          recording_id = %action.recording_id,
          error = %e,
          "failed to update retention action record"
        );
      }
    }

    // Finalize execution
    execution.status = ExecutionStatus::Completed;
    execution.completed_at = Some(
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64,
    );
    execution.duration_secs =
      Some((execution.completed_at.unwrap() - execution.started_at) as i32);

    self.store.update_execution(&execution).await?;

    info!(
      execution_id = %execution.id,
      policy_id = %policy.id,
      recordings_deleted = execution.recordings_deleted,
      recordings_moved = execution.recordings_moved_to_cold,
      bytes_freed = execution.bytes_freed,
      bytes_moved = execution.bytes_moved,
      duration_secs = execution.duration_secs,
      "retention policy execution completed"
    );

    Ok(execution)
  }

  /// Filter recordings that match policy conditions
  fn filter_recordings(
    &self,
    recordings: &[common::recordings::RecordingInfo],
    policy: &RetentionPolicy,
  ) -> Vec<common::recordings::RecordingInfo> {
    recordings
      .iter()
      .filter(|rec| {
        // Check conditions
        for (key, value) in &policy.conditions {
          match key.as_str() {
            "device_id" => {
              if let Some(device_id) = value.as_str() {
                if rec.config.source_stream_id.as_ref() != Some(&device_id.to_string()) {
                  return false;
                }
              }
            }
            "min_duration_secs" => {
              if let Some(min_duration) = value.as_i64() {
                if let Some(metadata) = &rec.metadata {
                  if let Some(duration) = metadata.duration_secs {
                    if (duration as i64) < min_duration {
                      return false;
                    }
                  }
                }
              }
            }
            _ => {}
          }
        }
        true
      })
      .cloned()
      .collect()
  }

  /// Determine what action to take for each recording
  fn determine_actions(
    &self,
    recordings: &[common::recordings::RecordingInfo],
    policy: &RetentionPolicy,
  ) -> Vec<RetentionAction> {
    let now = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap()
      .as_secs();

    recordings
      .iter()
      .filter_map(|rec| {
        let recording_age_days = rec
          .started_at
          .map(|started| ((now - started) / 86400) as i32)?;

        // Determine action based on policy type
        let action_type = match policy.policy_type {
          PolicyType::TimeBased => {
            if let Some(retention_days) = policy.retention_days {
              if recording_age_days > retention_days {
                // Check if should move to cold storage first
                if policy.enable_tiered_storage {
                  if let Some(cold_after_days) = policy.cold_storage_after_days {
                    if recording_age_days > cold_after_days
                      && recording_age_days <= retention_days
                    {
                      Some(ActionType::MoveToCold)
                    } else if recording_age_days > retention_days {
                      Some(ActionType::Delete)
                    } else {
                      None
                    }
                  } else {
                    Some(ActionType::Delete)
                  }
                } else {
                  Some(ActionType::Delete)
                }
              } else {
                // Check if should move to cold storage
                if policy.enable_tiered_storage {
                  if let Some(cold_after_days) = policy.cold_storage_after_days {
                    if recording_age_days > cold_after_days {
                      Some(ActionType::MoveToCold)
                    } else {
                      None
                    }
                  } else {
                    None
                  }
                } else {
                  None
                }
              }
            } else {
              None
            }
          }
          PolicyType::StorageQuota => {
            // For storage quota, we would need to track total storage
            // and delete oldest recordings first
            // This is a simplified version
            Some(ActionType::Delete)
          }
          PolicyType::Conditional => {
            // Custom conditional logic based on conditions
            None
          }
        };

        action_type.map(|at| {
          let file_size = rec
            .metadata
            .as_ref()
            .and_then(|m| m.file_size_bytes)
            .unwrap_or(0);

          RetentionAction {
            id: Uuid::new_v4().to_string(),
            execution_id: String::new(), // Will be set by executor
            recording_id: rec.config.id.clone(),
            action_type: at,
            status: ActionStatus::Pending,
            recording_path: rec.storage_path.clone(),
            recording_size_bytes: Some(file_size as i64),
            recording_duration_secs: rec
              .metadata
              .as_ref()
              .and_then(|m| m.duration_secs)
              .map(|d| d as i64),
            recording_created_at: rec.started_at.map(|t| t as i64),
            performed_at: None,
            error_message: None,
            created_at: Some(now as i64),
          }
        })
      })
      .collect()
  }

  /// Perform the actual retention action
  async fn perform_action(&self, action: &RetentionAction) -> Result<i64> {
    match action.action_type {
      ActionType::Delete => {
        if let Some(path) = &action.recording_path {
          let full_path = Path::new(&self.recording_storage_root).join(path);

          // Get file size before deletion
          let file_size = if let Ok(metadata) = fs::metadata(&full_path).await {
            metadata.len() as i64
          } else {
            action.recording_size_bytes.unwrap_or(0)
          };

          // Delete the file
          if full_path.exists() {
            fs::remove_file(&full_path).await?;
            info!(
              recording_id = %action.recording_id,
              path = %full_path.display(),
              size_bytes = file_size,
              "deleted recording file"
            );
          } else {
            warn!(
              recording_id = %action.recording_id,
              path = %full_path.display(),
              "recording file not found, skipping deletion"
            );
          }

          Ok(file_size)
        } else {
          Err(anyhow::anyhow!("no storage path for recording"))
        }
      }
      ActionType::MoveToCold => {
        if let Some(source_path) = &action.recording_path {
          let cold_storage_path = self
            .store
            .get_policy(&action.execution_id)
            .await?
            .and_then(|p| p.cold_storage_path)
            .ok_or_else(|| anyhow::anyhow!("no cold storage path configured"))?;

          let source = Path::new(&self.recording_storage_root).join(source_path);
          let dest = Path::new(&cold_storage_path).join(source_path);

          // Create destination directory if needed
          if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
          }

          // Get file size
          let file_size = if let Ok(metadata) = fs::metadata(&source).await {
            metadata.len() as i64
          } else {
            action.recording_size_bytes.unwrap_or(0)
          };

          // Move the file
          fs::rename(&source, &dest).await?;
          info!(
            recording_id = %action.recording_id,
            from = %source.display(),
            to = %dest.display(),
            size_bytes = file_size,
            "moved recording to cold storage"
          );

          Ok(file_size)
        } else {
          Err(anyhow::anyhow!("no storage path for recording"))
        }
      }
      ActionType::Skip => Ok(0),
    }
  }

  /// Execute all enabled policies
  pub async fn execute_all_policies(&self) -> Result<Vec<RetentionExecution>> {
    let policies = self.store.list_policies(None).await?;
    let enabled_policies: Vec<_> = policies.into_iter().filter(|p| p.enabled).collect();

    info!(
      policy_count = enabled_policies.len(),
      "executing all enabled retention policies"
    );

    let mut executions = Vec::new();
    for policy in enabled_policies {
      match self.execute_policy(&policy.id).await {
        Ok(execution) => {
          executions.push(execution);
        }
        Err(e) => {
          error!(
            policy_id = %policy.id,
            policy_name = %policy.name,
            error = %e,
            "failed to execute retention policy"
          );
        }
      }
    }

    Ok(executions)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_retention_executor_creation() {
    // Basic test to ensure compilation
    // Real tests would need mock store
  }
}
