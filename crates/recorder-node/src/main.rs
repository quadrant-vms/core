use axum::{routing::get, routing::post, Router};
use common::state_store::StateStore;
use common::state_store_client::StateStoreClient;
use std::sync::Arc;
use telemetry::init as telemetry_init;
use tokio::net::TcpListener;
use tracing::{info, warn};

mod api;
mod coordinator;
mod recording;
mod storage;

use coordinator::HttpCoordinatorClient;
use recording::manager::RECORDING_MANAGER;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  telemetry_init();

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

  let app = Router::new()
    .route("/healthz", get(api::healthz))
    .route("/metrics", get(|| async {
      telemetry::metrics::encode_metrics().unwrap_or_else(|e| format!("Error: {}", e))
    }))
    .route("/recordings", get(api::list_recordings))
    .route("/start", post(api::start_recording))
    .route("/stop", post(api::stop_recording));

  let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8081));
  let listener = TcpListener::bind(addr).await?;
  info!(%addr, "recorder-node started");
  axum::serve(listener, app).await?;
  Ok(())
}
