use tracing_subscriber::{fmt, EnvFilter};

pub mod correlation;
pub mod http_tracing;
pub mod logging;
pub mod metrics;
pub mod tracing;

// Re-export commonly used items
pub use correlation::{CorrelationId, CorrelationIdLayer, X_CORRELATION_ID, X_REQUEST_ID};
pub use http_tracing::{add_correlation_id_header, create_traced_client, trace_http_request};
pub use logging::{init_structured_logging, init_with_service, LogConfig, LogFormat};
pub use tracing::{init_distributed_tracing, shutdown_tracing, TracingBackend, TracingConfig};

/// Legacy init function for backwards compatibility
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
