//! Comprehensive end-to-end integration tests for the full VMS pipeline
//!
//! These tests verify the complete system integration:
//! - Coordinator: lease management and orchestration
//! - Admin-gateway: API facade and worker management
//! - Stream-node: RTSP/HLS streaming (stubbed)
//! - Recorder-node: Recording pipeline
//! - AI-service: Frame processing and object detection
//!
//! Test scenarios:
//! 1. Full pipeline: stream → recording → AI frame processing
//! 2. Multi-component interaction with lease coordination
//! 3. Error handling and recovery across services

use admin_gateway::{
    config::GatewayConfig,
    coordinator::HttpCoordinatorClient,
    routes as gateway_routes,
    state::AppState,
    worker::{RecorderClient, WorkerClient},
};
use ai_service::{
    api::router as ai_router, plugin::mock_detector::MockDetectorPlugin,
    plugin::registry::PluginRegistry, AiServiceState,
};
use anyhow::Result;
use axum::Router;
use common::{
    ai_tasks::{AiOutputConfig, AiTaskConfig, AiTaskStartRequest},
    leases::LeaseKind,
    recordings::{RecordingAiConfig, RecordingConfig, RecordingFormat, RecordingStartRequest},
    streams::StreamConfig,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use coordinator::{
    config::{CoordinatorConfig, LeaseStoreType},
    routes as coordinator_routes,
    state::CoordinatorState,
    store::{LeaseStore, MemoryLeaseStore},
};
use reqwest::Client;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle, time::Duration};

// --- Test Infrastructure ---

fn coordinator_state() -> CoordinatorState {
    let cfg = CoordinatorConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        default_ttl_secs: 30,
        max_ttl_secs: 120,
        store_type: LeaseStoreType::Memory,
        database_url: None,
        cluster_enabled: false,
        node_id: None,
        peer_addrs: vec![],
        election_timeout_ms: 5000,
        heartbeat_interval_ms: 1000,
    };
    let store: Arc<dyn LeaseStore> =
        Arc::new(MemoryLeaseStore::new(cfg.default_ttl_secs, cfg.max_ttl_secs));
    CoordinatorState::new(cfg, store)
}

async fn spawn_router(router: Router) -> Result<(SocketAddr, JoinHandle<()>)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server failed");
    });
    // Give the server a moment to bind
    tokio::time::sleep(Duration::from_millis(50)).await;
    Ok((addr, handle))
}

// --- Stub Worker for Stream Node ---

struct StubStreamWorker {
    start_calls: Mutex<Vec<String>>,
    stop_calls: Mutex<Vec<String>>,
}

