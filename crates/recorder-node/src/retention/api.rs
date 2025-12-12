use axum::{
  extract::{Path, State},
  http::StatusCode,
  Json,
};
use common::retention::*;
use std::sync::Arc;
use tracing::{error, info};

use super::executor::RetentionExecutor;
use super::store::RetentionStore;

pub struct RetentionApiState {
  pub store: Arc<dyn RetentionStore>,
  pub executor: Arc<RetentionExecutor>,
}

/// Create a new retention policy
pub async fn create_policy(
  State(state): State<Arc<RetentionApiState>>,
  Json(req): Json<CreateRetentionPolicyRequest>,
) -> Result<Json<RetentionPolicy>, StatusCode> {
  info!(
    policy_name = %req.name,
    policy_type = ?req.policy_type,
    "creating retention policy"
  );

  match state.store.create_policy(req).await {
    Ok(policy) => {
      info!(policy_id = %policy.id, "retention policy created");
      Ok(Json(policy))
    }
    Err(e) => {
      error!(error = %e, "failed to create retention policy");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Get a specific retention policy
pub async fn get_policy(
  State(state): State<Arc<RetentionApiState>>,
  Path(policy_id): Path<String>,
) -> Result<Json<RetentionPolicy>, StatusCode> {
  match state.store.get_policy(&policy_id).await {
    Ok(Some(policy)) => Ok(Json(policy)),
    Ok(None) => Err(StatusCode::NOT_FOUND),
    Err(e) => {
      error!(error = %e, "failed to get retention policy");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// List all retention policies
pub async fn list_policies(
  State(state): State<Arc<RetentionApiState>>,
) -> Result<Json<ListPoliciesResponse>, StatusCode> {
  match state.store.list_policies(None).await {
    Ok(policies) => Ok(Json(ListPoliciesResponse { policies })),
    Err(e) => {
      error!(error = %e, "failed to list retention policies");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Update a retention policy
pub async fn update_policy(
  State(state): State<Arc<RetentionApiState>>,
  Path(policy_id): Path<String>,
  Json(req): Json<UpdateRetentionPolicyRequest>,
) -> Result<Json<RetentionPolicy>, StatusCode> {
  info!(policy_id = %policy_id, "updating retention policy");

  match state.store.update_policy(&policy_id, req).await {
    Ok(policy) => {
      info!(policy_id = %policy.id, "retention policy updated");
      Ok(Json(policy))
    }
    Err(e) => {
      error!(policy_id = %policy_id, error = %e, "failed to update retention policy");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Delete a retention policy
pub async fn delete_policy(
  State(state): State<Arc<RetentionApiState>>,
  Path(policy_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
  info!(policy_id = %policy_id, "deleting retention policy");

  match state.store.delete_policy(&policy_id).await {
    Ok(true) => {
      info!(policy_id = %policy_id, "retention policy deleted");
      Ok(StatusCode::NO_CONTENT)
    }
    Ok(false) => Err(StatusCode::NOT_FOUND),
    Err(e) => {
      error!(policy_id = %policy_id, error = %e, "failed to delete retention policy");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Execute a specific retention policy
pub async fn execute_policy(
  State(state): State<Arc<RetentionApiState>>,
  Path(policy_id): Path<String>,
) -> Result<Json<ExecutePolicyResponse>, StatusCode> {
  info!(policy_id = %policy_id, "executing retention policy");

  match state.executor.execute_policy(&policy_id).await {
    Ok(execution) => {
      info!(
        policy_id = %policy_id,
        execution_id = %execution.id,
        "retention policy executed"
      );
      Ok(Json(ExecutePolicyResponse {
        execution_id: execution.id,
        message: "Retention policy executed successfully".to_string(),
      }))
    }
    Err(e) => {
      error!(policy_id = %policy_id, error = %e, "failed to execute retention policy");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Execute all enabled retention policies
pub async fn execute_all_policies(
  State(state): State<Arc<RetentionApiState>>,
) -> Result<Json<ListExecutionsResponse>, StatusCode> {
  info!("executing all enabled retention policies");

  match state.executor.execute_all_policies().await {
    Ok(executions) => {
      info!(
        execution_count = executions.len(),
        "all retention policies executed"
      );
      Ok(Json(ListExecutionsResponse { executions }))
    }
    Err(e) => {
      error!(error = %e, "failed to execute retention policies");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Get a specific retention execution
pub async fn get_execution(
  State(state): State<Arc<RetentionApiState>>,
  Path(execution_id): Path<String>,
) -> Result<Json<RetentionExecution>, StatusCode> {
  match state.store.get_execution(&execution_id).await {
    Ok(Some(execution)) => Ok(Json(execution)),
    Ok(None) => Err(StatusCode::NOT_FOUND),
    Err(e) => {
      error!(error = %e, "failed to get retention execution");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// List retention executions for a policy
pub async fn list_executions(
  State(state): State<Arc<RetentionApiState>>,
  Path(policy_id): Path<String>,
) -> Result<Json<ListExecutionsResponse>, StatusCode> {
  match state.store.list_executions(Some(&policy_id)).await {
    Ok(executions) => Ok(Json(ListExecutionsResponse { executions })),
    Err(e) => {
      error!(error = %e, "failed to list retention executions");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// List all retention executions
pub async fn list_all_executions(
  State(state): State<Arc<RetentionApiState>>,
) -> Result<Json<ListExecutionsResponse>, StatusCode> {
  match state.store.list_executions(None).await {
    Ok(executions) => Ok(Json(ListExecutionsResponse { executions })),
    Err(e) => {
      error!(error = %e, "failed to list retention executions");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// List retention actions for an execution
pub async fn list_actions(
  State(state): State<Arc<RetentionApiState>>,
  Path(execution_id): Path<String>,
) -> Result<Json<ListActionsResponse>, StatusCode> {
  match state.store.list_actions(&execution_id).await {
    Ok(actions) => Ok(Json(ListActionsResponse { actions })),
    Err(e) => {
      error!(error = %e, "failed to list retention actions");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}

/// Get storage statistics
pub async fn get_storage_stats(
  State(state): State<Arc<RetentionApiState>>,
) -> Result<Json<StorageStatsResponse>, StatusCode> {
  match state.store.get_storage_stats(None, None).await {
    Ok(statistics) => Ok(Json(StorageStatsResponse { statistics })),
    Err(e) => {
      error!(error = %e, "failed to get storage statistics");
      Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
  }
}
