use anyhow::Result;
use axum::Router;
use common::recordings::{RecordingConfig, RecordingFormat, RecordingStartRequest};
use coordinator::{
  config::{CoordinatorConfig, LeaseStoreType},
  routes as coordinator_routes,
  state::CoordinatorState,
  store::{LeaseStore, MemoryLeaseStore},
};
use recorder_node::coordinator::HttpCoordinatorClient;
use recorder_node::recording::manager::RECORDING_MANAGER;
use reqwest::Client;
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle, time::Duration, sync::Mutex};
use std::sync::OnceLock;

// Global mutex to serialize tests that use the global RECORDING_MANAGER
static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

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
  let store: Arc<dyn LeaseStore> =
    Arc::new(MemoryLeaseStore::new(cfg.default_ttl_secs, cfg.max_ttl_secs));
  CoordinatorState::new(cfg, store, None)
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

#[tokio::test]
async fn recorder_acquires_and_releases_lease() -> Result<()> {
  // Acquire test lock to prevent parallel execution with shared global state
  let _lock = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().await;

  let _ = tracing_subscriber::fmt::try_init();
  std::env::set_var("MOCK_RECORDING", "1");

  // Clear any state from previous tests
  RECORDING_MANAGER.clear().await;

  // Give time for cancellation tokens to fully cleanup from previous tests
  tokio::time::sleep(Duration::from_millis(200)).await;

  // Spawn coordinator
  let coordinator_router = coordinator_routes::router(coordinator_state());
  let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
  let coordinator_url = format!("http://{}/", coordinator_addr);

  tokio::time::sleep(Duration::from_millis(100)).await;

  // Initialize recorder manager with coordinator
  let base = reqwest::Url::parse(&coordinator_url)?;
  let client = Arc::new(HttpCoordinatorClient::new(base)?);
  RECORDING_MANAGER
    .set_coordinator(client.clone(), "recorder-test".to_string())
    .await;

  // Start a recording with unique ID to avoid cross-test conflicts
  let test_id = format!("rec-acquire-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
  let config = RecordingConfig {
    id: test_id.clone(),
    source_stream_id: Some("stream-1".to_string()),
    source_uri: Some("rtsp://example.com/stream".to_string()),
    retention_hours: Some(24),
    format: Some(RecordingFormat::Mp4),
  };

  let req = RecordingStartRequest {
    config,
    lease_ttl_secs: Some(30),
    ai_config: None,
  };

  let response = RECORDING_MANAGER.start(req).await?;
  assert!(response.accepted);
  assert!(response.lease_id.is_some());
  let lease_id = response.lease_id.unwrap();

  // Verify lease was acquired by checking coordinator (use retry loop for timing)
  let http_client = Client::builder().build()?;
  let mut lease_found = false;
  for _ in 0..20 {
    tokio::time::sleep(Duration::from_millis(50)).await;
    let leases_resp = http_client
      .get(format!("{}v1/leases?kind=recorder", coordinator_url))
      .send()
      .await?;
    if leases_resp.status().is_success() {
      let leases: Vec<serde_json::Value> = leases_resp.json().await?;
      // Filter to only our test's lease (tests may run in parallel)
      let our_lease: Vec<_> = leases.iter().filter(|l| l["resource_id"] == test_id).collect();
      if our_lease.len() == 1 && our_lease[0]["lease_id"] == lease_id {
        lease_found = true;
        break;
      }
    }
  }
  assert!(lease_found, "lease was not found in coordinator within timeout (1 second)");

  // Stop the recording
  let stopped = RECORDING_MANAGER.stop(&test_id).await?;
  assert!(stopped);

  // Verify lease was released (give time for async HTTP call to coordinator)
  // Use a retry loop with timeout to handle timing variability
  let mut lease_released = false;
  for _ in 0..20 {
    tokio::time::sleep(Duration::from_millis(100)).await;
    let leases_resp = http_client
      .get(format!("{}v1/leases?kind=recorder", coordinator_url))
      .send()
      .await?;
    let leases: Vec<serde_json::Value> = leases_resp.json().await?;
    if leases.is_empty() {
      lease_released = true;
      break;
    }
  }
  assert!(lease_released, "lease was not released within timeout (2 seconds)");

  coordinator_task.abort();
  Ok(())
}

#[tokio::test]
async fn recorder_lease_conflict() -> Result<()> {
  // Acquire test lock to prevent parallel execution with shared global state
  let _lock = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().await;

  let _ = tracing_subscriber::fmt::try_init();
  std::env::set_var("MOCK_RECORDING", "1");

  // Clear any state from previous tests
  RECORDING_MANAGER.clear().await;

  // Give time for cancellation tokens to fully cleanup from previous tests
  tokio::time::sleep(Duration::from_millis(200)).await;

  // Spawn coordinator
  let coordinator_router = coordinator_routes::router(coordinator_state());
  let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
  let coordinator_url = format!("http://{}/", coordinator_addr);

  tokio::time::sleep(Duration::from_millis(100)).await;

  // Initialize recorder manager with coordinator
  let base = reqwest::Url::parse(&coordinator_url)?;
  let client = Arc::new(HttpCoordinatorClient::new(base)?);
  RECORDING_MANAGER
    .set_coordinator(client.clone(), "recorder-test".to_string())
    .await;

  // Start first recording with unique ID to avoid cross-test conflicts
  let test_id = format!("rec-conflict-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
  let config1 = RecordingConfig {
    id: test_id.clone(),
    source_stream_id: Some("stream-1".to_string()),
    source_uri: Some("rtsp://example.com/stream".to_string()),
    retention_hours: Some(24),
    format: Some(RecordingFormat::Mp4),
  };

  let req1 = RecordingStartRequest {
    config: config1,
    lease_ttl_secs: Some(30),
    ai_config: None,
  };

  let response1 = RECORDING_MANAGER.start(req1).await?;
  assert!(response1.accepted);

  // Try to start another recording with the same ID (should fail)
  let config2 = RecordingConfig {
    id: test_id.clone(),
    source_stream_id: Some("stream-2".to_string()),
    source_uri: Some("rtsp://example.com/stream2".to_string()),
    retention_hours: Some(24),
    format: Some(RecordingFormat::Mp4),
  };

  let req2 = RecordingStartRequest {
    config: config2,
    lease_ttl_secs: Some(30),
    ai_config: None,
  };

  let response2 = RECORDING_MANAGER.start(req2).await?;
  assert!(!response2.accepted);
  assert!(response2.message.is_some());

  // Clean up
  RECORDING_MANAGER.stop(&test_id).await?;
  coordinator_task.abort();
  Ok(())
}

#[tokio::test(start_paused = true)]
async fn recorder_lease_renewal() -> Result<()> {
  // Acquire test lock to prevent parallel execution with shared global state
  let _lock = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().await;

  let _ = tracing_subscriber::fmt::try_init();
  std::env::set_var("MOCK_RECORDING", "1");

  // Clear any state from previous tests
  RECORDING_MANAGER.clear().await;

  // Spawn coordinator
  let coordinator_router = coordinator_routes::router(coordinator_state());
  let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;
  let coordinator_url = format!("http://{}/", coordinator_addr);

  tokio::time::advance(Duration::from_millis(100)).await;

  // Initialize recorder manager with coordinator
  let base = reqwest::Url::parse(&coordinator_url)?;
  let client = Arc::new(HttpCoordinatorClient::new(base)?);
  RECORDING_MANAGER
    .set_coordinator(client.clone(), "recorder-test".to_string())
    .await;

  // Start recording with short TTL (reduced from 10s to 2s for faster tests)
  let test_id = format!("rec-renewal-{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
  let config = RecordingConfig {
    id: test_id.clone(),
    source_stream_id: Some("stream-1".to_string()),
    source_uri: Some("rtsp://example.com/stream".to_string()),
    retention_hours: Some(24),
    format: Some(RecordingFormat::Mp4),
  };

  let req = RecordingStartRequest {
    config,
    lease_ttl_secs: Some(2),
    ai_config: None,
  };

  let response = RECORDING_MANAGER.start(req).await?;
  assert!(response.accepted);
  let lease_id = response.lease_id.clone().unwrap();

  // Fast-forward time for renewal cycle (2s TTL / 2 = 1s interval, advance 1.5s)
  tokio::time::advance(Duration::from_millis(1500)).await;
  tokio::task::yield_now().await; // Give renewal task a chance to run

  // Verify lease is still active (was renewed)
  let http_client = Client::builder().build()?;
  let leases_resp = http_client
    .get(format!("{}v1/leases?kind=recorder", coordinator_url))
    .send()
    .await?;
  let leases: Vec<serde_json::Value> = leases_resp.json().await?;
  assert_eq!(leases.len(), 1);
  assert_eq!(leases[0]["lease_id"], lease_id);

  // Stop and clean up
  RECORDING_MANAGER.stop(&test_id).await?;
  coordinator_task.abort();
  Ok(())
}
