use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tracing::{error, info, warn};

use crate::correlation::extract_or_generate_correlation_id;

/// Axum middleware for HTTP request tracing
pub async fn trace_http_request(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = req.method().clone();
    let uri = req.uri().clone();
    let version = req.version();

    // Extract or generate correlation ID
    let correlation_id = extract_or_generate_correlation_id(req.headers());

    // Create a span for this HTTP request
    let span = tracing::info_span!(
        "http_request",
        method = %method,
        uri = %uri,
        version = ?version,
        correlation_id = %correlation_id,
        status = tracing::field::Empty,
        latency_ms = tracing::field::Empty,
    );

    let _enter = span.enter();

    // Process request
    let response = next.run(req).await;

    // Calculate latency
    let latency = start.elapsed();
    let latency_ms = latency.as_millis();
    let status = response.status();

    // Record span fields
    span.record("status", status.as_u16());
    span.record("latency_ms", latency_ms);

    // Log based on status code
    match status.as_u16() {
        200..=299 => {
            info!(
                method = %method,
                uri = %uri,
                status = %status.as_u16(),
                latency_ms = %latency_ms,
                correlation_id = %correlation_id,
                "HTTP request completed"
            );
        }
        400..=499 => {
            warn!(
                method = %method,
                uri = %uri,
                status = %status.as_u16(),
                latency_ms = %latency_ms,
                correlation_id = %correlation_id,
                "HTTP request failed (client error)"
            );
        }
        500..=599 => {
            error!(
                method = %method,
                uri = %uri,
                status = %status.as_u16(),
                latency_ms = %latency_ms,
                correlation_id = %correlation_id,
                "HTTP request failed (server error)"
            );
        }
        _ => {
            info!(
                method = %method,
                uri = %uri,
                status = %status.as_u16(),
                latency_ms = %latency_ms,
                correlation_id = %correlation_id,
                "HTTP request completed"
            );
        }
    }

    // Add correlation ID to response headers
    let mut response = response;
    response.headers_mut().insert(
        "x-correlation-id",
        correlation_id
            .parse()
            .unwrap_or_else(|_| "invalid".parse().expect("BUG: header value should be valid")),
    );

    response
}

/// Create a traced HTTP client with correlation ID propagation
pub fn create_traced_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent("quadrant-vms/0.1.0")
        .build()
        .expect("BUG: HTTP client should build successfully")
}

/// Helper to add correlation ID to outgoing HTTP requests
pub fn add_correlation_id_header(
    mut request: reqwest::RequestBuilder,
    correlation_id: &str,
) -> reqwest::RequestBuilder {
    request = request.header("x-correlation-id", correlation_id);
    request = request.header("x-request-id", correlation_id);
    request
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};
    use tower::ServiceExt;

    async fn test_handler() -> &'static str {
        "Hello, World!"
    }

    #[tokio::test]
    async fn test_trace_http_request_middleware() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum::middleware::from_fn(trace_http_request));

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .expect("BUG: request should build successfully");

        let response = app.oneshot(request).await.expect("BUG: request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-correlation-id"));
    }

    #[tokio::test]
    async fn test_correlation_id_propagation() {
        let app = Router::new()
            .route("/test", get(test_handler))
            .layer(axum::middleware::from_fn(trace_http_request));

        let test_correlation_id = "test-correlation-id-123";

        let request = Request::builder()
            .uri("/test")
            .header("x-correlation-id", test_correlation_id)
            .body(Body::empty())
            .expect("BUG: request should build successfully");

        let response = app.oneshot(request).await.expect("BUG: request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        let correlation_id = response
            .headers()
            .get("x-correlation-id")
            .expect("BUG: correlation ID header should exist")
            .to_str()
            .expect("BUG: header should be valid string");
        assert_eq!(correlation_id, test_correlation_id);
    }
}
