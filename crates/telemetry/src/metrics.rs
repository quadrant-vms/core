use lazy_static::lazy_static;
use prometheus::{
    Counter, Histogram, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Opts, Registry,
};

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // ==== Coordinator Metrics ====
    pub static ref COORDINATOR_ACTIVE_LEASES: IntGauge = {
        let metric = IntGauge::new("coordinator_active_leases", "Number of active leases")
            .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref COORDINATOR_LEASE_OPERATIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "coordinator_lease_operations_total",
                "Total number of lease operations",
            ),
            &["operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref COORDINATOR_LEASE_DURATION: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "coordinator_lease_duration_seconds",
                "Duration of lease operations",
            ),
            &["operation"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref COORDINATOR_CLUSTER_NODES: IntGauge = {
        let metric = IntGauge::new("coordinator_cluster_nodes", "Number of cluster nodes")
            .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref COORDINATOR_LEADER_ELECTIONS: IntCounter = {
        let metric = IntCounter::new(
            "coordinator_leader_elections_total",
            "Total number of leader elections",
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref COORDINATOR_FORWARDED_REQUESTS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "coordinator_forwarded_requests_total",
                "Total number of requests forwarded to leader",
            ),
            &["operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Stream Node Metrics ====
    pub static ref STREAM_NODE_ACTIVE_STREAMS: IntGauge = {
        let metric = IntGauge::new("stream_node_active_streams", "Number of active streams")
            .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref STREAM_NODE_OPERATIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "stream_node_operations_total",
                "Total number of stream operations",
            ),
            &["operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref STREAM_NODE_DURATION: Histogram = {
        let metric = Histogram::with_opts(
            HistogramOpts::new(
                "stream_node_stream_duration_seconds",
                "Duration of active streams",
            )
            .buckets(vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0]),
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref STREAM_NODE_HLS_SEGMENTS: IntCounter = {
        let metric = IntCounter::new(
            "stream_node_hls_segments_total",
            "Total number of HLS segments created",
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref STREAM_NODE_S3_UPLOADS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "stream_node_s3_uploads_total",
                "Total number of S3 upload attempts",
            ),
            &["status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref STREAM_NODE_BYTES_PROCESSED: Counter = {
        let metric = Counter::new(
            "stream_node_bytes_processed_total",
            "Total bytes processed",
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Recorder Node Metrics ====
    pub static ref RECORDER_NODE_ACTIVE_RECORDINGS: IntGauge = {
        let metric = IntGauge::new("recorder_node_active_recordings", "Number of active recordings")
            .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref RECORDER_NODE_OPERATIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "recorder_node_operations_total",
                "Total number of recording operations",
            ),
            &["operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref RECORDER_NODE_DURATION: Histogram = {
        let metric = Histogram::with_opts(
            HistogramOpts::new(
                "recorder_node_recording_duration_seconds",
                "Duration of recordings",
            )
            .buckets(vec![60.0, 300.0, 600.0, 1800.0, 3600.0, 7200.0]),
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref RECORDER_NODE_COMPLETED: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "recorder_node_recordings_completed_total",
                "Total number of completed recordings",
            ),
            &["format", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref RECORDER_NODE_BYTES_RECORDED: Counter = {
        let metric = Counter::new(
            "recorder_node_bytes_recorded_total",
            "Total bytes recorded",
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    // ==== Admin Gateway Metrics ====
    pub static ref ADMIN_GATEWAY_HTTP_REQUESTS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "admin_gateway_http_requests_total",
                "Total number of HTTP requests",
            ),
            &["method", "path", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref ADMIN_GATEWAY_HTTP_DURATION: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "admin_gateway_http_request_duration_seconds",
                "HTTP request duration",
            ),
            &["method", "path"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref ADMIN_GATEWAY_ACTIVE_WORKERS: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "admin_gateway_active_workers",
                "Number of active workers by type",
            ),
            &["worker_type"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref ADMIN_GATEWAY_WORKER_OPERATIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "admin_gateway_worker_operations_total",
                "Total number of worker operations",
            ),
            &["worker_type", "operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };
}

/// Helper function to encode metrics for Prometheus scraping
pub fn encode_metrics() -> Result<String, prometheus::Error> {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer)?;
    String::from_utf8(buffer).map_err(|e| {
        prometheus::Error::Msg(format!("Failed to convert metrics to UTF-8: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coordinator_metrics_accessible() {
        COORDINATOR_ACTIVE_LEASES.set(5);
        assert_eq!(COORDINATOR_ACTIVE_LEASES.get(), 5);
    }

    #[test]
    fn test_stream_node_metrics_accessible() {
        STREAM_NODE_ACTIVE_STREAMS.set(10);
        assert_eq!(STREAM_NODE_ACTIVE_STREAMS.get(), 10);
    }

    #[test]
    fn test_recorder_node_metrics_accessible() {
        RECORDER_NODE_ACTIVE_RECORDINGS.set(3);
        assert_eq!(RECORDER_NODE_ACTIVE_RECORDINGS.get(), 3);
    }

    #[test]
    fn test_admin_gateway_metrics_accessible() {
        ADMIN_GATEWAY_ACTIVE_WORKERS
            .with_label_values(&["stream"])
            .set(2);
        assert_eq!(
            ADMIN_GATEWAY_ACTIVE_WORKERS
                .with_label_values(&["stream"])
                .get(),
            2
        );
    }

    #[test]
    fn test_encode_metrics_succeeds() {
        // Just verify that encoding doesn't panic
        let _encoded = encode_metrics().expect("metrics should encode");
    }
}
