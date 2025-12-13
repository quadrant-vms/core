use axum::{middleware, routing::get, Router};
use telemetry::{trace_http_request, TracingConfig};
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tracing::info;

mod api;
mod compat;
mod metrics;
mod storage;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Initialize distributed tracing (falls back to regular logging if disabled)
  let tracing_config = TracingConfig::new("stream-node")
      .with_version(env!("CARGO_PKG_VERSION"));

  if let Err(e) = telemetry::init_distributed_tracing(tracing_config) {
      // Fallback to structured logging if distributed tracing fails
      tracing::warn!("Failed to initialize distributed tracing: {}, falling back to structured logging", e);
      let log_config = telemetry::LogConfig::new("stream-node")
          .with_version(env!("CARGO_PKG_VERSION"));
      telemetry::init_structured_logging(log_config);
  }

  let app = Router::new()
    .route("/healthz", get(api::healthz))
    .route("/streams", get(api::list_streams))
    .route("/start", get(api::start_stream_api))
    .route("/stop", get(api::stop_stream_api))
    .route("/metrics", get(|| async { metrics::render() }))
    .layer(
      ServiceBuilder::new()
        .layer(middleware::from_fn(trace_http_request))
    );

  let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 8080));
  let listener = TcpListener::bind(addr).await?;
  info!(%addr, "stream-node started");
  axum::serve(listener, app).await?;

  // Shutdown tracing provider
  telemetry::shutdown_tracing();

  Ok(())
}
