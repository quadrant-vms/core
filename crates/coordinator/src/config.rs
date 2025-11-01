use anyhow::{Context, Result};
use std::{env, net::SocketAddr};

#[derive(Clone)]
pub struct CoordinatorConfig {
  pub bind_addr: SocketAddr,
  pub default_ttl_secs: u64,
  pub max_ttl_secs: u64,
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

    Ok(Self {
      bind_addr,
      default_ttl_secs: default_ttl,
      max_ttl_secs: max_ttl.max(default_ttl),
    })
  }
}
