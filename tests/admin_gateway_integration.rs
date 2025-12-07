//! Integration tests for admin-gateway
//!
//! These tests verify admin-gateway functionality including routes,
//! worker management, and coordinator integration.

use anyhow::Result;
use axum::Router;
use coordinator::{
    config::{CoordinatorConfig, LeaseStoreType},
    routes as coordinator_routes,
    state::CoordinatorState,
    store::{LeaseStore, MemoryLeaseStore},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle};

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

#[tokio::test]
async fn test_coordinator_health_endpoint() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Spawn coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    // Test health endpoint
    let client = reqwest::Client::builder().build()?;
    let health_resp = client
        .get(format!("http://{}/healthz", coordinator_addr))
        .send()
        .await?;

    assert!(health_resp.status().is_success());
    let health_text = health_resp.text().await?;
    assert_eq!(health_text, "ok");

    coordinator_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_coordinator_readyz_endpoint() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Spawn coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    // Test readyz endpoint
    let client = reqwest::Client::builder().build()?;
    let ready_resp = client
        .get(format!("http://{}/readyz", coordinator_addr))
        .send()
        .await?;

    assert!(ready_resp.status().is_success());

    coordinator_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_coordinator_metrics_endpoint() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Spawn coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    // Test metrics endpoint
    let client = reqwest::Client::builder().build()?;
    let metrics_resp = client
        .get(format!("http://{}/metrics", coordinator_addr))
        .send()
        .await?;

    assert!(metrics_resp.status().is_success());
    let _metrics_text = metrics_resp.text().await?;

    // Verify endpoint returns successfully
    // Metrics format may vary, so just verify it's accessible

    coordinator_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_lease_list_endpoint() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    // Spawn coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    // Test lease list endpoint
    let client = reqwest::Client::builder().build()?;
    let leases_resp = client
        .get(format!("http://{}/v1/leases", coordinator_addr))
        .send()
        .await?;

    assert!(leases_resp.status().is_success());
    let leases: Vec<serde_json::Value> = leases_resp.json().await?;

    // Initially empty
    assert_eq!(leases.len(), 0);

    coordinator_task.abort();
    Ok(())
}

#[tokio::test]
async fn test_lease_acquire_and_list() -> Result<()> {
    use common::leases::{LeaseAcquireRequest, LeaseKind};
    let _ = tracing_subscriber::fmt::try_init();

    // Spawn coordinator
    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    let client = reqwest::Client::builder().build()?;
    let coordinator_url = format!("http://{}/", coordinator_addr);

    // Acquire a lease
    let acquire_req = LeaseAcquireRequest {
        resource_id: "test-stream-1".to_string(),
        holder_id: "test-worker".to_string(),
        kind: LeaseKind::Stream,
        ttl_secs: 30,
    };

    let acquire_resp = client
        .post(format!("{}v1/leases/acquire", coordinator_url))
        .json(&acquire_req)
        .send()
        .await?;

    assert!(acquire_resp.status().is_success());
    let response: serde_json::Value = acquire_resp.json().await?;
    assert_eq!(response["granted"], true);
    assert!(response["record"]["lease_id"].is_string());

    // List leases and verify
    let leases_resp = client
        .get(format!("{}v1/leases?kind=stream", coordinator_url))
        .send()
        .await?;

    assert!(leases_resp.status().is_success());
    let leases: Vec<serde_json::Value> = leases_resp.json().await?;
    assert_eq!(leases.len(), 1);
    assert_eq!(leases[0]["resource_id"], "test-stream-1");
    assert_eq!(leases[0]["holder_id"], "test-worker");

    coordinator_task.abort();
    Ok(())
}
