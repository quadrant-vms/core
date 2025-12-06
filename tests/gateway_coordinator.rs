use admin_gateway::{
    config::GatewayConfig,
    coordinator::HttpCoordinatorClient,
    routes as gateway_routes,
    state::AppState,
    worker::{RecorderClient, WorkerClient},
};
use anyhow::Result;
use axum::Router;
use coordinator::{
    config::{CoordinatorConfig, LeaseStoreType},
    routes as coordinator_routes,
    state::CoordinatorState,
    store::{LeaseStore, MemoryLeaseStore},
};
use reqwest::Client;
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle, time::Duration};

fn coordinator_state() -> CoordinatorState {
    let cfg = CoordinatorConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        default_ttl_secs: 15,
        max_ttl_secs: 60,
        store_type: LeaseStoreType::Memory,
        database_url: None,
        cluster_enabled: false,
        node_id: None,
        peer_addrs: vec![],
        election_timeout_ms: 5000,
        heartbeat_interval_ms: 1000,
    };
    let store: Arc<dyn LeaseStore> = Arc::new(MemoryLeaseStore::new(cfg.default_ttl_secs, cfg.max_ttl_secs));
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
    Ok((addr, handle))
}

struct StubWorker {
    start_calls: Mutex<Vec<String>>,
    stop_calls: Mutex<Vec<String>>,
}

impl StubWorker {
    fn new() -> Self {
        Self {
            start_calls: Mutex::new(vec![]),
            stop_calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait::async_trait]
impl WorkerClient for StubWorker {
    async fn start_stream(&self, config: &common::streams::StreamConfig) -> Result<()> {
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

struct StubRecorder;

impl StubRecorder {
    fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl RecorderClient for StubRecorder {
    async fn start_recording(&self, _request: &common::recordings::RecordingStartRequest) -> Result<common::recordings::RecordingStartResponse> {
        Ok(common::recordings::RecordingStartResponse {
            accepted: true,
            lease_id: None,
            message: None,
        })
    }

    async fn stop_recording(&self, _request: &common::recordings::RecordingStopRequest) -> Result<common::recordings::RecordingStopResponse> {
        Ok(common::recordings::RecordingStopResponse {
            stopped: true,
            message: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

#[tokio::test]
async fn gateway_and_coordinator_end_to_end() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    let coordinator_url = format!("http://{}", coordinator_addr);

    let worker = Arc::new(StubWorker::new());

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "gateway-test".to_string(),
        worker_base_url: reqwest::Url::parse("http://worker.local/")?,
        recorder_base_url: reqwest::Url::parse("http://recorder.local/")?,
    };
    let coordinator_client = Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let worker_client = worker.clone() as Arc<dyn WorkerClient>;
    let recorder_client = Arc::new(StubRecorder::new()) as Arc<dyn RecorderClient>;
    let app_state = AppState::new(gateway_cfg.clone(), coordinator_client, worker_client, recorder_client);
    let gateway_router = gateway_routes::router(app_state);
    let (gateway_addr, gateway_task) = spawn_router(gateway_router).await?;

    // Give servers a moment to bind fully
    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::builder().build()?;
    let base = format!("http://{}", gateway_addr);

    let start_resp = client
        .post(format!("{base}/v1/streams"))
        .json(&serde_json::json!({
            "config": {
                "id": "cam-1",
                "uri": "rtsp://example",
                "codec": "h264",
                "container": "ts"
            }
        }))
        .send()
        .await?;
    assert!(start_resp.status().is_success());
    let body: serde_json::Value = start_resp.json().await?;
    assert_eq!(body["accepted"], true);

    let list_resp = client.get(format!("{base}/v1/streams")).send().await?;
    assert!(list_resp.status().is_success());
    let list: serde_json::Value = list_resp.json().await?;
    assert_eq!(list.as_array().unwrap().len(), 1);

    let stop_resp = client
        .delete(format!("{base}/v1/streams/cam-1"))
        .send()
        .await?;
    assert!(stop_resp.status().is_success());

    let list_resp = client.get(format!("{base}/v1/streams")).send().await?;
    let list: serde_json::Value = list_resp.json().await?;
    assert!(list.as_array().unwrap().is_empty());

    gateway_task.abort();
    coordinator_task.abort();

    let starts = worker.start_calls.lock().await.clone();
    let stops = worker.stop_calls.lock().await.clone();
    assert_eq!(starts, vec!["cam-1".to_string()]);
    assert_eq!(stops, vec!["cam-1".to_string()]);

    Ok(())
}
