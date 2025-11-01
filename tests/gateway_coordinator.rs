use admin_gateway::{
    config::GatewayConfig,
    coordinator::HttpCoordinatorClient,
    routes as gateway_routes,
    state::AppState,
};
use anyhow::Result;
use axum::Router;
use coordinator::{
    config::CoordinatorConfig,
    routes as coordinator_routes,
    state::CoordinatorState,
    store::{LeaseStore, MemoryLeaseStore},
};
use reqwest::Client;
use std::{net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, task::JoinHandle, time::Duration};

fn coordinator_state() -> CoordinatorState {
    let cfg = CoordinatorConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        default_ttl_secs: 15,
        max_ttl_secs: 60,
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

#[tokio::test]
async fn gateway_and_coordinator_end_to_end() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    let coordinator_url = format!("http://{}", coordinator_addr);

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "gateway-test".to_string(),
    };
    let coordinator_client = Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let app_state = AppState::new(gateway_cfg.clone(), coordinator_client);
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

    Ok(())
}
