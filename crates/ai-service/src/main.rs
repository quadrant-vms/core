use ai_service::{
    api, config::AiServiceConfig, coordinator::HttpCoordinatorClient,
    plugin::action_recognition::ActionRecognitionPlugin,
    plugin::facial_recognition::FacialRecognitionPlugin, plugin::lpr::LprPlugin,
    plugin::mock_detector::MockDetectorPlugin, plugin::pose_estimation::PoseEstimationPlugin,
    plugin::registry::PluginRegistry, plugin::yolov8_detector::YoloV8DetectorPlugin,
    plugin::AiPlugin, AiServiceState,
};
use anyhow::Result;
use common::state_store::StateStore;
use common::state_store_client::StateStoreClient;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::RwLock};
use tracing::{info, warn};

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

    // Register Pose Estimation plugin if model file exists
    let pose_model_path = std::env::var("POSE_MODEL_PATH")
        .unwrap_or_else(|_| "models/movenet.onnx".to_string());

    if std::path::Path::new(&pose_model_path).exists() {
        let mut pose_plugin = PoseEstimationPlugin::new();
        let pose_config = serde_json::json!({
            "model_path": pose_model_path,
            "pose_confidence_threshold": std::env::var("POSE_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.5),
            "keypoint_confidence_threshold": std::env::var("POSE_KEYPOINT_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.3)
        });
        if let Err(e) = pose_plugin.init(pose_config).await {
            tracing::warn!("Failed to initialize Pose Estimation plugin: {}", e);
        } else {
            registry.register(Arc::new(RwLock::new(pose_plugin))).await?;
            info!("Registered pose_estimation plugin with model: {}", pose_model_path);
        }
    } else {
        info!(
            "Pose estimation model not found at '{}', skipping pose_estimation plugin registration. \
            Set POSE_MODEL_PATH environment variable to enable.",
            pose_model_path
        );
    }

    // Register License Plate Recognition (LPR) plugin if model files exist
    let lpr_detection_model = std::env::var("LPR_DETECTION_MODEL")
        .unwrap_or_else(|_| "models/lpr_detector.onnx".to_string());

    if std::path::Path::new(&lpr_detection_model).exists() {
        let mut lpr_plugin = LprPlugin::new();
        let lpr_ocr_model = std::env::var("LPR_OCR_MODEL").ok();

        let lpr_config = serde_json::json!({
            "detection_model_path": lpr_detection_model,
            "ocr_model_path": lpr_ocr_model,
            "confidence_threshold": std::env::var("LPR_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.6)
        });
        if let Err(e) = lpr_plugin.init(lpr_config).await {
            tracing::warn!("Failed to initialize LPR plugin: {}", e);
        } else {
            registry.register(Arc::new(RwLock::new(lpr_plugin))).await?;
            info!("Registered lpr plugin with detection model: {}", lpr_detection_model);
        }
    } else {
        info!(
            "LPR detection model not found at '{}', skipping lpr plugin registration. \
            Set LPR_DETECTION_MODEL and optionally LPR_OCR_MODEL environment variables to enable.",
            lpr_detection_model
        );
    }

    // Register Facial Recognition plugin if model files exist
    let face_detection_model = std::env::var("FACE_DETECTION_MODEL")
        .unwrap_or_else(|_| "models/face_detector.onnx".to_string());

    if std::path::Path::new(&face_detection_model).exists() {
        let mut face_recognition_plugin = FacialRecognitionPlugin::new();
        let face_embedding_model = std::env::var("FACE_EMBEDDING_MODEL").ok();

        let face_recognition_config = serde_json::json!({
            "detection_model_path": face_detection_model,
            "embedding_model_path": face_embedding_model,
            "confidence_threshold": std::env::var("FACE_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.6),
            "similarity_threshold": std::env::var("FACE_SIMILARITY_THRESHOLD")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.5)
        });
        if let Err(e) = face_recognition_plugin.init(face_recognition_config).await {
            tracing::warn!("Failed to initialize Facial Recognition plugin: {}", e);
        } else {
            registry.register(Arc::new(RwLock::new(face_recognition_plugin))).await?;
            info!("Registered facial_recognition plugin with detection model: {}", face_detection_model);
        }
    } else {
        info!(
            "Face detection model not found at '{}', skipping facial_recognition plugin registration. \
            Set FACE_DETECTION_MODEL and optionally FACE_EMBEDDING_MODEL environment variables to enable.",
            face_detection_model
        );
    }

    // Register Action Recognition plugin if model file exists
    let action_model_path = std::env::var("ACTION_RECOGNITION_MODEL")
        .unwrap_or_else(|_| "models/action_recognition.onnx".to_string());

    if std::path::Path::new(&action_model_path).exists() {
        let mut action_plugin = ActionRecognitionPlugin::new();
        let action_config = serde_json::json!({
            "model_path": action_model_path,
            "confidence_threshold": std::env::var("ACTION_CONFIDENCE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.6),
            "temporal_window": std::env::var("ACTION_TEMPORAL_WINDOW")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(16)
        });
        if let Err(e) = action_plugin.init(action_config).await {
            tracing::warn!("Failed to initialize Action Recognition plugin: {}", e);
        } else {
            registry.register(Arc::new(RwLock::new(action_plugin))).await?;
            info!("Registered action_recognition plugin with model: {}", action_model_path);
        }
    } else {
        info!(
            "Action recognition model not found at '{}', skipping action_recognition plugin registration. \
            Set ACTION_RECOGNITION_MODEL environment variable to enable.",
            action_model_path
        );
    }

    let plugin_count = registry.count().await;
    info!("Plugin registry initialized with {} plugins", plugin_count);

    // Create application state
    let state_store_enabled = std::env::var("ENABLE_STATE_STORE")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase() == "true";

    let state = if let Some(coordinator_url) = config.coordinator_url {
        info!("Connecting to coordinator at: {}", coordinator_url);
        let coordinator = Arc::new(HttpCoordinatorClient::new(coordinator_url.clone())?);

        if state_store_enabled {
            let state_store: Arc<dyn StateStore> = Arc::new(StateStoreClient::new(coordinator_url.to_string()));
            let state = AiServiceState::with_coordinator_and_state_store(
                config.node_id.clone(),
                coordinator,
                registry,
                state_store,
            );

            // Bootstrap: restore state from StateStore
            if let Err(e) = state.bootstrap().await {
                warn!(error = %e, "failed to bootstrap state from StateStore");
            } else {
                info!("state store enabled and bootstrapped");
            }

            state
        } else {
            AiServiceState::with_coordinator(config.node_id.clone(), coordinator, registry)
        }
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