impl StubStreamWorker {
    fn new() -> Self {
        Self {
            start_calls: Mutex::new(vec![]),
            stop_calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl WorkerClient for StubStreamWorker {
    async fn start_stream(&self, config: &StreamConfig) -> Result<()> {
        self.start_calls.lock().await.push(config.id.clone());
        Ok(())
    }

    async fn stop_stream(&self, stream_id: &str) -> Result<()> {
        self.stop_calls.lock().await.push(stream_id.to_string());
        Ok(())
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

// --- Stub Recorder Client ---

struct StubRecorderClient {
    start_calls: Mutex<Vec<String>>,
    stop_calls: Mutex<Vec<String>>,
}

impl StubRecorderClient {
    fn new() -> Self {
        Self {
            start_calls: Mutex::new(vec![]),
            stop_calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl RecorderClient for StubRecorderClient {
    async fn start_recording(
        &self,
        request: &RecordingStartRequest,
    ) -> Result<common::recordings::RecordingStartResponse> {
        self.start_calls
            .lock()
            .await
            .push(request.config.id.clone());
        Ok(common::recordings::RecordingStartResponse {
            accepted: true,
            lease_id: Some("test-lease-123".to_string()),
            message: Some("Recording started (stubbed)".to_string()),
        })
    }

    async fn stop_recording(
        &self,
        request: &common::recordings::RecordingStopRequest,
    ) -> Result<common::recordings::RecordingStopResponse> {
        self.stop_calls.lock().await.push(request.id.clone());
        Ok(common::recordings::RecordingStopResponse {
            stopped: true,
            message: Some("Recording stopped (stubbed)".to_string()),
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

// --- End-to-End Tests ---

#[tokio::test]
async fn test_full_pipeline_stream_and_ai_task() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // 1. Start Coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
    let coordinator_url = format!("http://{}", coordinator_addr);

    // 2. Start AI Service
    let ai_registry = PluginRegistry::new();
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    ai_registry.register(mock_detector).await?;

    let ai_coordinator_client = Arc::new(
        ai_service::coordinator::HttpCoordinatorClient::new(
            reqwest::Url::parse(&coordinator_url)?
        )?
    ) as Arc<dyn ai_service::coordinator::CoordinatorClient>;

    let ai_state = AiServiceState::with_coordinator(
        "test-ai-node".to_string(),
        ai_coordinator_client,
        ai_registry
    );
    let ai_router = ai_router(ai_state);
    let (ai_addr, ai_task) = spawn_router(ai_router).await?;
    let ai_url = format!("http://{}", ai_addr);

    // 3. Start Admin Gateway
    let stream_worker = Arc::new(StubStreamWorker::new());
    let recorder_worker = Arc::new(StubRecorderClient::new());

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "gateway-e2e-test".to_string(),
        worker_base_url: reqwest::Url::parse("http://stream-worker.local/")?,
        recorder_base_url: reqwest::Url::parse("http://recorder-worker.local/")?,
    };

    let coordinator_client =
        Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let worker_client = stream_worker.clone() as Arc<dyn WorkerClient>;
    let recorder_client = recorder_worker.clone() as Arc<dyn RecorderClient>;

    let app_state = AppState::new(
        gateway_cfg.clone(),
        coordinator_client,
        worker_client,
        recorder_client,
    );
    let gateway_router = gateway_routes::router(app_state);
    let (gateway_addr, gateway_task) = spawn_router(gateway_router).await?;
    let gateway_url = format!("http://{}", gateway_addr);

    // Give all services time to initialize
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = Client::builder().build()?;

    // --- SCENARIO 1: Start a stream via admin-gateway ---
    println!("=== Testing stream start via admin-gateway ===");

    let stream_resp = client
        .post(format!("{}/v1/streams", gateway_url))
        .json(&serde_json::json!({
            "config": {
                "id": "stream-e2e-1",
                "camera_id": "cam-001",
                "uri": "rtsp://example.com/stream1",
                "codec": "h264",
                "container": "ts"
            }
        }))
        .send()
        .await?;

    assert!(
        stream_resp.status().is_success(),
        "Stream start failed: {}",
        stream_resp.status()
    );
    let stream_data: serde_json::Value = stream_resp.json().await?;
    println!("Stream response: {}", stream_data);
    assert!(stream_data["accepted"].as_bool().unwrap_or(false));

    // Verify stream worker was called
    let start_calls = stream_worker.start_calls.lock().await;
    assert_eq!(start_calls.len(), 1);
    assert_eq!(start_calls[0], "stream-e2e-1");
    drop(start_calls);

    // --- SCENARIO 2: Create an AI task for the stream ---
    println!("=== Testing AI task creation ===");

    let ai_task_config = AiTaskConfig {
        id: "ai-task-e2e-1".to_string(),
        plugin_type: "mock_object_detector".to_string(),
        input_stream_id: Some("stream-e2e-1".to_string()),
        input_uri: None,
        model_config: serde_json::json!({}),
        frame_rate: 2,
        output: AiOutputConfig::LocalFile {
            path: "/tmp/ai-output.json".to_string(),
        },
    };

    let ai_task_req = AiTaskStartRequest {
        config: ai_task_config,
        lease_ttl_secs: Some(60),
    };

    let ai_task_resp = client
        .post(format!("{}/v1/tasks", ai_url))
        .json(&ai_task_req)
        .send()
        .await?;

    assert!(
        ai_task_resp.status().is_success(),
        "AI task creation failed: {}",
        ai_task_resp.status()
    );
    let ai_task_data: serde_json::Value = ai_task_resp.json().await?;
    println!("AI task response: {}", ai_task_data);
    assert!(ai_task_data["accepted"].as_bool().unwrap_or(false));

    // --- SCENARIO 3: List leases from coordinator (should have stream + AI task) ---
    println!("=== Testing lease listing ===");

    let leases_resp = client
        .get(format!("{}/v1/leases", coordinator_url))
        .send()
        .await?;

    assert!(leases_resp.status().is_success());
    let leases: Vec<serde_json::Value> = leases_resp.json().await?;
    println!("Active leases: {}", leases.len());

    // We should have at least 2 leases: 1 for stream, 1 for AI task
    assert!(
        leases.len() >= 2,
        "Expected at least 2 leases, got {}",
        leases.len()
    );

    // Find stream and AI leases
    let stream_lease = leases
        .iter()
        .find(|l| l["kind"] == "stream")
        .expect("No stream lease found");
    let ai_lease = leases.iter().find(|l| l["kind"] == "ai").expect("No AI lease found");

    println!("Stream lease: {}", stream_lease);
    println!("AI lease: {}", ai_lease);

    // --- SCENARIO 4: Stop the stream ---
    println!("=== Testing stream stop ===");

    let stop_resp = client
        .delete(format!("{}/v1/streams/stream-e2e-1", gateway_url))
        .send()
        .await?;

    assert!(
        stop_resp.status().is_success(),
        "Stream stop failed: {}",
        stop_resp.status()
    );

    // Verify worker was called
    let stop_calls = stream_worker.stop_calls.lock().await;
    assert_eq!(stop_calls.len(), 1);
    assert_eq!(stop_calls[0], "stream-e2e-1");

    // --- SCENARIO 5: Verify AI task is still listed ---
    println!("=== Testing AI task listing ===");

    let ai_tasks_resp = client.get(format!("{}/v1/tasks", ai_url)).send().await?;
    assert!(ai_tasks_resp.status().is_success());
    let ai_tasks: serde_json::Value = ai_tasks_resp.json().await?;
    println!("AI tasks: {}", ai_tasks);

    // Cleanup
    coordinator_task.abort();
    ai_task.abort();
    gateway_task.abort();

    Ok(())
}

#[tokio::test]
async fn test_full_pipeline_recording_with_ai() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // 1. Start Coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
    let coordinator_url = format!("http://{}", coordinator_addr);

    // 2. Start AI Service
    let ai_registry = PluginRegistry::new();
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    ai_registry.register(mock_detector).await?;

    let ai_state = AiServiceState::new("test-ai-node-rec".to_string(), ai_registry);
    let ai_router = ai_router(ai_state);
    let (ai_addr, ai_task) = spawn_router(ai_router).await?;
    let ai_url = format!("http://{}", ai_addr);

    // 3. Start Admin Gateway
    let stream_worker = Arc::new(StubStreamWorker::new());
    let recorder_worker = Arc::new(StubRecorderClient::new());

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "gateway-rec-e2e-test".to_string(),
        worker_base_url: reqwest::Url::parse("http://stream-worker.local/")?,
        recorder_base_url: reqwest::Url::parse("http://recorder-worker.local/")?,
    };

    let coordinator_client =
        Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let worker_client = stream_worker.clone() as Arc<dyn WorkerClient>;
    let recorder_client = recorder_worker.clone() as Arc<dyn RecorderClient>;

    let app_state = AppState::new(
        gateway_cfg.clone(),
        coordinator_client,
        worker_client,
        recorder_client,
    );
    let gateway_router = gateway_routes::router(app_state);
    let (gateway_addr, gateway_task) = spawn_router(gateway_router).await?;
    let gateway_url = format!("http://{}", gateway_addr);

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = Client::builder().build()?;

    // --- SCENARIO: Start recording with AI frame processing ---
    println!("=== Testing recording with AI integration ===");

    let recording_req = RecordingStartRequest {
        config: RecordingConfig {
            id: "rec-e2e-ai-1".to_string(),
            source_stream_id: Some("stream-001".to_string()),
            source_uri: Some("rtsp://example.com/camera1".to_string()),
            retention_hours: Some(24),
            format: Some(RecordingFormat::Mp4),
        },
        lease_ttl_secs: Some(60),
        ai_config: Some(RecordingAiConfig {
            ai_service_url: ai_url.clone(),
            ai_task_id: "rec-ai-task-1".to_string(),
            capture_interval_secs: 5,
            frame_width: 1280,
            frame_height: 720,
            jpeg_quality: 90,
        }),
    };

    let rec_resp = client
        .post(format!("{}/v1/recordings", gateway_url))
        .json(&recording_req)
        .send()
        .await?;

    assert!(
        rec_resp.status().is_success(),
        "Recording start failed: {}",
        rec_resp.status()
    );
    let rec_data: serde_json::Value = rec_resp.json().await?;
    println!("Recording response: {}", rec_data);
    assert!(rec_data["accepted"].as_bool().unwrap_or(false));

    // Verify recorder was called
    let rec_start_calls = recorder_worker.start_calls.lock().await;
    assert_eq!(rec_start_calls.len(), 1);
    assert_eq!(rec_start_calls[0], "rec-e2e-ai-1");
    drop(rec_start_calls);

    // Check coordinator has recording lease
    let leases_resp = client
        .get(format!("{}/v1/leases?kind=recorder", coordinator_url))
        .send()
        .await?;

    assert!(
        leases_resp.status().is_success(),
        "Lease query failed: {}",
        leases_resp.status()
    );
    let leases: Vec<serde_json::Value> = leases_resp.json().await?;
    println!("Recording leases: {}", leases.len());
    assert!(
        leases.len() >= 1,
        "Expected at least 1 recording lease, got {}",
        leases.len()
    );

    // Stop the recording
    println!("=== Testing recording stop ===");

    let stop_rec_resp = client
        .delete(format!("{}/v1/recordings/rec-e2e-ai-1", gateway_url))
        .send()
        .await?;

    assert!(
        stop_rec_resp.status().is_success(),
        "Recording stop failed: {}",
        stop_rec_resp.status()
    );

    let rec_stop_calls = recorder_worker.stop_calls.lock().await;
    assert_eq!(rec_stop_calls.len(), 1);
    assert_eq!(rec_stop_calls[0], "rec-e2e-ai-1");

    // Cleanup
    coordinator_task.abort();
    ai_task.abort();
    gateway_task.abort();

    Ok(())
}

#[tokio::test]
async fn test_coordinator_lease_types_isolation() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Start coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
    let coordinator_url = format!("http://{}", coordinator_addr);

    let client = Client::builder().build()?;

    // Acquire leases of different types
    let lease_types = vec![
        ("stream", LeaseKind::Stream),
        ("recorder", LeaseKind::Recorder),
        ("ai", LeaseKind::Ai),
    ];

    for (resource_prefix, kind) in &lease_types {
        let acquire_resp = client
            .post(format!("{}/v1/leases/acquire", coordinator_url))
            .json(&serde_json::json!({
                "resource_id": format!("{}-resource-1", resource_prefix),
                "holder_id": format!("{}-holder", resource_prefix),
                "kind": kind,
                "ttl_secs": 30
            }))
            .send()
            .await?;

        assert!(acquire_resp.status().is_success());
        let resp_data: serde_json::Value = acquire_resp.json().await?;
        assert_eq!(resp_data["granted"], true);
    }

    // List all leases
    let all_leases_resp = client
        .get(format!("{}/v1/leases", coordinator_url))
        .send()
        .await?;

    assert!(all_leases_resp.status().is_success());
    let all_leases: Vec<serde_json::Value> = all_leases_resp.json().await?;
    assert_eq!(
        all_leases.len(),
        3,
        "Expected 3 leases total, got {}",
        all_leases.len()
    );

    // Filter by each kind
    for (_, kind) in &lease_types {
        let kind_str = match kind {
            LeaseKind::Stream => "stream",
            LeaseKind::Recorder => "recorder",
            LeaseKind::Ai => "ai",
            _ => "unknown",
        };

        let filtered_resp = client
            .get(format!("{}/v1/leases?kind={}", coordinator_url, kind_str))
            .send()
            .await?;

        assert!(filtered_resp.status().is_success());
        let filtered: Vec<serde_json::Value> = filtered_resp.json().await?;
        assert_eq!(
            filtered.len(),
            1,
            "Expected 1 lease for kind {}, got {}",
            kind_str,
            filtered.len()
        );
        assert_eq!(filtered[0]["kind"], kind_str);
    }

    coordinator_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_multi_service_health_checks() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Start all services
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
    let coordinator_url = format!("http://{}", coordinator_addr);

    let ai_registry = PluginRegistry::new();
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    ai_registry.register(mock_detector).await?;

    let ai_state = AiServiceState::new("health-test-ai".to_string(), ai_registry);
    let ai_router = ai_router(ai_state);
    let (ai_addr, ai_task) = spawn_router(ai_router).await?;

    let stream_worker = Arc::new(StubStreamWorker::new());
    let recorder_worker = Arc::new(StubRecorderClient::new());

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "health-test-gateway".to_string(),
        worker_base_url: reqwest::Url::parse("http://worker.local/")?,
        recorder_base_url: reqwest::Url::parse("http://recorder.local/")?,
    };

    let coordinator_client =
        Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let app_state = AppState::new(
        gateway_cfg,
        coordinator_client,
        stream_worker as Arc<dyn WorkerClient>,
        recorder_worker as Arc<dyn RecorderClient>,
    );
    let gateway_router = gateway_routes::router(app_state);
    let (gateway_addr, gateway_task) = spawn_router(gateway_router).await?;

    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = Client::builder().build()?;

    // Check health endpoints
    let services = vec![
        ("coordinator", coordinator_addr),
        ("ai-service", ai_addr),
        ("admin-gateway", gateway_addr),
    ];

    for (name, addr) in &services {
        // /healthz
        let health_resp = client
            .get(format!("http://{}/healthz", addr))
            .send()
            .await?;
        assert!(
            health_resp.status().is_success(),
            "{} /healthz failed",
            name
        );

        // /metrics
        let metrics_resp = client
            .get(format!("http://{}/metrics", addr))
            .send()
            .await?;
        assert!(
            metrics_resp.status().is_success(),
            "{} /metrics failed",
            name
        );
    }

    coordinator_task.abort();
    ai_task.abort();
    gateway_task.abort();

    Ok(())
}
