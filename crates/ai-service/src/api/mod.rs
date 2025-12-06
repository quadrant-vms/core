pub mod routes;

use crate::state::AiServiceState;
use axum::{routing::{get, post}, Router};
use tower_http::trace::TraceLayer;

/// Build the API router
pub fn router(state: AiServiceState) -> Router {
    Router::new()
        // Health and metrics endpoints
        .route("/healthz", get(routes::healthz))
        .route("/readyz", get(routes::readyz))
        .route("/metrics", get(routes::metrics))
        // Plugin endpoints
        .route("/v1/plugins", get(routes::list_plugins))
        .route("/v1/plugins/:id", get(routes::get_plugin))
        // Task endpoints
        .route("/v1/tasks", get(routes::list_tasks).post(routes::start_task))
        .route("/v1/tasks/:id", get(routes::get_task).delete(routes::stop_task))
        .route("/v1/tasks/:id/frames", post(routes::submit_frame))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
