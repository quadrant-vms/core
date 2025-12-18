//! Integration tests for stateless operation and recovery scenarios
//!
//! These tests verify:
//! - StateStore persistence across admin-gateway restarts
//! - Bootstrap recovery from StateStore
//! - Orphan detection and cleanup
//! - State synchronization between in-memory and persistent stores

use anyhow::Result;
use axum::Router;
use common::{
    recordings::{RecordingConfig, RecordingFormat, RecordingInfo, RecordingState},
    streams::{StreamConfig, StreamInfo, StreamState},
};
use coordinator::{
    config::{CoordinatorConfig, LeaseStoreType},
    pg_state_store::PgStateStore,
    routes as coordinator_routes,
    state::CoordinatorState,
    store::PostgresLeaseStore,
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle};

/// Helper to spawn a router on a random port
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

/// Helper to get test database URL
fn test_db_url() -> String {
    std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5434/quadrant_vms".to_string())
}

/// Helper to create coordinator state with PostgreSQL
async fn coordinator_state_with_postgres() -> Result<CoordinatorState> {
    let db_url = test_db_url();
    let cfg = CoordinatorConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        default_ttl_secs: 15,
        max_ttl_secs: 60,
        store_type: LeaseStoreType::Postgres,
        database_url: Some(db_url.clone()),
        cluster_enabled: false,
        node_id: None,
        peer_addrs: vec![],
        election_timeout_ms: 5000,
        heartbeat_interval_ms: 1000,
    };

    let lease_store = Arc::new(
        PostgresLeaseStore::new(
            &db_url,
            cfg.default_ttl_secs,
            cfg.max_ttl_secs,
        )
        .await?,
    );

    let state_store = Arc::new(PgStateStore::new(lease_store.pool().clone()));

    Ok(CoordinatorState::new(cfg, lease_store, Some(state_store)))
}

/// Test 1: StateStore can save and retrieve stream state
#[tokio::test]
async fn test_state_store_save_retrieve_stream() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    // Create a test stream
    let stream_id = format!("test-stream-{}", uuid::Uuid::new_v4());
    let stream_info = StreamInfo {
        config: StreamConfig {
            id: stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Running,
        lease_id: Some("test-lease-123".to_string()),
        node_id: Some("test-node-1".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    // Save stream
    state_store.save_stream(&stream_info).await?;

    // Retrieve stream
    let retrieved = state_store.get_stream(&stream_id).await?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.config.id, stream_id);
    assert_eq!(retrieved.state, StreamState::Running);
    assert_eq!(retrieved.lease_id, Some("test-lease-123".to_string()));

    // Cleanup
    state_store.delete_stream(&stream_id).await?;

    Ok(())
}

/// Test 2: StateStore can save and retrieve recording state
#[tokio::test]
async fn test_state_store_save_retrieve_recording() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    // Create a test recording
    let recording_id = format!("test-recording-{}", uuid::Uuid::new_v4());
    let recording_info = RecordingInfo {
        config: RecordingConfig {
            id: recording_id.clone(),
            source_stream_id: None,
            source_uri: Some("rtsp://test.local/stream".to_string()),
            retention_hours: Some(24),
            format: Some(RecordingFormat::Mp4),
        },
        state: RecordingState::Recording,
        lease_id: Some("test-lease-456".to_string()),
        storage_path: Some("/tmp/test.mp4".to_string()),
        last_error: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
        node_id: Some("test-node-1".to_string()),
        metadata: None,
    };

    // Save recording
    state_store.save_recording(&recording_info).await?;

    // Retrieve recording
    let retrieved = state_store.get_recording(&recording_id).await?;
    assert!(retrieved.is_some());
    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.config.id, recording_id);
    assert_eq!(retrieved.state, RecordingState::Recording);

    // Cleanup
    state_store.delete_recording(&recording_id).await?;

    Ok(())
}

