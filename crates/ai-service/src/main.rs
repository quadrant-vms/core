use ai_service::{
    api, config::AiServiceConfig, coordinator::HttpCoordinatorClient,
    plugin::mock_detector::MockDetectorPlugin, plugin::registry::PluginRegistry,
    plugin::yolov8_detector::YoloV8DetectorPlugin, plugin::AiPlugin, AiServiceState,
};
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

    // Always register mock detector
    let mock_detector = Arc::new(RwLock::new(MockDetectorPlugin::new()));
    registry.register(mock_detector).await?;
    info!("Registered mock_object_detector plugin");

    // Register YOLOv8 detector if model file exists
    let yolov8_model_path = std::env::var("YOLOV8_MODEL_PATH")
        .unwrap_or_else(|_| "models/yolov8n.onnx".to_string());

    if std::path::Path::new(&yolov8_model_path).exists() {
        let mut yolov8 = YoloV8DetectorPlugin::new();
        let yolov8_config = serde_json::json!({
            "model_path": yolov8_model_path,
            "confidence_threshold": std::env::var("YOLOV8_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.5)
        });
        if let Err(e) = yolov8.init(yolov8_config).await {
            tracing::warn!("Failed to initialize YOLOv8 plugin: {}", e);
        } else {
            registry.register(Arc::new(RwLock::new(yolov8))).await?;
            info!("Registered yolov8_detector plugin with model: {}", yolov8_model_path);
        }
    } else {
        info!(
            "YOLOv8 model not found at '{}', skipping yolov8_detector plugin registration. \
            Set YOLOV8_MODEL_PATH environment variable to enable.",
            yolov8_model_path
        );
    }

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
