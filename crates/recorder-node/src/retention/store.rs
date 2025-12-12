use anyhow::Result;
use async_trait::async_trait;
use common::retention::*;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::warn;
use uuid::Uuid;

#[async_trait]
pub trait RetentionStore: Send + Sync {
  // Policy CRUD
  async fn create_policy(&self, req: CreateRetentionPolicyRequest) -> Result<RetentionPolicy>;
  async fn get_policy(&self, policy_id: &str) -> Result<Option<RetentionPolicy>>;
  async fn list_policies(&self, tenant_id: Option<&str>) -> Result<Vec<RetentionPolicy>>;
  async fn update_policy(
    &self,
    policy_id: &str,
    req: UpdateRetentionPolicyRequest,
  ) -> Result<RetentionPolicy>;
  async fn delete_policy(&self, policy_id: &str) -> Result<bool>;

  // Execution tracking
  async fn create_execution(&self, policy_id: &str) -> Result<RetentionExecution>;
  async fn update_execution(&self, execution: &RetentionExecution) -> Result<()>;
  async fn get_execution(&self, execution_id: &str) -> Result<Option<RetentionExecution>>;
  async fn list_executions(&self, policy_id: Option<&str>) -> Result<Vec<RetentionExecution>>;

  // Action tracking
  async fn create_action(&self, action: &RetentionAction) -> Result<()>;
  async fn update_action(&self, action: &RetentionAction) -> Result<()>;
  async fn list_actions(&self, execution_id: &str) -> Result<Vec<RetentionAction>>;

  // Storage statistics
  async fn update_storage_stats(&self, stats: &StorageStatistics) -> Result<()>;
  async fn get_storage_stats(
    &self,
    tenant_id: Option<&str>,
    device_id: Option<&str>,
  ) -> Result<Vec<StorageStatistics>>;
}

pub struct PostgresRetentionStore {
  pool: PgPool,
}

impl PostgresRetentionStore {
  pub fn new(pool: PgPool) -> Self {
    Self { pool }
  }

  fn map_policy_row(row: sqlx::postgres::PgRow) -> Result<RetentionPolicy> {
    use sqlx::Row;

    let policy_type_str: String = row.try_get("policy_type")?;
    let policy_type = match policy_type_str.as_str() {
      "time_based" => PolicyType::TimeBased,
      "storage_quota" => PolicyType::StorageQuota,
      "conditional" => PolicyType::Conditional,
      _ => PolicyType::TimeBased,
    };

    let condition_json: serde_json::Value = row.try_get("condition_json")?;
    let conditions: HashMap<String, serde_json::Value> = serde_json::from_value(condition_json)
      .unwrap_or_default();

    let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
    let updated_at: chrono::DateTime<chrono::Utc> = row.try_get("updated_at")?;

    Ok(RetentionPolicy {
      id: row.try_get::<Uuid, _>("id")?.to_string(),
      tenant_id: row
        .try_get::<Option<Uuid>, _>("tenant_id")?
        .map(|u| u.to_string()),
      name: row.try_get("name")?,
      description: row.try_get("description")?,
      enabled: row.try_get("enabled")?,
      policy_type,
      retention_days: row.try_get("retention_days")?,
      max_storage_bytes: row.try_get("max_storage_bytes")?,
      conditions,
      enable_tiered_storage: row.try_get("enable_tiered_storage")?,
      cold_storage_after_days: row.try_get("cold_storage_after_days")?,
      cold_storage_path: row.try_get("cold_storage_path")?,
      priority: row.try_get("priority")?,
      dry_run: row.try_get("dry_run")?,
      created_at: Some(created_at.timestamp()),
      updated_at: Some(updated_at.timestamp()),
      created_by: row
        .try_get::<Option<Uuid>, _>("created_by")?
        .map(|u| u.to_string()),
    })
  }

  fn map_execution_row(row: sqlx::postgres::PgRow) -> Result<RetentionExecution> {
    use sqlx::Row;

    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
      "running" => ExecutionStatus::Running,
      "completed" => ExecutionStatus::Completed,
      "failed" => ExecutionStatus::Failed,
      _ => ExecutionStatus::Running,
    };

    let started_at: chrono::DateTime<chrono::Utc> = row.try_get("started_at")?;
    let completed_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("completed_at")?;
    let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;

