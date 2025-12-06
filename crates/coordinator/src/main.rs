use anyhow::{Context, Result};
use coordinator::{
  cluster::ClusterManager,
  config::{CoordinatorConfig, LeaseStoreType},
  routes,
  state::CoordinatorState,
  store::{LeaseStore, MemoryLeaseStore, PostgresLeaseStore},
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
  telemetry::init();

  let config = CoordinatorConfig::from_env()?;
  let bind_addr = config.bind_addr;

  let store: Arc<dyn LeaseStore> = match config.store_type {
    LeaseStoreType::Memory => {
      info!("using in-memory lease store");
      Arc::new(MemoryLeaseStore::new(
        config.default_ttl_secs,
        config.max_ttl_secs,
      ))
    }
    LeaseStoreType::Postgres => {
      let database_url = config
        .database_url
        .as_ref()
        .expect("DATABASE_URL required for Postgres");
      info!(url = %database_url, "using PostgreSQL lease store");
      Arc::new(
        PostgresLeaseStore::new(database_url, config.default_ttl_secs, config.max_ttl_secs)
          .await?,
      )
    }
  };

  let state = if config.cluster_enabled {
    let node_id = config
      .node_id
      .clone()
      .context("NODE_ID required when clustering is enabled")?;
    let node_addr = config.bind_addr.to_string();
    let peer_addrs = config.peer_addrs.clone();

    info!(
      node_id = %node_id,
      peers = ?peer_addrs,
      "clustering enabled"
    );

    let cluster = Arc::new(ClusterManager::new(
      node_id,
      node_addr,
      peer_addrs,
      config.election_timeout_ms,
      config.heartbeat_interval_ms,
    ));

    let election_monitor = cluster.clone();
    tokio::spawn(async move {
      election_monitor.start_election_monitor().await;
    });

    let heartbeat_sender = cluster.clone();
    tokio::spawn(async move {
      heartbeat_sender.start_heartbeat_sender().await;
    });

    CoordinatorState::with_cluster(config.clone(), store, cluster)
  } else {
    info!("clustering disabled, running as standalone coordinator");
    CoordinatorState::new(config.clone(), store)
  };

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
    use tokio::signal::unix::{SignalKind, signal};
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
