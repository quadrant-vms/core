use axum::{middleware, routing::get, routing::post, routing::delete, routing::put, Router};
use common::state_store::StateStore;
use common::state_store_client::StateStoreClient;
use std::sync::Arc;
use telemetry::{trace_http_request, TracingConfig};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tracing::{info, warn};

mod api;
mod coordinator;
mod recording;
mod retention;
mod storage;

use coordinator::HttpCoordinatorClient;
use recording::manager::RECORDING_MANAGER;
use retention::{PostgresRetentionStore, RetentionExecutor};
use retention::api::RetentionApiState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Initialize distributed tracing (falls back to regular logging if disabled)
  let tracing_config = TracingConfig::new("recorder-node")
      .with_version(env!("CARGO_PKG_VERSION"));

  if let Err(e) = telemetry::init_distributed_tracing(tracing_config) {
      // Fallback to structured logging if distributed tracing fails
      tracing::warn!("Failed to initialize distributed tracing: {}, falling back to structured logging", e);
      let log_config = telemetry::LogConfig::new("recorder-node")
          .with_version(env!("CARGO_PKG_VERSION"));
      telemetry::init_structured_logging(log_config);
  }

  // Initialize coordinator client if configured
  if let Ok(coordinator_url) = std::env::var("COORDINATOR_URL") {
    let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| "recorder-node".to_string());
    info!(coordinator_url = %coordinator_url, node_id = %node_id, "initializing coordinator client");

    let base = reqwest::Url::parse(&coordinator_url)?;
    let client = Arc::new(HttpCoordinatorClient::new(base)?);
    RECORDING_MANAGER.set_coordinator(client, node_id).await;

    // Initialize StateStore client if enabled
    let state_store_enabled = std::env::var("ENABLE_STATE_STORE")
      .unwrap_or_else(|_| "false".to_string())
      .to_lowercase() == "true";

    if state_store_enabled {
      let state_store_client: Arc<dyn StateStore> = Arc::new(StateStoreClient::new(coordinator_url));
      RECORDING_MANAGER.set_state_store(state_store_client).await;

      // Bootstrap: restore state from StateStore
      if let Err(e) = RECORDING_MANAGER.bootstrap().await {
        warn!(error = %e, "failed to bootstrap state from StateStore");
      } else {
        info!("state store enabled and bootstrapped");
      }
    }
  } else {
    info!("COORDINATOR_URL not set, running without lease management");
  }

  let mut app = Router::new()
    .route("/healthz", get(api::healthz))
    .route("/metrics", get(|| async {
      telemetry::metrics::encode_metrics().unwrap_or_else(|e| format!("Error: {}", e))
    }))
    .route("/recordings", get(api::list_recordings))
    .route("/start", post(api::start_recording))
    .route("/stop", post(api::stop_recording))
    .route("/thumbnail", get(api::get_thumbnail))
    .route("/thumbnail/grid", get(api::get_thumbnail_grid));

  // Initialize retention system if DATABASE_URL is set
  if let Ok(database_url) = std::env::var("DATABASE_URL") {
    info!("initializing retention system with PostgreSQL backend");

    let recording_storage_root = std::env::var("RECORDING_STORAGE_ROOT")
      .unwrap_or_else(|_| "./data/recordings".to_string());

    // Connect to database
    let pool = sqlx::postgres::PgPoolOptions::new()
      .max_connections(5)
      .connect(&database_url)
      .await?;

    // Run migrations (commented out - run migrations manually)
    // info!("running retention database migrations");
    // sqlx::migrate!()
    //   .run(&pool)
    //   .await?;

    // Initialize retention store and executor
    let retention_store = Arc::new(PostgresRetentionStore::new(pool));
    let retention_executor = Arc::new(RetentionExecutor::new(
      Arc::clone(&retention_store) as Arc<dyn retention::store::RetentionStore>,
      recording_storage_root,
    ));

    let retention_state = Arc::new(RetentionApiState {
      store: Arc::clone(&retention_store) as Arc<dyn retention::store::RetentionStore>,
      executor: retention_executor,
    });

    // Add retention routes
    let retention_routes = Router::new()
      .route("/v1/retention/policies", post(retention::api::create_policy))
      .route("/v1/retention/policies", get(retention::api::list_policies))
      .route("/v1/retention/policies/:policy_id", get(retention::api::get_policy))
      .route("/v1/retention/policies/:policy_id", put(retention::api::update_policy))
      .route("/v1/retention/policies/:policy_id", delete(retention::api::delete_policy))
      .route("/v1/retention/policies/:policy_id/execute", post(retention::api::execute_policy))
      .route("/v1/retention/execute", post(retention::api::execute_all_policies))
      .route("/v1/retention/executions", get(retention::api::list_all_executions))
      .route("/v1/retention/executions/:execution_id", get(retention::api::get_execution))
      .route("/v1/retention/policies/:policy_id/executions", get(retention::api::list_executions))
      .route("/v1/retention/executions/:execution_id/actions", get(retention::api::list_actions))
      .route("/v1/retention/storage/stats", get(retention::api::get_storage_stats))
      .with_state(retention_state);

    app = app.merge(retention_routes);
    info!("retention system initialized successfully");
  } else {
    info!("DATABASE_URL not set, retention system disabled");
  }

  // Add HTTP tracing middleware
  let app = app.layer(
    ServiceBuilder::new()
      .layer(middleware::from_fn(trace_http_request))
  );

  let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8085));
  let listener = TcpListener::bind(addr).await?;
  info!(%addr, "recorder-node started");
  axum::serve(listener, app).await?;

  // Shutdown tracing provider
  telemetry::shutdown_tracing();

  Ok(())
}
