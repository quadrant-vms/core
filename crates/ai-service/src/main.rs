use ai_service::{api, config::AiServiceConfig, coordinator::HttpCoordinatorClient, plugin::mock_detector::MockDetectorPlugin, plugin::registry::PluginRegistry, AiServiceState};
use anyhow::Result;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize telemetry (logging and metrics)
    telemetry::init();

    info!("Starting AI Service...");

    // Load configuration from environment
    let config = AiServiceConfig::from_env()?;
    info!(
        "AI Service configuration: bind={}, node_id={}",
        config.bind_addr, config.node_id
    );

    // Initialize plugin registry
    let registry = PluginRegistry::new();

    // Register built-in plugins
    info!("Registering built-in plugins...");
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    registry.register(mock_detector).await?;
    info!("Registered mock_object_detector plugin");

    let plugin_count = registry.count().await;
    info!("Plugin registry initialized with {} plugins", plugin_count);

    // Create application state
    let state = if let Some(coordinator_url) = config.coordinator_url {
        info!("Connecting to coordinator at: {}", coordinator_url);
        let coordinator = Arc::new(HttpCoordinatorClient::new(coordinator_url)?);
        AiServiceState::with_coordinator(config.node_id.clone(), coordinator, registry)
    } else {
        info!("Running in standalone mode (no coordinator)");
        AiServiceState::new(config.node_id.clone(), registry)
    };

    // Build HTTP router
    let app = api::router(state.clone());

    // Bind and serve
    info!("Binding to {}", config.bind_addr);
    let listener = TcpListener::bind(&config.bind_addr).await?;
    info!("AI Service listening on {}", config.bind_addr);

    // Run with graceful shutdown
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    Ok(())
}

async fn shutdown_signal(state: AiServiceState) {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }

    info!("Shutting down gracefully...");
    if let Err(e) = state.shutdown().await {
        tracing::error!("Error during shutdown: {}", e);
    }
}
