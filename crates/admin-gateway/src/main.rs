mod config;
mod coordinator;
mod error;
mod routes;
mod state;

use crate::{
    config::GatewayConfig,
    coordinator::{CoordinatorClient, HttpCoordinatorClient},
    state::AppState,
};
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    let config = GatewayConfig::from_env()?;
    let coordinator: Arc<dyn CoordinatorClient> =
        Arc::new(HttpCoordinatorClient::new(config.coordinator_base_url.clone())?);
    let state = AppState::new(config.clone(), coordinator);

    let app = routes::router(state.clone());
    let listener = TcpListener::bind(config.bind_addr).await?;

    info!(addr = %config.bind_addr, node_id = %config.node_id, "admin-gateway listening");

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
