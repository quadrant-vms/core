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

    // ==== AI Service Metrics ====
    pub static ref AI_SERVICE_ACTIVE_TASKS: IntGauge = {
        let metric = IntGauge::new("ai_service_active_tasks", "Number of active AI tasks")
            .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_TASK_OPERATIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "ai_service_task_operations_total",
                "Total number of AI task operations",
            ),
            &["operation", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_FRAMES_PROCESSED: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "ai_service_frames_processed_total",
                "Total number of frames processed",
            ),
            &["plugin_type", "status"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_DETECTIONS: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "ai_service_detections_total",
                "Total number of detections made",
            ),
            &["plugin_type", "class"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_DETECTION_LATENCY: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "ai_service_detection_latency_seconds",
                "Latency of AI detection operations",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["plugin_type"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_PLUGIN_HEALTH: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "ai_service_plugin_health",
                "Health status of plugins (1=healthy, 0=unhealthy)",
            ),
            &["plugin_id"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_GPU_INFERENCE: IntCounterVec = {
        let metric = IntCounterVec::new(
            Opts::new(
                "ai_service_gpu_inference_total",
                "Total number of GPU inference operations",
            ),
            &["plugin_type", "execution_provider"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_GPU_UTILIZATION: IntGaugeVec = {
        let metric = IntGaugeVec::new(
            Opts::new(
                "ai_service_gpu_utilization_percent",
                "GPU utilization percentage",
            ),
            &["plugin_type", "device_id"],
        )
        .expect("metric can be created");
        REGISTRY.register(Box::new(metric.clone())).ok();
        metric
    };

    pub static ref AI_SERVICE_INFERENCE_TIME: HistogramVec = {
        let metric = HistogramVec::new(
            HistogramOpts::new(
                "ai_service_inference_time_seconds",
                "Time spent on inference (excluding pre/post processing)",
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2, 0.5, 1.0]),
            &["plugin_type", "execution_provider"],
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
    fn test_ai_service_metrics_accessible() {
        AI_SERVICE_ACTIVE_TASKS.set(5);
        assert_eq!(AI_SERVICE_ACTIVE_TASKS.get(), 5);

        AI_SERVICE_FRAMES_PROCESSED
            .with_label_values(&["mock_detector", "success"])
            .inc();
        assert_eq!(
            AI_SERVICE_FRAMES_PROCESSED
                .with_label_values(&["mock_detector", "success"])
                .get(),
            1
        );
    }

    #[test]
    fn test_encode_metrics_succeeds() {
        // Just verify that encoding doesn't panic
        let _encoded = encode_metrics().expect("metrics should encode");
    }
}
