use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

/// HTTP header name for correlation ID
pub const X_CORRELATION_ID: &str = "x-correlation-id";

/// HTTP header name for request ID (same as correlation ID)
pub const X_REQUEST_ID: &str = "x-request-id";

/// Generate a new correlation ID
pub fn generate_correlation_id() -> String {
    Uuid::new_v4().to_string()
}

/// Extract correlation ID from HTTP headers or generate a new one
pub fn extract_or_generate_correlation_id(
    headers: &axum::http::HeaderMap,
) -> String {
    headers
        .get(X_CORRELATION_ID)
        .or_else(|| headers.get(X_REQUEST_ID))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_correlation_id)
}

/// Tower layer for adding correlation IDs to requests
#[derive(Clone)]
pub struct CorrelationIdLayer;

impl CorrelationIdLayer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CorrelationIdLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Layer<S> for CorrelationIdLayer {
    type Service = CorrelationIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CorrelationIdService { inner }
    }
}

/// Tower service for correlation ID middleware
#[derive(Clone)]
pub struct CorrelationIdService<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<axum::http::Request<ReqBody>> for CorrelationIdService<S>
where
    S: Service<axum::http::Request<ReqBody>, Response = axum::http::Response<ResBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: axum::http::Request<ReqBody>) -> Self::Future {
        // Extract or generate correlation ID
        let correlation_id = extract_or_generate_correlation_id(req.headers());

        // Add correlation ID to request extensions for downstream use
        req.extensions_mut().insert(CorrelationId(correlation_id.clone()));

        // Add correlation ID to tracing span
        let _span = tracing::info_span!(
            "http_request",
            correlation_id = %correlation_id,
        )
        .entered();

        self.inner.call(req)
    }
}

/// Correlation ID wrapper for use in request extensions
#[derive(Clone, Debug)]
pub struct CorrelationId(pub String);

impl CorrelationId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn test_generate_correlation_id() {
        let id = generate_correlation_id();
        assert!(!id.is_empty());
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_extract_correlation_id_from_x_correlation_id() {
        let mut headers = HeaderMap::new();
        let test_id = "test-correlation-id-123";
        headers.insert(X_CORRELATION_ID, HeaderValue::from_static(test_id));

        let id = extract_or_generate_correlation_id(&headers);
        assert_eq!(id, test_id);
    }

    #[test]
    fn test_extract_correlation_id_from_x_request_id() {
        let mut headers = HeaderMap::new();
        let test_id = "test-request-id-456";
        headers.insert(X_REQUEST_ID, HeaderValue::from_static(test_id));

        let id = extract_or_generate_correlation_id(&headers);
        assert_eq!(id, test_id);
    }

    #[test]
    fn test_extract_correlation_id_prefers_x_correlation_id() {
        let mut headers = HeaderMap::new();
        let correlation_id = "correlation-123";
        let request_id = "request-456";
        headers.insert(X_CORRELATION_ID, HeaderValue::from_static(correlation_id));
        headers.insert(X_REQUEST_ID, HeaderValue::from_static(request_id));

        let id = extract_or_generate_correlation_id(&headers);
        assert_eq!(id, correlation_id);
    }

    #[test]
    fn test_extract_correlation_id_generates_if_missing() {
        let headers = HeaderMap::new();
        let id = extract_or_generate_correlation_id(&headers);
        assert!(!id.is_empty());
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_correlation_id_wrapper() {
        let id = CorrelationId("test-id".to_string());
        assert_eq!(id.as_str(), "test-id");
        assert_eq!(id.to_string(), "test-id");
        assert_eq!(id.into_string(), "test-id");
    }
}
