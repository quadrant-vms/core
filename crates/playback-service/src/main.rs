use anyhow::Result;
use playback_service::{api, cache, playback};
use cache::{CacheConfig, EdgeCache};
use playback::{PlaybackManager, PlaybackStore};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
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

    // Edge cache configuration
    let cache_enabled = std::env::var("EDGE_CACHE_ENABLED")
        .unwrap_or_else(|_| "true".to_string())
        .parse::<bool>()
        .unwrap_or(true);

    let cache_max_items = std::env::var("EDGE_CACHE_MAX_ITEMS")
        .unwrap_or_else(|_| "10000".to_string())
        .parse::<usize>()
        .unwrap_or(10000);

    let cache_max_size_mb = std::env::var("EDGE_CACHE_MAX_SIZE_MB")
        .unwrap_or_else(|_| "1024".to_string())
        .parse::<usize>()
        .unwrap_or(1024);

    let cache_playlist_ttl_secs = std::env::var("EDGE_CACHE_PLAYLIST_TTL_SECS")
        .unwrap_or_else(|_| "2".to_string())
        .parse::<u64>()
        .unwrap_or(2);

    let cache_segment_ttl_secs = std::env::var("EDGE_CACHE_SEGMENT_TTL_SECS")
        .unwrap_or_else(|_| "60".to_string())
        .parse::<u64>()
        .unwrap_or(60);

    let cache_config = CacheConfig {
        max_items: cache_max_items,
        max_size_bytes: cache_max_size_mb * 1024 * 1024,
        playlist_ttl: Duration::from_secs(cache_playlist_ttl_secs),
        segment_ttl: Duration::from_secs(cache_segment_ttl_secs),
        enabled: cache_enabled,
    };

    let edge_cache = Arc::new(EdgeCache::new(cache_config.clone()));

    if cache_enabled {
        info!(
            "Edge cache enabled: max_items={}, max_size={}MB, playlist_ttl={}s, segment_ttl={}s",
            cache_max_items,
            cache_max_size_mb,
            cache_playlist_ttl_secs,
            cache_segment_ttl_secs
        );
    } else {
        info!("Edge cache disabled");
    }

    // Initialize database connection if DATABASE_URL is provided
    let store = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        info!("Connecting to database: {}", database_url);

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;

        // Run migrations (commented out - run migrations manually)
        // info!("Running database migrations");
        // sqlx::migrate!()
        //     .run(&pool)
        //     .await?;

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
    let api_router = api::create_router(manager.clone(), edge_cache.clone());

    // Create file serving router for HLS files
    let hls_serve_dir = ServeDir::new(&hls_root);
    let recording_serve_dir = ServeDir::new(&recording_storage_root);

    // Combine routes
    let app = axum::Router::new()
        .nest("/api", api_router)
        .nest_service("/hls/streams", hls_serve_dir)
        .nest_service("/hls/recordings", recording_serve_dir)
        .layer(axum::middleware::from_fn_with_state(
            edge_cache.clone(),
            cache::middleware::cache_layer,
        ))
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
