//! Service Level Objective (SLO) metrics for monitoring service health and performance.
//!
//! This module provides comprehensive SLO metrics across four key dimensions:
//! - **Availability**: Uptime and service reachability
//! - **Latency**: Request processing time and response times
//! - **Error Rate**: Failed requests and operations
//! - **Throughput**: Request volume and data processed
//!
//! All metrics are labeled by tenant and node for granular monitoring.

use lazy_static::lazy_static;
use prometheus::{
    CounterVec, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
};

lazy_static! {
    pub static ref SLO_REGISTRY: Registry = Registry::new_custom(
        Some("slo".to_string()),
        None
    ).expect("SLO registry can be created");

    // ==== Availability Metrics ====

    /// Service uptime status (1 = up, 0 = down) by tenant and node
    pub static ref SLO_SERVICE_UP: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "service_up",
                "Service uptime status (1=up, 0=down) by tenant and node"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Health check successes by tenant and node
    pub static ref SLO_HEALTH_CHECK_SUCCESS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "health_check_success_total",
                "Total successful health checks by tenant and node"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Health check failures by tenant and node
    pub static ref SLO_HEALTH_CHECK_FAILURE: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "health_check_failure_total",
                "Total failed health checks by tenant and node"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Dependency availability (external services, databases, etc.)
    pub static ref SLO_DEPENDENCY_UP: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "dependency_up",
                "Dependency availability status (1=up, 0=down)"
            ),
            &["service", "dependency", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Latency Metrics ====

    /// Request latency histogram by tenant, node, and endpoint
    pub static ref SLO_REQUEST_LATENCY: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "request_latency_seconds",
                "Request processing latency in seconds"
            )
            .buckets(vec![
                0.001, 0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0
            ]),
            &["service", "endpoint", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Database query latency by operation type
    pub static ref SLO_DB_LATENCY: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "db_query_latency_seconds",
                "Database query latency in seconds"
            )
            .buckets(vec![
                0.0001, 0.0005, 0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0
            ]),
            &["service", "operation", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// External API call latency
    pub static ref SLO_EXTERNAL_API_LATENCY: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "external_api_latency_seconds",
                "External API call latency in seconds"
            )
            .buckets(vec![
                0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0
            ]),
            &["service", "api", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Time to first byte (TTFB) for playback and streaming
    pub static ref SLO_TTFB: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "ttfb_seconds",
                "Time to first byte in seconds"
            )
            .buckets(vec![
                0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0
            ]),
            &["service", "protocol", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Error Rate Metrics ====

    /// Total requests by tenant, node, endpoint, and status
    pub static ref SLO_REQUESTS_TOTAL: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "requests_total",
                "Total number of requests"
            ),
            &["service", "endpoint", "method", "status", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Failed requests (4xx, 5xx) by tenant and node
    pub static ref SLO_REQUESTS_FAILED: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "requests_failed_total",
                "Total number of failed requests (4xx, 5xx)"
            ),
            &["service", "endpoint", "method", "status_class", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Database operation errors by operation type
    pub static ref SLO_DB_ERRORS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "db_errors_total",
                "Total number of database errors"
            ),
            &["service", "operation", "error_type", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// External API call errors
    pub static ref SLO_EXTERNAL_API_ERRORS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "external_api_errors_total",
                "Total number of external API errors"
            ),
            &["service", "api", "error_type", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Pipeline failures (stream, recording, AI)
    pub static ref SLO_PIPELINE_FAILURES: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "pipeline_failures_total",
                "Total number of pipeline failures"
            ),
            &["service", "pipeline_type", "failure_reason", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Throughput Metrics ====

    /// Requests per second by tenant and node
    pub static ref SLO_REQUEST_RATE: CounterVec = {
        let metric = CounterVec::new(
            Opts::new(
                "request_rate_total",
                "Total request count for rate calculation"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Bytes processed (ingested, streamed, recorded) by tenant and node
    pub static ref SLO_BYTES_PROCESSED: CounterVec = {
        let metric = CounterVec::new(
            Opts::new(
                "bytes_processed_total",
                "Total bytes processed"
            ),
            &["service", "operation", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Active concurrent operations by tenant and node
    pub static ref SLO_CONCURRENT_OPERATIONS: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "concurrent_operations",
                "Number of active concurrent operations"
            ),
            &["service", "operation_type", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Queue depth for async operations
    pub static ref SLO_QUEUE_DEPTH: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "queue_depth",
                "Current queue depth for async operations"
            ),
            &["service", "queue_type", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Resource Utilization Metrics ====

    /// CPU utilization percentage by tenant and node
    pub static ref SLO_CPU_UTILIZATION: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "cpu_utilization_percent",
                "CPU utilization percentage"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Memory utilization in bytes by tenant and node
    pub static ref SLO_MEMORY_USAGE: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "memory_usage_bytes",
                "Memory usage in bytes"
            ),
            &["service", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Disk I/O operations by tenant and node
    pub static ref SLO_DISK_IO: CounterVec = {
        let metric = CounterVec::new(
            Opts::new(
                "disk_io_total",
                "Total disk I/O operations"
            ),
            &["service", "operation", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    /// Network bytes transferred by tenant and node
    pub static ref SLO_NETWORK_BYTES: CounterVec = {
        let metric = CounterVec::new(
            Opts::new(
                "network_bytes_total",
                "Total network bytes transferred"
            ),
            &["service", "direction", "tenant_id", "node_id"],
        )
        .expect("metric can be created");
        SLO_REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };
}

/// SLO configuration and tracking
#[derive(Clone)]
pub struct SloTracker {
    service_name: String,
    node_id: String,
    default_tenant: String,
}

impl SloTracker {
    /// Create a new SLO tracker for a service
    pub fn new(service_name: impl Into<String>, node_id: impl Into<String>) -> Self {
        let service_name = service_name.into();
        let node_id = node_id.into();
        let default_tenant = "default".to_string();

        // Initialize service as up
        SLO_SERVICE_UP
            .with_label_values(&[&service_name, &default_tenant, &node_id])
            .set(1);

        Self {
            service_name,
            node_id,
            default_tenant,
        }
    }

    /// Mark service as up or down
    pub fn set_service_status(&self, up: bool, tenant_id: Option<&str>) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_SERVICE_UP
            .with_label_values(&[&self.service_name, tenant, &self.node_id])
            .set(if up { 1 } else { 0 });
    }

    /// Record a successful health check
    pub fn record_health_check_success(&self, tenant_id: Option<&str>) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_HEALTH_CHECK_SUCCESS
            .with_label_values(&[&self.service_name, tenant, &self.node_id])
            .inc();
    }

    /// Record a failed health check
    pub fn record_health_check_failure(&self, tenant_id: Option<&str>) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_HEALTH_CHECK_FAILURE
            .with_label_values(&[&self.service_name, tenant, &self.node_id])
            .inc();
    }

    /// Record request latency
    pub fn record_request_latency(
        &self,
        endpoint: &str,
        duration_secs: f64,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_REQUEST_LATENCY
            .with_label_values(&[&self.service_name, endpoint, tenant, &self.node_id])
            .observe(duration_secs);
    }

    /// Record a request (success or failure)
    pub fn record_request(
        &self,
        endpoint: &str,
        method: &str,
        status: u16,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        let status_str = status.to_string();

        // Total requests
        SLO_REQUESTS_TOTAL
            .with_label_values(&[
                &self.service_name,
                endpoint,
                method,
                &status_str,
                tenant,
                &self.node_id,
            ])
            .inc();

        // Failed requests (4xx, 5xx)
        if status >= 400 {
            let status_class = if status < 500 { "4xx" } else { "5xx" };
            SLO_REQUESTS_FAILED
                .with_label_values(&[
                    &self.service_name,
                    endpoint,
                    method,
                    status_class,
                    tenant,
                    &self.node_id,
                ])
                .inc();
        }

        // Request rate
        SLO_REQUEST_RATE
            .with_label_values(&[&self.service_name, tenant, &self.node_id])
            .inc();
    }

    /// Record database operation latency
    pub fn record_db_latency(
        &self,
        operation: &str,
        duration_secs: f64,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_DB_LATENCY
            .with_label_values(&[&self.service_name, operation, tenant, &self.node_id])
            .observe(duration_secs);
    }

    /// Record database error
    pub fn record_db_error(
        &self,
        operation: &str,
        error_type: &str,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_DB_ERRORS
            .with_label_values(&[&self.service_name, operation, error_type, tenant, &self.node_id])
            .inc();
    }

    /// Record bytes processed
    pub fn record_bytes_processed(
        &self,
        operation: &str,
        bytes: u64,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_BYTES_PROCESSED
            .with_label_values(&[&self.service_name, operation, tenant, &self.node_id])
            .inc_by(bytes as f64);
    }

    /// Set concurrent operations gauge
    pub fn set_concurrent_operations(
        &self,
        operation_type: &str,
        count: i64,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_CONCURRENT_OPERATIONS
            .with_label_values(&[&self.service_name, operation_type, tenant, &self.node_id])
            .set(count);
    }

    /// Record pipeline failure
    pub fn record_pipeline_failure(
        &self,
        pipeline_type: &str,
        failure_reason: &str,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_PIPELINE_FAILURES
            .with_label_values(&[
                &self.service_name,
                pipeline_type,
                failure_reason,
                tenant,
                &self.node_id,
            ])
            .inc();
    }

    /// Set dependency status
    pub fn set_dependency_status(
        &self,
        dependency: &str,
        up: bool,
        tenant_id: Option<&str>,
    ) {
        let tenant = tenant_id.unwrap_or(&self.default_tenant);
        SLO_DEPENDENCY_UP
            .with_label_values(&[&self.service_name, dependency, tenant, &self.node_id])
            .set(if up { 1 } else { 0 });
    }
}

/// Helper function to encode SLO metrics for Prometheus scraping
pub fn encode_slo_metrics() -> Result<String, prometheus::Error> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = SLO_REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    String::from_utf8(buffer).map_err(|e| {
        prometheus::Error::Msg(format!("Failed to convert SLO metrics to UTF-8: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slo_tracker_creation() {
        let tracker = SloTracker::new("test-service", "node-1");
        assert_eq!(tracker.service_name, "test-service");
        assert_eq!(tracker.node_id, "node-1");
    }

    #[test]
    fn test_service_status() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.set_service_status(true, Some("tenant-1"));

        let value = SLO_SERVICE_UP
            .with_label_values(&["test-service", "tenant-1", "node-1"])
            .get();
        assert_eq!(value, 1);

        tracker.set_service_status(false, Some("tenant-1"));
        let value = SLO_SERVICE_UP
            .with_label_values(&["test-service", "tenant-1", "node-1"])
            .get();
        assert_eq!(value, 0);
    }

    #[test]
    fn test_health_checks() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_health_check_success(Some("tenant-1"));
        tracker.record_health_check_success(Some("tenant-1"));
        tracker.record_health_check_failure(Some("tenant-1"));

        let success = SLO_HEALTH_CHECK_SUCCESS
            .with_label_values(&["test-service", "tenant-1", "node-1"])
            .get();
        let failure = SLO_HEALTH_CHECK_FAILURE
            .with_label_values(&["test-service", "tenant-1", "node-1"])
            .get();

        assert_eq!(success, 2);
        assert_eq!(failure, 1);
    }

    #[test]
    fn test_request_recording() {
        let tracker = SloTracker::new("test-service-req", "node-req");

        // Record successful request
        tracker.record_request("/api/test", "GET", 200, Some("tenant-req"));

        let total = SLO_REQUESTS_TOTAL
            .with_label_values(&["test-service-req", "/api/test", "GET", "200", "tenant-req", "node-req"])
            .get();
        assert_eq!(total, 1);

        // Record failed request
        tracker.record_request("/api/test", "POST", 500, Some("tenant-req"));

        let failed = SLO_REQUESTS_FAILED
            .with_label_values(&["test-service-req", "/api/test", "POST", "5xx", "tenant-req", "node-req"])
            .get();
        assert_eq!(failed, 1);
    }

    #[test]
    fn test_latency_recording() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_request_latency("/api/test", 0.05, Some("tenant-1"));
        tracker.record_db_latency("select", 0.001, Some("tenant-1"));

        // Just verify no panics - histogram values are harder to assert
        let _encoded = encode_slo_metrics().expect("metrics should encode");
    }

    #[test]
    fn test_bytes_processed() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_bytes_processed("ingest", 1024, Some("tenant-1"));
        tracker.record_bytes_processed("ingest", 2048, Some("tenant-1"));

        let total = SLO_BYTES_PROCESSED
            .with_label_values(&["test-service", "ingest", "tenant-1", "node-1"])
            .get();
        assert_eq!(total, 3072.0);
    }

    #[test]
    fn test_concurrent_operations() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.set_concurrent_operations("stream", 5, Some("tenant-1"));

        let value = SLO_CONCURRENT_OPERATIONS
            .with_label_values(&["test-service", "stream", "tenant-1", "node-1"])
            .get();
        assert_eq!(value, 5);
    }

    #[test]
    fn test_pipeline_failures() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_pipeline_failure("recording", "ffmpeg_error", Some("tenant-1"));

        let value = SLO_PIPELINE_FAILURES
            .with_label_values(&["test-service", "recording", "ffmpeg_error", "tenant-1", "node-1"])
            .get();
        assert_eq!(value, 1);
    }

    #[test]
    fn test_dependency_status() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.set_dependency_status("postgres", true, Some("tenant-1"));

        let value = SLO_DEPENDENCY_UP
            .with_label_values(&["test-service", "postgres", "tenant-1", "node-1"])
            .get();
        assert_eq!(value, 1);
    }

    #[test]
    fn test_default_tenant() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_request("/api/test", "GET", 200, None);

        let total = SLO_REQUESTS_TOTAL
            .with_label_values(&["test-service", "/api/test", "GET", "200", "default", "node-1"])
            .get();
        assert_eq!(total, 1);
    }

    #[test]
    fn test_encode_slo_metrics() {
        let tracker = SloTracker::new("test-service", "node-1");
        tracker.record_request("/api/test", "GET", 200, Some("tenant-1"));

        let encoded = encode_slo_metrics().expect("should encode");
        assert!(encoded.contains("slo_requests_total"));
        assert!(encoded.contains("tenant_id=\"tenant-1\""));
        assert!(encoded.contains("node_id=\"node-1\""));
    }
}