    Ok(RetentionExecution {
      id: row.try_get::<Uuid, _>("id")?.to_string(),
      policy_id: row.try_get::<Uuid, _>("policy_id")?.to_string(),
      status,
      recordings_scanned: row.try_get("recordings_scanned")?,
      recordings_deleted: row.try_get("recordings_deleted")?,
      recordings_moved_to_cold: row.try_get("recordings_moved_to_cold")?,
      bytes_freed: row.try_get("bytes_freed")?,
      bytes_moved: row.try_get("bytes_moved")?,
      started_at: started_at.timestamp(),
      completed_at: completed_at.map(|t| t.timestamp()),
      duration_secs: row.try_get("duration_secs")?,
      error_message: row.try_get("error_message")?,
      created_at: Some(created_at.timestamp()),
    })
  }

  fn map_action_row(row: sqlx::postgres::PgRow) -> Result<RetentionAction> {
    use sqlx::Row;

    let action_type_str: String = row.try_get("action_type")?;
    let action_type = match action_type_str.as_str() {
      "delete" => ActionType::Delete,
      "move_to_cold" => ActionType::MoveToCold,
      "skip" => ActionType::Skip,
      _ => ActionType::Skip,
    };

    let status_str: String = row.try_get("status")?;
    let status = match status_str.as_str() {
      "pending" => ActionStatus::Pending,
      "completed" => ActionStatus::Completed,
      "failed" => ActionStatus::Failed,
      _ => ActionStatus::Pending,
    };

    let recording_created_at: Option<chrono::DateTime<chrono::Utc>> =
      row.try_get("recording_created_at")?;
    let performed_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("performed_at")?;
    let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;

    Ok(RetentionAction {
      id: row.try_get::<Uuid, _>("id")?.to_string(),
      execution_id: row.try_get::<Uuid, _>("execution_id")?.to_string(),
      recording_id: row.try_get("recording_id")?,
      action_type,
      status,
      recording_path: row.try_get("recording_path")?,
      recording_size_bytes: row.try_get("recording_size_bytes")?,
      recording_duration_secs: row.try_get("recording_duration_secs")?,
      recording_created_at: recording_created_at.map(|t| t.timestamp()),
      performed_at: performed_at.map(|t| t.timestamp()),
      error_message: row.try_get("error_message")?,
      created_at: Some(created_at.timestamp()),
    })
  }

  fn map_stats_row(row: sqlx::postgres::PgRow) -> Result<StorageStatistics> {
    use sqlx::Row;

    let oldest_recording_at: Option<chrono::DateTime<chrono::Utc>> =
      row.try_get("oldest_recording_at")?;
    let newest_recording_at: Option<chrono::DateTime<chrono::Utc>> =
      row.try_get("newest_recording_at")?;
    let calculated_at: chrono::DateTime<chrono::Utc> = row.try_get("calculated_at")?;

    Ok(StorageStatistics {
      id: row.try_get::<Uuid, _>("id")?.to_string(),
      tenant_id: row
        .try_get::<Option<Uuid>, _>("tenant_id")?
        .map(|u| u.to_string()),
      device_id: row
        .try_get::<Option<Uuid>, _>("device_id")?
        .map(|u| u.to_string()),
      zone: row.try_get("zone")?,
      total_recordings: row.try_get("total_recordings")?,
      total_bytes: row.try_get("total_bytes")?,
      oldest_recording_at: oldest_recording_at.map(|t| t.timestamp()),
      newest_recording_at: newest_recording_at.map(|t| t.timestamp()),
      calculated_at: calculated_at.timestamp(),
    })
  }
}

