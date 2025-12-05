use axum::{routing::get, routing::post, Router};
use telemetry::init as telemetry_init;
use tokio::net::TcpListener;
use tracing::info;

mod api;
mod recording;
mod storage;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  telemetry_init();

  let app = Router::new()
    .route("/healthz", get(api::healthz))
    .route("/recordings", get(api::list_recordings))
    .route("/start", post(api::start_recording))
    .route("/stop", post(api::stop_recording));

  let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8081));
  let listener = TcpListener::bind(addr).await?;
  info!(%addr, "recorder-node started");
  axum::serve(listener, app).await?;
  Ok(())
}
