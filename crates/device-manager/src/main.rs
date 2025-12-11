use anyhow::{Context, Result};
use device_manager::{
    DeviceManagerState, DeviceProber, DeviceStore, FirmwareExecutor, FirmwareStorage,
    HealthMonitor, OnvifDiscoveryClient, TourExecutor,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    // Load configuration from environment
    let bind_addr: std::net::SocketAddr = std::env::var("DEVICE_MANAGER_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8084".to_string())
        .parse()
        .context("invalid bind address")?;

    let database_url = std::env::var("DATABASE_URL")
        .context("DATABASE_URL environment variable required")?;

    let probe_timeout_secs = std::env::var("PROBE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let health_check_interval_secs = std::env::var("HEALTH_CHECK_INTERVAL_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);

    let max_consecutive_failures = std::env::var("MAX_CONSECUTIVE_FAILURES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);

    let ptz_timeout_secs = std::env::var("PTZ_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let discovery_timeout_secs = std::env::var("DISCOVERY_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);

    let firmware_storage_root = std::env::var("FIRMWARE_STORAGE_ROOT")
        .unwrap_or_else(|_| "./data/firmware".to_string());

    // Initialize store
    info!("connecting to database");
    let store = Arc::new(DeviceStore::new(&database_url).await?);

    // Initialize prober
    let prober = Arc::new(DeviceProber::new(probe_timeout_secs));

    // Initialize tour executor
    let tour_executor = Arc::new(TourExecutor::new(Arc::clone(&store), ptz_timeout_secs));

    // Initialize discovery client
    let discovery_client = Arc::new(OnvifDiscoveryClient::new(discovery_timeout_secs));

    // Initialize firmware storage
    info!("initializing firmware storage at {}", firmware_storage_root);
    let firmware_storage = Arc::new(
        FirmwareStorage::new(&firmware_storage_root)
            .context("failed to create firmware storage")?,
    );
    firmware_storage
        .init()
        .await
        .context("failed to initialize firmware storage")?;

    // Initialize firmware executor
    let firmware_executor = Arc::new(FirmwareExecutor::new(
        (*store).clone(),
        (*firmware_storage).clone(),
    ));

    // Create state
    let state = DeviceManagerState::new(
        Arc::clone(&store),
        Arc::clone(&prober),
        Arc::clone(&tour_executor),
        Arc::clone(&discovery_client),
        Arc::clone(&firmware_executor),
        Arc::clone(&firmware_storage),
    );

    // Start health monitor in background
    let health_monitor = HealthMonitor::new(
        Arc::clone(&store),
        Arc::clone(&prober),
        health_check_interval_secs,
        max_consecutive_failures,
    );

    tokio::spawn(async move {
        health_monitor.start().await;
    });

    // Create router
    let app = device_manager::routes::router(state);

    // Start server
    let listener = TcpListener::bind(bind_addr).await?;
    info!(addr = %bind_addr, "device-manager listening");

    axum::serve(listener, app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            let _ = sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received");
}