#[async_trait]
impl RetentionStore for PostgresRetentionStore {
  async fn create_policy(&self, req: CreateRetentionPolicyRequest) -> Result<RetentionPolicy> {
    let id = Uuid::new_v4();
    let tenant_id = req.tenant_id.as_ref().and_then(|s| Uuid::parse_str(s).ok());
    let policy_type_str = match req.policy_type {
      PolicyType::TimeBased => "time_based",
      PolicyType::StorageQuota => "storage_quota",
      PolicyType::Conditional => "conditional",
    };
    let condition_json = serde_json::to_value(&req.conditions)?;

    let row = sqlx::query(
      r#"
      INSERT INTO retention_policies
        (id, tenant_id, name, description, policy_type, retention_days, max_storage_bytes,
         condition_json, enable_tiered_storage, cold_storage_after_days, cold_storage_path,
         priority, dry_run)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
      RETURNING *
      "#,
    )
    .bind(id)
    .bind(tenant_id)
    .bind(&req.name)
    .bind(&req.description)
    .bind(policy_type_str)
    .bind(req.retention_days)
    .bind(req.max_storage_bytes)
    .bind(condition_json)
    .bind(req.enable_tiered_storage)
    .bind(req.cold_storage_after_days)
    .bind(&req.cold_storage_path)
    .bind(req.priority)
    .bind(req.dry_run)
    .fetch_one(&self.pool)
    .await?;

    Self::map_policy_row(row)
  }

  async fn get_policy(&self, policy_id: &str) -> Result<Option<RetentionPolicy>> {
    let uuid = Uuid::parse_str(policy_id)?;
    let row = sqlx::query("SELECT * FROM retention_policies WHERE id = $1")
      .bind(uuid)
      .fetch_optional(&self.pool)
      .await?;

    match row {
      Some(r) => Ok(Some(Self::map_policy_row(r)?)),
      None => Ok(None),
    }
  }

  async fn list_policies(&self, tenant_id: Option<&str>) -> Result<Vec<RetentionPolicy>> {
    let rows = if let Some(tid) = tenant_id {
      let uuid = Uuid::parse_str(tid)?;
      sqlx::query("SELECT * FROM retention_policies WHERE tenant_id = $1 ORDER BY priority DESC")
        .bind(uuid)
        .fetch_all(&self.pool)
        .await?
    } else {
      sqlx::query("SELECT * FROM retention_policies ORDER BY priority DESC")
        .fetch_all(&self.pool)
        .await?
    };

    rows.into_iter().map(Self::map_policy_row).collect()
  }

