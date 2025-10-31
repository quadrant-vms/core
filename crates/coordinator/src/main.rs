mod config;
mod error;
mod routes;
mod state;
mod store;

use crate::{
    config::CoordinatorConfig,
    state::CoordinatorState,
    store::{LeaseStore, MemoryLeaseStore},
};
use anyhow::Result;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    let config = CoordinatorConfig::from_env()?;
    let bind_addr = config.bind_addr;
    let store: Arc<dyn LeaseStore> = Arc::new(MemoryLeaseStore::new(
        config.default_ttl_secs,
        config.max_ttl_secs,
    ));
    let state = CoordinatorState::new(config, store);

    let app = routes::router(state.clone());
    let listener = TcpListener::bind(bind_addr).await?;

    info!(
        addr = %bind_addr,
        default_ttl = %state.config().default_ttl_secs,
        "coordinator listening"
    );

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
