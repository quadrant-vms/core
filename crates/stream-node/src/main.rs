use axum::{routing::get, Router};
use telemetry::init as telemetry_init;
use tokio::net::TcpListener;
use tracing::info;

mod api;
mod stream;
mod storage;
mod metrics;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry_init();

    let app = Router::new()
        .route("/healthz", get(api::healthz))
        .route("/streams", get(api::list_streams))
        .route("/start", get(api::start_stream_api))
        .route("/stop",    get(api::stop_stream_api))
        .route("/metrics", get(|| async { metrics::render() }));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "stream-node started");
    axum::serve(listener, app).await?;
    Ok(())
}