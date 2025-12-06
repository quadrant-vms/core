use axum::{routing::get, routing::post, Router};
use std::sync::Arc;
use telemetry::init as telemetry_init;
use tokio::net::TcpListener;
use tracing::info;

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
