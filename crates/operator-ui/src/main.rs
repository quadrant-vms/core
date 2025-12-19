use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::info;

mod api;
mod config;
mod incident;
mod state;
mod websocket;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize telemetry
    telemetry::init();

    // Load configuration
    let config = Config::from_env()?;
    info!("Starting Operator UI on {}", config.bind_addr);
    info!("Frontend directory: {}", config.frontend_dir.display());

    // Initialize application state
    let state = AppState::new(config.clone()).await?;

    // Build API router
    let api_router = Router::new()
        // Health check
        .route("/healthz", get(api::health::health_check))
        .route("/readyz", get(api::health::ready_check))
        // Dashboard stats
        .route("/api/dashboard/stats", get(api::dashboard::get_stats))
        // Devices
        .route("/api/devices", get(api::devices::list_devices))
        .route("/api/devices/:id", get(api::devices::get_device))
        .route("/api/devices/:id/health", get(api::devices::get_device_health))
        // Streams
        .route("/api/streams", get(api::streams::list_streams))
        .route("/api/streams/:id", get(api::streams::get_stream))
        .route("/api/streams/:id/stop", post(api::streams::stop_stream))
        // Recordings
        .route("/api/recordings", get(api::recordings::list_recordings))
        .route("/api/recordings/search", post(api::recordings::search_recordings))
        .route("/api/recordings/:id", get(api::recordings::get_recording))
        .route("/api/recordings/:id/thumbnail", get(api::recordings::get_thumbnail))
        // AI Tasks
        .route("/api/ai/tasks", get(api::ai::list_tasks))
        .route("/api/ai/tasks/:id", get(api::ai::get_task))
        .route("/api/ai/detections", get(api::ai::list_detections))
        // Alerts
        .route("/api/alerts", get(api::alerts::list_alerts))
        .route("/api/alerts/:id", get(api::alerts::get_alert))
        .route("/api/alerts/rules", get(api::alerts::list_rules))
        .route("/api/alerts/rules/:id", get(api::alerts::get_rule))
        .route("/api/alerts/rules/:id/enable", post(api::alerts::enable_rule))
        .route("/api/alerts/rules/:id/disable", post(api::alerts::disable_rule))
        // Incidents
        .route("/api/incidents", get(api::incidents::list_incidents))
        .route("/api/incidents", post(api::incidents::create_incident))
        .route("/api/incidents/:id", get(api::incidents::get_incident))
        .route("/api/incidents/:id", post(api::incidents::update_incident))
        .route("/api/incidents/:id/acknowledge", post(api::incidents::acknowledge_incident))
        .route("/api/incidents/:id/resolve", post(api::incidents::resolve_incident))
        .route("/api/incidents/:id/notes", post(api::incidents::add_note))
        // WebSocket for real-time updates
        .route("/ws", get(websocket::ws_handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Serve static frontend files
    let frontend_service = ServeDir::new(&config.frontend_dir)
        .append_index_html_on_directories(true);

    // Combine API and frontend
    let app = Router::new()
        .nest("/", api_router)
        .fallback_service(frontend_service);

    // Start server
    let addr: SocketAddr = config.bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Operator UI listening on http://{}", addr);
    info!("API endpoints available at http://{}/api", addr);
    info!("WebSocket available at ws://{}/ws", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
