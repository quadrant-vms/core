use anyhow::{Context, Result};
use std::{env, net::SocketAddr};

#[derive(Clone, Debug, PartialEq)]
pub enum LeaseStoreType {
  Memory,
  Postgres,
}

#[derive(Clone)]
pub struct CoordinatorConfig {
  pub bind_addr: SocketAddr,
  pub default_ttl_secs: u64,
  pub max_ttl_secs: u64,
  pub store_type: LeaseStoreType,
  pub database_url: Option<String>,
  pub cluster_enabled: bool,
  pub node_id: Option<String>,
  pub peer_addrs: Vec<String>,
  pub election_timeout_ms: u64,
  pub heartbeat_interval_ms: u64,
}

impl CoordinatorConfig {
  pub fn from_env() -> Result<Self> {
    let bind = env::var("COORDINATOR_ADDR").unwrap_or_else(|_| "0.0.0.0:8082".to_string());
    let bind_addr: SocketAddr = bind.parse().context("invalid COORDINATOR_ADDR")?;

    let default_ttl = env::var("LEASE_DEFAULT_TTL_SECS")
      .ok()
      .and_then(|v| v.parse::<u64>().ok())
      .unwrap_or(30);

    let max_ttl = env::var("LEASE_MAX_TTL_SECS")
      .ok()
      .and_then(|v| v.parse::<u64>().ok())
      .unwrap_or(300);

    let store_type_str = env::var("LEASE_STORE_TYPE").unwrap_or_else(|_| "memory".to_string());
    let store_type = match store_type_str.to_lowercase().as_str() {
      "postgres" | "postgresql" => LeaseStoreType::Postgres,
      _ => LeaseStoreType::Memory,
    };

    let database_url = if store_type == LeaseStoreType::Postgres {
      Some(env::var("DATABASE_URL").context("DATABASE_URL required for Postgres store")?)
    } else {
      env::var("DATABASE_URL").ok()
    };

    let cluster_enabled = env::var("CLUSTER_ENABLED")
      .ok()
      .and_then(|v| v.parse::<bool>().ok())
      .unwrap_or(false);

    let node_id = env::var("NODE_ID").ok();

    let peer_addrs = env::var("CLUSTER_PEERS")
      .ok()
      .map(|s| {
        s.split(',')
          .map(|addr| addr.trim().to_string())
          .filter(|addr| !addr.is_empty())
          .collect()
      })
      .unwrap_or_default();

    let election_timeout_ms = env::var("ELECTION_TIMEOUT_MS")
      .ok()
      .and_then(|v| v.parse::<u64>().ok())
      .unwrap_or(5000);

    let heartbeat_interval_ms = env::var("HEARTBEAT_INTERVAL_MS")
      .ok()
      .and_then(|v| v.parse::<u64>().ok())
      .unwrap_or(1000);

    Ok(Self {
      bind_addr,
      default_ttl_secs: default_ttl,
      max_ttl_secs: max_ttl.max(default_ttl),
      store_type,
      database_url,
      cluster_enabled,
      node_id,
      peer_addrs,
      election_timeout_ms,
      heartbeat_interval_ms,
    })
  }
}