  async fn update_policy(
    &self,
    policy_id: &str,
    req: UpdateRetentionPolicyRequest,
  ) -> Result<RetentionPolicy> {
    let _uuid = Uuid::parse_str(policy_id)?;

    // For now, use individual update statements for simplicity
    // In production, you'd want to build a dynamic query

    if let Some(name) = &req.name {
      sqlx::query("UPDATE retention_policies SET name = $1 WHERE id = $2")
        .bind(name)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(description) = &req.description {
      sqlx::query("UPDATE retention_policies SET description = $1 WHERE id = $2")
        .bind(description)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(enabled) = req.enabled {
      sqlx::query("UPDATE retention_policies SET enabled = $1 WHERE id = $2")
        .bind(enabled)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(retention_days) = req.retention_days {
      sqlx::query("UPDATE retention_policies SET retention_days = $1 WHERE id = $2")
        .bind(retention_days)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(max_storage_bytes) = req.max_storage_bytes {
      sqlx::query("UPDATE retention_policies SET max_storage_bytes = $1 WHERE id = $2")
        .bind(max_storage_bytes)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(conditions) = &req.conditions {
      let condition_json = serde_json::to_value(conditions)?;
      sqlx::query("UPDATE retention_policies SET condition_json = $1 WHERE id = $2")
        .bind(condition_json)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(enable_tiered_storage) = req.enable_tiered_storage {
      sqlx::query("UPDATE retention_policies SET enable_tiered_storage = $1 WHERE id = $2")
        .bind(enable_tiered_storage)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(cold_storage_after_days) = req.cold_storage_after_days {
      sqlx::query("UPDATE retention_policies SET cold_storage_after_days = $1 WHERE id = $2")
        .bind(cold_storage_after_days)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(cold_storage_path) = &req.cold_storage_path {
      sqlx::query("UPDATE retention_policies SET cold_storage_path = $1 WHERE id = $2")
        .bind(cold_storage_path)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(priority) = req.priority {
      sqlx::query("UPDATE retention_policies SET priority = $1 WHERE id = $2")
        .bind(priority)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }
    if let Some(dry_run) = req.dry_run {
      sqlx::query("UPDATE retention_policies SET dry_run = $1 WHERE id = $2")
        .bind(dry_run)
        .bind(_uuid)
        .execute(&self.pool)
        .await?;
    }

    self
      .get_policy(policy_id)
      .await?
      .ok_or_else(|| anyhow::anyhow!("policy not found"))
  }

  async fn delete_policy(&self, policy_id: &str) -> Result<bool> {
    let _uuid = Uuid::parse_str(policy_id)?;
    let result = sqlx::query("DELETE FROM retention_policies WHERE id = $1")
      .bind(_uuid)
      .execute(&self.pool)
      .await?;

    Ok(result.rows_affected() > 0)
  }

  async fn create_execution(&self, policy_id: &str) -> Result<RetentionExecution> {
    let id = Uuid::new_v4();
    let policy_uuid = Uuid::parse_str(policy_id)?;

    let row = sqlx::query(
      r#"
      INSERT INTO retention_executions (id, policy_id)
      VALUES ($1, $2)
      RETURNING *
      "#,
    )
    .bind(id)
    .bind(policy_uuid)
    .fetch_one(&self.pool)
    .await?;

    Self::map_execution_row(row)
  }

  async fn update_execution(&self, execution: &RetentionExecution) -> Result<()> {
    let uuid = Uuid::parse_str(&execution.id)?;
    let status_str = match execution.status {
      ExecutionStatus::Running => "running",
      ExecutionStatus::Completed => "completed",
      ExecutionStatus::Failed => "failed",
    };

    let completed_at = execution
      .completed_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();

    sqlx::query(
      r#"
      UPDATE retention_executions
      SET status = $1, recordings_scanned = $2, recordings_deleted = $3,
          recordings_moved_to_cold = $4, bytes_freed = $5, bytes_moved = $6,
          completed_at = $7, duration_secs = $8, error_message = $9
      WHERE id = $10
      "#,
    )
    .bind(status_str)
    .bind(execution.recordings_scanned)
    .bind(execution.recordings_deleted)
    .bind(execution.recordings_moved_to_cold)
    .bind(execution.bytes_freed)
    .bind(execution.bytes_moved)
    .bind(completed_at)
    .bind(execution.duration_secs)
    .bind(&execution.error_message)
    .bind(uuid)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn get_execution(&self, execution_id: &str) -> Result<Option<RetentionExecution>> {
    let uuid = Uuid::parse_str(execution_id)?;
    let row = sqlx::query("SELECT * FROM retention_executions WHERE id = $1")
      .bind(uuid)
      .fetch_optional(&self.pool)
      .await?;

    match row {
      Some(r) => Ok(Some(Self::map_execution_row(r)?)),
      None => Ok(None),
    }
  }

  async fn list_executions(&self, policy_id: Option<&str>) -> Result<Vec<RetentionExecution>> {
    let rows = if let Some(pid) = policy_id {
      let uuid = Uuid::parse_str(pid)?;
      sqlx::query(
        "SELECT * FROM retention_executions WHERE policy_id = $1 ORDER BY started_at DESC",
      )
      .bind(uuid)
      .fetch_all(&self.pool)
      .await?
    } else {
      sqlx::query("SELECT * FROM retention_executions ORDER BY started_at DESC")
        .fetch_all(&self.pool)
        .await?
    };

    rows.into_iter().map(Self::map_execution_row).collect()
  }

  async fn create_action(&self, action: &RetentionAction) -> Result<()> {
    let id = Uuid::parse_str(&action.id)?;
    let execution_uuid = Uuid::parse_str(&action.execution_id)?;
    let action_type_str = match action.action_type {
      ActionType::Delete => "delete",
      ActionType::MoveToCold => "move_to_cold",
      ActionType::Skip => "skip",
    };
    let status_str = match action.status {
      ActionStatus::Pending => "pending",
      ActionStatus::Completed => "completed",
      ActionStatus::Failed => "failed",
    };

    let recording_created_at = action
      .recording_created_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();
    let performed_at = action
      .performed_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();

    sqlx::query(
      r#"
      INSERT INTO retention_actions
        (id, execution_id, recording_id, action_type, status, recording_path,
         recording_size_bytes, recording_duration_secs, recording_created_at,
         performed_at, error_message)
      VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
      "#,
    )
    .bind(id)
    .bind(execution_uuid)
    .bind(&action.recording_id)
    .bind(action_type_str)
    .bind(status_str)
    .bind(&action.recording_path)
    .bind(action.recording_size_bytes)
    .bind(action.recording_duration_secs)
    .bind(recording_created_at)
    .bind(performed_at)
    .bind(&action.error_message)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn update_action(&self, action: &RetentionAction) -> Result<()> {
    let uuid = Uuid::parse_str(&action.id)?;
    let status_str = match action.status {
      ActionStatus::Pending => "pending",
      ActionStatus::Completed => "completed",
      ActionStatus::Failed => "failed",
    };

    let performed_at = action
      .performed_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();

    sqlx::query(
      r#"
      UPDATE retention_actions
      SET status = $1, performed_at = $2, error_message = $3
      WHERE id = $4
      "#,
    )
    .bind(status_str)
    .bind(performed_at)
    .bind(&action.error_message)
    .bind(uuid)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn list_actions(&self, execution_id: &str) -> Result<Vec<RetentionAction>> {
    let uuid = Uuid::parse_str(execution_id)?;
    let rows =
      sqlx::query("SELECT * FROM retention_actions WHERE execution_id = $1 ORDER BY created_at")
        .bind(uuid)
        .fetch_all(&self.pool)
        .await?;

    rows.into_iter().map(Self::map_action_row).collect()
  }

  async fn update_storage_stats(&self, stats: &StorageStatistics) -> Result<()> {
    let tenant_uuid = stats
      .tenant_id
      .as_ref()
      .and_then(|s| Uuid::parse_str(s).ok());
    let device_uuid = stats
      .device_id
      .as_ref()
      .and_then(|s| Uuid::parse_str(s).ok());

    let oldest_recording_at = stats
      .oldest_recording_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();
    let newest_recording_at = stats
      .newest_recording_at
      .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
      .flatten();

    sqlx::query(
      r#"
      INSERT INTO storage_statistics
        (tenant_id, device_id, zone, total_recordings, total_bytes,
         oldest_recording_at, newest_recording_at)
      VALUES ($1, $2, $3, $4, $5, $6, $7)
      ON CONFLICT (tenant_id, device_id, zone)
      DO UPDATE SET
        total_recordings = EXCLUDED.total_recordings,
        total_bytes = EXCLUDED.total_bytes,
        oldest_recording_at = EXCLUDED.oldest_recording_at,
        newest_recording_at = EXCLUDED.newest_recording_at,
        calculated_at = NOW()
      "#,
    )
    .bind(tenant_uuid)
    .bind(device_uuid)
    .bind(&stats.zone)
    .bind(stats.total_recordings)
    .bind(stats.total_bytes)
    .bind(oldest_recording_at)
    .bind(newest_recording_at)
    .execute(&self.pool)
    .await?;

    Ok(())
  }

  async fn get_storage_stats(
    &self,
    tenant_id: Option<&str>,
    device_id: Option<&str>,
  ) -> Result<Vec<StorageStatistics>> {
    let rows = match (tenant_id, device_id) {
      (Some(tid), Some(did)) => {
        let tenant_uuid = Uuid::parse_str(tid)?;
        let device_uuid = Uuid::parse_str(did)?;
        sqlx::query(
          "SELECT * FROM storage_statistics WHERE tenant_id = $1 AND device_id = $2 ORDER BY calculated_at DESC",
        )
        .bind(tenant_uuid)
        .bind(device_uuid)
        .fetch_all(&self.pool)
        .await?
      }
      (Some(tid), None) => {
        let tenant_uuid = Uuid::parse_str(tid)?;
        sqlx::query(
          "SELECT * FROM storage_statistics WHERE tenant_id = $1 ORDER BY calculated_at DESC",
        )
        .bind(tenant_uuid)
        .fetch_all(&self.pool)
        .await?
      }
      (None, Some(did)) => {
        let device_uuid = Uuid::parse_str(did)?;
        sqlx::query(
          "SELECT * FROM storage_statistics WHERE device_id = $1 ORDER BY calculated_at DESC",
        )
        .bind(device_uuid)
        .fetch_all(&self.pool)
        .await?
      }
      (None, None) => {
        sqlx::query("SELECT * FROM storage_statistics ORDER BY calculated_at DESC")
          .fetch_all(&self.pool)
          .await?
      }
    };

    rows.into_iter().map(Self::map_stats_row).collect()
  }
}