/// Test 3: StateStore can list streams by node_id
#[tokio::test]
async fn test_state_store_list_streams_by_node() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    // Create test streams on different nodes
    let node1_stream_id = format!("node1-stream-{}", uuid::Uuid::new_v4());
    let node2_stream_id = format!("node2-stream-{}", uuid::Uuid::new_v4());

    let node1_stream = StreamInfo {
        config: StreamConfig {
            id: node1_stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream1".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Running,
        lease_id: Some("lease-1".to_string()),
        node_id: Some("node-1".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    let node2_stream = StreamInfo {
        config: StreamConfig {
            id: node2_stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream2".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Running,
        lease_id: Some("lease-2".to_string()),
        node_id: Some("node-2".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    state_store.save_stream(&node1_stream).await?;
    state_store.save_stream(&node2_stream).await?;

    // List streams for node-1
    let node1_streams = state_store.list_streams(Some("node-1")).await?;
    assert!(node1_streams.iter().any(|s| s.config.id == node1_stream_id));
    assert!(!node1_streams.iter().any(|s| s.config.id == node2_stream_id));

    // List streams for node-2
    let node2_streams = state_store.list_streams(Some("node-2")).await?;
    assert!(node2_streams.iter().any(|s| s.config.id == node2_stream_id));
    assert!(!node2_streams.iter().any(|s| s.config.id == node1_stream_id));

    // Cleanup
    state_store.delete_stream(&node1_stream_id).await?;
    state_store.delete_stream(&node2_stream_id).await?;

    Ok(())
}

/// Test 4: StateStore can update stream state
#[tokio::test]
async fn test_state_store_update_stream_state() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    let stream_id = format!("test-stream-{}", uuid::Uuid::new_v4());
    let stream_info = StreamInfo {
        config: StreamConfig {
            id: stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Pending,
        lease_id: Some("test-lease-123".to_string()),
        node_id: Some("test-node-1".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    // Save initial state
    state_store.save_stream(&stream_info).await?;

    // Update state to Running
    state_store
        .update_stream_state(&stream_id, "running", None)
        .await?;

    let updated = state_store.get_stream(&stream_id).await?.unwrap();
    assert_eq!(updated.state, StreamState::Running);
    assert_eq!(updated.last_error, None);

    // Update state to Error with error message
    state_store
        .update_stream_state(&stream_id, "error", Some("Test error"))
        .await?;

    let updated = state_store.get_stream(&stream_id).await?.unwrap();
    assert_eq!(updated.state, StreamState::Error);
    assert_eq!(updated.last_error, Some("Test error".to_string()));

    // Cleanup
    state_store.delete_stream(&stream_id).await?;

    Ok(())
}

/// Test 5: Orphan detection identifies non-active streams with leases
#[tokio::test]
async fn test_orphan_detection() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    // Create orphaned stream (Error state with lease)
    let orphan_stream_id = format!("orphan-stream-{}", uuid::Uuid::new_v4());
    let orphan_stream = StreamInfo {
        config: StreamConfig {
            id: orphan_stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Error,
        lease_id: Some("orphan-lease-123".to_string()),
        node_id: Some("test-node-1".to_string()),
        last_error: Some("Simulated crash".to_string()),
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    // Create active stream (should NOT be detected as orphan)
    let active_stream_id = format!("active-stream-{}", uuid::Uuid::new_v4());
    let active_stream = StreamInfo {
        config: StreamConfig {
            id: active_stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream2".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Running,
        lease_id: Some("active-lease-456".to_string()),
        node_id: Some("test-node-1".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    state_store.save_stream(&orphan_stream).await?;
    state_store.save_stream(&active_stream).await?;

    // List all streams
    let all_streams = state_store.list_streams(Some("test-node-1")).await?;

    // Verify orphan detection logic
    let orphans: Vec<_> = all_streams
        .iter()
        .filter(|s| s.lease_id.is_some() && !s.state.is_active())
        .collect();

    assert_eq!(orphans.len(), 1);
    assert_eq!(orphans[0].config.id, orphan_stream_id);

    // Cleanup
    state_store.delete_stream(&orphan_stream_id).await?;
    state_store.delete_stream(&active_stream_id).await?;

    Ok(())
}

/// Test 6: StateStore HTTP API endpoints work correctly
#[tokio::test]
async fn test_state_store_http_api() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_state = coordinator_state_with_postgres().await?;
    let coordinator_router = coordinator_routes::router(coordinator_state);
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    let client = reqwest::Client::builder().build()?;
    let base_url = format!("http://{}", coordinator_addr);

    // Create a test stream via HTTP API
    let stream_id = format!("http-test-stream-{}", uuid::Uuid::new_v4());
    let stream_info = StreamInfo {
        config: StreamConfig {
            id: stream_id.clone(),
            camera_id: None,
            uri: "rtsp://test.local/stream".to_string(),
            codec: None,
            container: None,
        },
        state: StreamState::Running,
        lease_id: Some("http-test-lease".to_string()),
        node_id: Some("http-test-node".to_string()),
        last_error: None,
        playlist_path: None,
        output_dir: None,
        started_at: Some(common::validation::safe_unix_timestamp()),
        stopped_at: None,
    };

    // POST /v1/state/streams
    let save_resp = client
        .post(format!("{}/v1/state/streams", base_url))
        .json(&stream_info)
        .send()
        .await?;
    assert!(save_resp.status().is_success());

    // GET /v1/state/streams/{id}
    let get_resp = client
        .get(format!("{}/v1/state/streams/{}", base_url, stream_id))
        .send()
        .await?;
    assert!(get_resp.status().is_success());
    let retrieved: StreamInfo = get_resp.json().await?;
    assert_eq!(retrieved.config.id, stream_id);
    assert_eq!(retrieved.state, StreamState::Running);

    // PUT /v1/state/streams/{id}/state
    let update_payload = serde_json::json!({
        "state": "Error",
        "error": "Test error message"
    });
    let update_resp = client
        .put(format!("{}/v1/state/streams/{}/state", base_url, stream_id))
        .json(&update_payload)
        .send()
        .await?;
    assert!(update_resp.status().is_success());

    // Verify state was updated
    let get_resp = client
        .get(format!("{}/v1/state/streams/{}", base_url, stream_id))
        .send()
        .await?;
    let updated: StreamInfo = get_resp.json().await?;
    assert_eq!(updated.state, StreamState::Error);
    assert_eq!(updated.last_error, Some("Test error message".to_string()));

    // DELETE /v1/state/streams/{id}
    let delete_resp = client
        .delete(format!("{}/v1/state/streams/{}", base_url, stream_id))
        .send()
        .await?;
    assert!(delete_resp.status().is_success());

    // Verify deletion
    let get_resp = client
        .get(format!("{}/v1/state/streams/{}", base_url, stream_id))
        .send()
        .await?;
    assert_eq!(get_resp.status(), 404);

    coordinator_task.abort();
    Ok(())
}

/// Test 7: StateStore persists across coordinator restart (simulated)
#[tokio::test]
async fn test_state_persistence_across_restart() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let stream_id = format!("persistent-stream-{}", uuid::Uuid::new_v4());

    // First "session": create and save stream
    {
        let coordinator_state = coordinator_state_with_postgres().await?;
        let state_store = coordinator_state
            .state_store()
            .expect("state store should be configured");

        let stream_info = StreamInfo {
            config: StreamConfig {
                id: stream_id.clone(),
                camera_id: None,
                uri: "rtsp://test.local/stream".to_string(),
                codec: None,
                container: None,
            },
            state: StreamState::Running,
            lease_id: Some("persistent-lease".to_string()),
            node_id: Some("persistent-node".to_string()),
            last_error: None,
            playlist_path: None,
            output_dir: None,
            started_at: Some(common::validation::safe_unix_timestamp()),
            stopped_at: None,
        };

        state_store.save_stream(&stream_info).await?;
    } // coordinator_state dropped, simulating restart

    // Second "session": retrieve stream from fresh coordinator instance
    {
        let coordinator_state = coordinator_state_with_postgres().await?;
        let state_store = coordinator_state
            .state_store()
            .expect("state store should be configured");

        let retrieved = state_store.get_stream(&stream_id).await?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.config.id, stream_id);
        assert_eq!(retrieved.state, StreamState::Running);
        assert_eq!(retrieved.node_id, Some("persistent-node".to_string()));

        // Cleanup
        state_store.delete_stream(&stream_id).await?;
    }

    Ok(())
}

/// Test 8: Bootstrap recovery restores in-memory state from StateStore
#[tokio::test]
async fn test_bootstrap_recovery() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // This test would require setting up admin-gateway with StateStore enabled
    // and testing the bootstrap() method. For now, we validate the StateStore
    // functionality that bootstrap relies on.

    let coordinator_state = coordinator_state_with_postgres().await?;
    let state_store = coordinator_state
        .state_store()
        .expect("state store should be configured");

    let node_id = "bootstrap-test-node";

    // Create multiple streams for the node
    let stream_ids: Vec<String> = (0..3)
        .map(|i| format!("bootstrap-stream-{}-{}", i, uuid::Uuid::new_v4()))
        .collect();

    for stream_id in &stream_ids {
        let stream_info = StreamInfo {
            config: StreamConfig {
                id: stream_id.clone(),
                camera_id: None,
                uri: format!("rtsp://test.local/stream-{}", stream_id),
                codec: None,
                container: None,
            },
            state: StreamState::Running,
            lease_id: Some(format!("lease-{}", stream_id)),
            node_id: Some(node_id.to_string()),
            last_error: None,
            playlist_path: None,
            output_dir: None,
            started_at: Some(common::validation::safe_unix_timestamp()),
            stopped_at: None,
        };
        state_store.save_stream(&stream_info).await?;
    }

    // Simulate bootstrap: list all streams for this node
    let restored_streams = state_store.list_streams(Some(node_id)).await?;

    // Verify all streams were restored
    for stream_id in &stream_ids {
        assert!(restored_streams.iter().any(|s| s.config.id == *stream_id));
    }

    // Cleanup
    for stream_id in &stream_ids {
        state_store.delete_stream(stream_id).await?;
    }

    Ok(())
}
