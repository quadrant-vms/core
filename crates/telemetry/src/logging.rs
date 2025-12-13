use serde::{Deserialize, Serialize};
use std::env;
use std::io;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Log output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogFormat {
    /// Human-readable format (default for development)
    Pretty,
    /// Compact text format
    Compact,
    /// JSON format for log aggregation systems
    Json,
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::Pretty
    }
}

impl LogFormat {
    /// Parse log format from environment variable
    pub fn from_env() -> Self {
        match env::var("LOG_FORMAT")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "json" => Self::Json,
            "compact" => Self::Compact,
            "pretty" => Self::Pretty,
            _ => Self::default(),
        }
    }
}

/// Configuration for structured logging
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Log output format (pretty/compact/json)
    pub format: LogFormat,
    /// Service name (e.g., "coordinator", "stream-node")
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Node ID for distributed systems
    pub node_id: Option<String>,
    /// Environment (dev/staging/production)
    pub environment: String,
    /// Enable span events (enter/exit/close)
    pub enable_span_events: bool,
    /// Log to file in addition to stdout
    pub log_to_file: bool,
    /// Log file directory
    pub log_dir: Option<String>,
}

impl LogConfig {
    /// Create a new log configuration with sensible defaults
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            format: LogFormat::from_env(),
            service_name: service_name.into(),
            service_version: env::var("SERVICE_VERSION").unwrap_or_else(|_| "0.1.0".to_string()),
            node_id: env::var("NODE_ID").ok(),
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            enable_span_events: env::var("LOG_SPAN_EVENTS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            log_to_file: env::var("LOG_TO_FILE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(false),
            log_dir: env::var("LOG_DIR").ok(),
        }
    }

    /// Set the log format
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the service version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    /// Set the node ID
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    /// Set the environment
    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    /// Enable span events (enter/exit/close)
    pub fn with_span_events(mut self, enable: bool) -> Self {
        self.enable_span_events = enable;
        self
    }

    /// Enable logging to file
    pub fn with_file_logging(mut self, log_dir: impl Into<String>) -> Self {
        self.log_to_file = true;
        self.log_dir = Some(log_dir.into());
        self
    }
}

/// Initialize structured logging with the given configuration
pub fn init_structured_logging(config: LogConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("hyper=warn".parse().expect("valid directive"))
        .add_directive("tokio=warn".parse().expect("valid directive"))
        .add_directive("sqlx=warn".parse().expect("valid directive"));

    // Store config values for logging initialization message
    let service_name = config.service_name.clone();
    let service_version = config.service_version.clone();
    let environment = config.environment.clone();
    let format = config.format;
    let enable_span_events = config.enable_span_events;

    // Create base subscriber
    let registry = tracing_subscriber::registry().with(filter);

    match config.format {
        LogFormat::Json => {
            let span_events = if enable_span_events {
                FmtSpan::NEW | FmtSpan::CLOSE
            } else {
                FmtSpan::NONE
            };

            let json_layer = fmt::layer()
                .json()
                .with_span_events(span_events)
                .with_current_span(true)
                .with_span_list(true)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_writer(io::stdout);

            if config.log_to_file {
                if let Some(log_dir) = config.log_dir {
                    let file_span_events = if enable_span_events {
                        FmtSpan::NEW | FmtSpan::CLOSE
                    } else {
                        FmtSpan::NONE
                    };

                    let file_appender = tracing_appender::rolling::daily(log_dir, "app.log");
                    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
                    let file_layer = fmt::layer()
                        .json()
                        .with_span_events(file_span_events)
                        .with_writer(non_blocking);

                    registry.with(json_layer).with(file_layer).init();

                    // Log initialization with context
                    tracing::info!(
                        service.name = %service_name,
                        service.version = %service_version,
                        environment = %environment,
                        format = ?format,
                        "structured logging initialized"
                    );
                    return;
                }
            }

            registry.with(json_layer).init();
        }
        LogFormat::Compact => {
            let span_events = if enable_span_events {
                FmtSpan::NEW | FmtSpan::CLOSE
            } else {
                FmtSpan::NONE
            };

            let compact_layer = fmt::layer()
                .compact()
                .with_span_events(span_events)
                .with_target(true)
                .with_thread_ids(false);

            registry.with(compact_layer).init();
        }
        LogFormat::Pretty => {
            let span_events = if enable_span_events {
                FmtSpan::NEW | FmtSpan::CLOSE
            } else {
                FmtSpan::NONE
            };

            let pretty_layer = fmt::layer()
                .pretty()
                .with_span_events(span_events)
                .with_target(true)
                .with_thread_ids(false)
                .with_line_number(true);

            registry.with(pretty_layer).init();
        }
    }

    // Log initialization with context
    tracing::info!(
        service.name = %service_name,
        service.version = %service_version,
        environment = %environment,
        format = ?format,
        "structured logging initialized"
    );
}

/// Initialize logging with simple defaults (backwards compatible)
pub fn init() {
    let config = LogConfig::new("unknown-service");
    init_structured_logging(config);
}

/// Initialize logging with service name
pub fn init_with_service(service_name: impl Into<String>) {
    let config = LogConfig::new(service_name);
    init_structured_logging(config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_env() {
        // Default is Pretty
        std::env::remove_var("LOG_FORMAT");
        assert_eq!(LogFormat::from_env(), LogFormat::Pretty);

        // Test JSON
        std::env::set_var("LOG_FORMAT", "json");
        assert_eq!(LogFormat::from_env(), LogFormat::Json);

        // Test Compact
        std::env::set_var("LOG_FORMAT", "compact");
        assert_eq!(LogFormat::from_env(), LogFormat::Compact);

        // Test Pretty
        std::env::set_var("LOG_FORMAT", "pretty");
        assert_eq!(LogFormat::from_env(), LogFormat::Pretty);

        // Cleanup
        std::env::remove_var("LOG_FORMAT");
    }

    #[test]
    fn test_log_config_builder() {
        let config = LogConfig::new("test-service")
            .with_version("1.0.0")
            .with_environment("production")
            .with_node_id("node-1")
            .with_format(LogFormat::Json)
            .with_span_events(true);

        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.service_version, "1.0.0");
        assert_eq!(config.environment, "production");
        assert_eq!(config.node_id, Some("node-1".to_string()));
        assert_eq!(config.format, LogFormat::Json);
        assert!(config.enable_span_events);
    }

    #[test]
    fn test_log_config_from_env() {
        // Clear all env vars first to avoid test pollution
        std::env::remove_var("SERVICE_VERSION");
        std::env::remove_var("NODE_ID");
        std::env::remove_var("ENVIRONMENT");
        std::env::remove_var("LOG_FORMAT");

        // Set test values
        std::env::set_var("SERVICE_VERSION", "2.0.0");
        std::env::set_var("NODE_ID", "test-node");
        std::env::set_var("ENVIRONMENT", "staging");
        std::env::set_var("LOG_FORMAT", "json");

        let config = LogConfig::new("test-service");

        assert_eq!(config.service_version, "2.0.0");
        assert_eq!(config.node_id, Some("test-node".to_string()));
        assert_eq!(config.environment, "staging");
        assert_eq!(config.format, LogFormat::Json);

        // Cleanup
        std::env::remove_var("SERVICE_VERSION");
        std::env::remove_var("NODE_ID");
        std::env::remove_var("ENVIRONMENT");
        std::env::remove_var("LOG_FORMAT");
    }
}
