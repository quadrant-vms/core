use telemetry::{TracingBackend, TracingConfig};

#[test]
fn test_tracing_config_builder() {
    let config = TracingConfig::new("test-service")
        .with_version("1.0.0")
        .with_environment("test")
        .with_sample_rate(0.5)
        .with_backend(TracingBackend::None);

    assert_eq!(config.service_name, "test-service");
    assert_eq!(config.service_version, "1.0.0");
    assert_eq!(config.environment, "test");
    assert_eq!(config.sample_rate, 0.5);
}

#[test]
fn test_tracing_backend_none() {
    let config = TracingConfig::new("test-service").with_backend(TracingBackend::None);

    // This should succeed without error and not initialize any tracing backend
    let result = telemetry::init_distributed_tracing(config);
    assert!(result.is_ok());
}

#[test]
fn test_sample_rate_clamping() {
    let config = TracingConfig::new("test")
        .with_sample_rate(1.5) // Above max
        .with_backend(TracingBackend::None);
    assert_eq!(config.sample_rate, 1.0);

    let config = TracingConfig::new("test")
        .with_sample_rate(-0.5) // Below min
        .with_backend(TracingBackend::None);
    assert_eq!(config.sample_rate, 0.0);

    let config = TracingConfig::new("test")
        .with_sample_rate(0.5) // Within range
        .with_backend(TracingBackend::None);
    assert_eq!(config.sample_rate, 0.5);
}

#[test]
fn test_tracing_backend_from_env() {
    // Clean environment
    std::env::remove_var("TRACING_BACKEND");
    std::env::remove_var("OTLP_ENDPOINT");

    // Test default (None)
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

#[tokio::test]
async fn test_http_tracing_middleware() {
    use axum::{body::Body, extract::Request, routing::get, Router};
    use telemetry::trace_http_request;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(trace_http_request));

    let request = Request::builder()
        .uri("/test")
        .body(Body::empty())
        .expect("BUG: request should build successfully");

    let response = app
        .oneshot(request)
        .await
        .expect("BUG: request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert!(response.headers().contains_key("x-correlation-id"));
}

#[tokio::test]
async fn test_correlation_id_propagation() {
    use axum::{body::Body, extract::Request, routing::get, Router};
    use telemetry::trace_http_request;
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    let app = Router::new()
        .route("/test", get(test_handler))
        .layer(axum::middleware::from_fn(trace_http_request));

    let test_correlation_id = "test-correlation-id-123";

    let request = Request::builder()
        .uri("/test")
        .header("x-correlation-id", test_correlation_id)
        .body(Body::empty())
        .expect("BUG: request should build successfully");

    let response = app
        .oneshot(request)
        .await
        .expect("BUG: request should succeed");

    assert_eq!(response.status(), axum::http::StatusCode::OK);

    let correlation_id = response
        .headers()
        .get("x-correlation-id")
        .expect("BUG: correlation ID header should exist")
        .to_str()
        .expect("BUG: header should be valid string");

    assert_eq!(correlation_id, test_correlation_id);
}
