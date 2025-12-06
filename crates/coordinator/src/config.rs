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

    Ok(Self {
      bind_addr,
      default_ttl_secs: default_ttl,
      max_ttl_secs: max_ttl.max(default_ttl),
      store_type,
      database_url,
    })
  }
}
