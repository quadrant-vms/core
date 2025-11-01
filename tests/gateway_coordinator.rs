use admin_gateway::{
    config::GatewayConfig,
    coordinator::HttpCoordinatorClient,
    routes as gateway_routes,
    state::AppState,
    worker::HttpWorkerClient,
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
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle, time::Duration};

use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};

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

async fn spawn_worker_server() -> Result<(SocketAddr, Arc<Mutex<Vec<String>>>, JoinHandle<()>)> {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let captured = calls.clone();
    let make_svc = make_service_fn(move |_| {
        let captured = captured.clone();
        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let captured = captured.clone();
                async move {
                    captured.lock().await.push(req.uri().to_string());
                    Ok::<_, Infallible>(Response::new(Body::from("ok")))
                }
            }))
        }
    });

    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await?;
    let addr = listener.local_addr()?;
    let server = Server::from_tcp(listener.into_std()?)?.serve(make_svc);
    let handle = tokio::spawn(async move {
        if let Err(err) = server.await {
            eprintln!("worker server error: {err}");
        }
    });
    Ok((addr, calls, handle))
}

#[tokio::test]
async fn gateway_and_coordinator_end_to_end() -> Result<()> {
    let _ = tracing_subscriber::fmt::try_init();

    let coordinator_router = coordinator_routes::router(coordinator_state());
    let (coordinator_addr, coordinator_task) = spawn_router(coordinator_router).await?;

    let coordinator_url = format!("http://{}", coordinator_addr);

    let (worker_addr, worker_calls, worker_task) = spawn_worker_server().await?;
    let worker_url = format!("http://{}/", worker_addr);

    let gateway_cfg = GatewayConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        coordinator_base_url: reqwest::Url::parse(&coordinator_url)?,
        node_id: "gateway-test".to_string(),
        worker_base_url: reqwest::Url::parse(&worker_url)?,
    };
    let coordinator_client = Arc::new(HttpCoordinatorClient::new(gateway_cfg.coordinator_base_url.clone())?);
    let worker_client = Arc::new(HttpWorkerClient::new(gateway_cfg.worker_base_url.clone())?);
    let app_state = AppState::new(gateway_cfg.clone(), coordinator_client, worker_client);
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
    worker_task.abort();

    let calls = worker_calls.lock().await.clone();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().any(|c| c.starts_with("/start")));
    assert!(calls.iter().any(|c| c.starts_with("/stop")));

    Ok(())
}
