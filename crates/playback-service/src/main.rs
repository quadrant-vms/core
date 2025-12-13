mod api;
mod playback;
mod webrtc;

use anyhow::Result;
use playback::{PlaybackManager, PlaybackStore};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("Starting Playback Service");

    // Configuration
    let addr = std::env::var("PLAYBACK_SERVICE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8087".to_string());

    let node_id = std::env::var("NODE_ID")
        .unwrap_or_else(|_| format!("playback-node-{}", uuid::Uuid::new_v4()));

    let hls_base_url = std::env::var("HLS_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:8087/hls".to_string());

    let rtsp_base_url = std::env::var("RTSP_BASE_URL")
        .unwrap_or_else(|_| "rtsp://localhost:8554".to_string());

    let hls_root = std::env::var("HLS_ROOT")
        .unwrap_or_else(|_| "./data/hls".to_string());

    let recording_storage_root = std::env::var("RECORDING_STORAGE_ROOT")
        .unwrap_or_else(|_| "./data/recordings".to_string());

    // LL-HLS configuration
    let ll_hls_enabled = std::env::var("LL_HLS_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap_or(false);

    if ll_hls_enabled {
        info!("LL-HLS (Low-Latency HLS) support enabled");
    }

    // Initialize database connection if DATABASE_URL is provided
    let store = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        info!("Connecting to database: {}", database_url);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        // Run migrations
        info!("Running database migrations");
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;

        Some(Arc::new(PlaybackStore::new(pool)))
    } else {
        info!("DATABASE_URL not set, running without persistent storage");
        None
    };

    // Create playback manager
    let manager = Arc::new(PlaybackManager::new(
        store,
        node_id.clone(),
        hls_base_url,
        rtsp_base_url,
    ));

    // Create API router
    let api_router = api::create_router(manager.clone());

    // Create file serving router for HLS files
    let hls_serve_dir = ServeDir::new(&hls_root);
    let recording_serve_dir = ServeDir::new(&recording_storage_root);

    // Combine routes
    let app = axum::Router::new()
        .nest("/api", api_router)
        .nest_service("/hls/streams", hls_serve_dir)
        .nest_service("/hls/recordings", recording_serve_dir)
        .layer(CorsLayer::permissive());

    // Bind and serve
    info!("Playback Service listening on {}", addr);
    info!("Node ID: {}", node_id);
    info!("HLS files served from: {}", hls_root);
    info!("Recording files served from: {}", recording_storage_root);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
