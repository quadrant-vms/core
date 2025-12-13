use tracing_subscriber::{fmt, EnvFilter};

pub mod correlation;
pub mod logging;
pub mod metrics;

// Re-export commonly used items
pub use correlation::{CorrelationId, CorrelationIdLayer, X_CORRELATION_ID, X_REQUEST_ID};
pub use logging::{init_structured_logging, init_with_service, LogConfig, LogFormat};

/// Legacy init function for backwards compatibility
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}
