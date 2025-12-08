use admin_gateway::{
  config::GatewayConfig,
  coordinator::{CoordinatorClient, HttpCoordinatorClient},
  routes,
  state::AppState,
  worker::{HttpRecorderClient, HttpWorkerClient, RecorderClient, WorkerClient},
};
use anyhow::Result;
use common::state_store::StateStore;
use common::state_store_client::StateStoreClient;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<()> {
  telemetry::init();

  let config = GatewayConfig::from_env()?;
  let coordinator: Arc<dyn CoordinatorClient> = Arc::new(HttpCoordinatorClient::new(
    config.coordinator_base_url.clone(),
  )?);
  let worker: Arc<dyn WorkerClient> =
    Arc::new(HttpWorkerClient::new(config.worker_base_url.clone())?);
  let recorder: Arc<dyn RecorderClient> =
    Arc::new(HttpRecorderClient::new(config.recorder_base_url.clone())?);

  // Initialize StateStore client (optional, enabled via env var)
  let state_store_enabled = std::env::var("ENABLE_STATE_STORE")
    .unwrap_or_else(|_| "false".to_string())
    .to_lowercase() == "true";

  let state = if state_store_enabled {
    let state_store_client: Arc<dyn StateStore> = Arc::new(StateStoreClient::new(
      config.coordinator_base_url.to_string(),
    ));
    let state = AppState::with_state_store(
      config.clone(),
      coordinator,
      worker,
      recorder,
      state_store_client,
    );

    // Bootstrap: restore state from StateStore
    if let Err(e) = state.bootstrap().await {
      warn!(error = %e, "failed to bootstrap state from StateStore");
    }

    state
  } else {
    AppState::new(config.clone(), coordinator, worker, recorder)
  };

  let app = routes::router(state.clone());
  let listener = TcpListener::bind(config.bind_addr).await?;

  info!(
    addr = %config.bind_addr,
    node_id = %config.node_id,
    state_store = state_store_enabled,
    "admin-gateway listening"
  );

  axum::serve(listener, app.into_make_service())
    .with_graceful_shutdown(shutdown_signal())
    .await?;

  Ok(())
}

async fn shutdown_signal() {
  let ctrl_c = async {
    let _ = tokio::signal::ctrl_c().await;
  };

  #[cfg(unix)]
  let terminate = async {
    use tokio::signal::unix::{SignalKind, signal};
    if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
      let _ = sigterm.recv().await;
    }
  };

  #[cfg(not(unix))]
  let terminate = std::future::pending::<()>();

  tokio::select! {
      _ = ctrl_c => {},
      _ = terminate => {},
  }

  info!("shutdown signal received");
}
