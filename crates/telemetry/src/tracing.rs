use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Tracing backend configuration
#[derive(Debug, Clone)]
pub enum TracingBackend {
    /// No distributed tracing (default)
    None,
    /// OTLP backend (OpenTelemetry Protocol)
    /// Supports both Jaeger (via OTLP) and other OTLP collectors
    Otlp {
        /// OTLP endpoint (e.g., "http://localhost:4317" for gRPC)
        endpoint: String,
    },
}

impl TracingBackend {
    /// Parse tracing backend from environment variables
    pub fn from_env() -> Self {
        if env::var("TRACING_BACKEND")
            .unwrap_or_default()
            .to_lowercase()
            == "otlp"
        {
            let endpoint = env::var("OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://localhost:4317".to_string());
            Self::Otlp { endpoint }
        } else {
            Self::None
        }
    }
}

/// Configuration for distributed tracing
#[derive(Debug, Clone)]
pub struct TracingConfig {
    /// Service name (e.g., "coordinator", "stream-node")
    pub service_name: String,
    /// Service version
    pub service_version: String,
    /// Tracing backend configuration
    pub backend: TracingBackend,
    /// Sample rate (0.0 to 1.0)
    pub sample_rate: f64,
    /// Environment (dev/staging/production)
    pub environment: String,
    /// Node ID for distributed systems
    pub node_id: Option<String>,
}

impl TracingConfig {
    /// Create a new tracing configuration
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            service_version: env::var("SERVICE_VERSION").unwrap_or_else(|_| "0.1.0".to_string()),
            backend: TracingBackend::from_env(),
            sample_rate: env::var("TRACE_SAMPLE_RATE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0), // Default: trace everything
            environment: env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string()),
            node_id: env::var("NODE_ID").ok(),
        }
    }

    /// Set the service version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.service_version = version.into();
        self
    }

    /// Set the tracing backend
    pub fn with_backend(mut self, backend: TracingBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Set the sample rate
    pub fn with_sample_rate(mut self, rate: f64) -> Self {
        self.sample_rate = rate.clamp(0.0, 1.0);
        self
    }

    /// Set the environment
    pub fn with_environment(mut self, environment: impl Into<String>) -> Self {
        self.environment = environment.into();
        self
    }

    /// Set the node ID
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }
}

/// Initialize distributed tracing with the given configuration
pub fn init_distributed_tracing(config: TracingConfig) -> anyhow::Result<()> {
    // Build resource attributes
    let mut resource_attrs = vec![
        KeyValue::new(SERVICE_NAME, config.service_name.clone()),
        KeyValue::new(SERVICE_VERSION, config.service_version.clone()),
        KeyValue::new("environment", config.environment.clone()),
    ];

    if let Some(node_id) = &config.node_id {
        resource_attrs.push(KeyValue::new("node.id", node_id.clone()));
    }

    let resource = Resource::new(resource_attrs);

    // Create tracer provider based on backend
    let tracer = match &config.backend {
        TracingBackend::None => {
            tracing::info!("Distributed tracing disabled");
            return Ok(());
        }
        TracingBackend::Otlp { endpoint } => {
            tracing::info!(endpoint = %endpoint, "Initializing OTLP tracing backend");

            let tracer_provider = TracerProvider::builder()
                .with_batch_exporter(
                    opentelemetry_otlp::SpanExporter::builder()
                        .with_tonic()
                        .with_endpoint(endpoint.clone())
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to create OTLP exporter: {}", e))?,
                    runtime::Tokio,
                )
                .with_sampler(Sampler::TraceIdRatioBased(config.sample_rate))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource)
                .build();

            global::set_tracer_provider(tracer_provider.clone());
            tracer_provider.tracer(config.service_name.clone())
        }
    };

    // Create OpenTelemetry tracing layer
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // Create filter
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"))
        .add_directive("hyper=warn".parse().expect("valid directive"))
        .add_directive("tokio=warn".parse().expect("valid directive"))
        .add_directive("sqlx=warn".parse().expect("valid directive"));

    // Create fmt layer for console output
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_line_number(true);

    // Initialize subscriber with both layers
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(telemetry_layer)
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize tracing subscriber: {}", e))?;

    tracing::info!(
        service.name = %config.service_name,
        service.version = %config.service_version,
        environment = %config.environment,
        sample_rate = %config.sample_rate,
        "Distributed tracing initialized"
    );

    Ok(())
}

/// Shutdown the global tracer provider gracefully
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_backend_from_env() {
        // Default is None
        std::env::remove_var("TRACING_BACKEND");
        matches!(TracingBackend::from_env(), TracingBackend::None);

        // Test OTLP
        std::env::set_var("TRACING_BACKEND", "otlp");
        std::env::set_var("OTLP_ENDPOINT", "http://localhost:4317");
        if let TracingBackend::Otlp { endpoint } = TracingBackend::from_env() {
            assert_eq!(endpoint, "http://localhost:4317");
        } else {
            panic!("Expected OTLP backend");
        }

        // Cleanup
        std::env::remove_var("TRACING_BACKEND");
        std::env::remove_var("OTLP_ENDPOINT");
    }

    #[test]
    fn test_tracing_config_builder() {
        let config = TracingConfig::new("test-service")
            .with_version("1.0.0")
            .with_environment("production")
            .with_node_id("node-1")
            .with_sample_rate(0.5)
            .with_backend(TracingBackend::Otlp {
                endpoint: "http://localhost:4317".to_string(),
            });

        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.service_version, "1.0.0");
        assert_eq!(config.environment, "production");
        assert_eq!(config.node_id, Some("node-1".to_string()));
        assert_eq!(config.sample_rate, 0.5);
    }

    #[test]
    fn test_sample_rate_clamping() {
        let config = TracingConfig::new("test").with_sample_rate(1.5);
        assert_eq!(config.sample_rate, 1.0);

        let config = TracingConfig::new("test").with_sample_rate(-0.5);
        assert_eq!(config.sample_rate, 0.0);
    }
}
