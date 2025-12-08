use anyhow::{Context, Result};
use async_trait::async_trait;
use common::leases::{
  LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest,
  LeaseReleaseResponse, LeaseRenewRequest, LeaseRenewResponse,
};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{
  collections::HashMap,
  time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use uuid::Uuid;

#[async_trait]
pub trait LeaseStore: Send + Sync {
  async fn acquire(&self, request: LeaseAcquireRequest) -> Result<LeaseAcquireResponse>;
  async fn renew(&self, request: LeaseRenewRequest) -> Result<LeaseRenewResponse>;
  async fn release(&self, request: LeaseReleaseRequest) -> Result<LeaseReleaseResponse>;
  async fn list(&self, kind: Option<LeaseKind>) -> Result<Vec<LeaseRecord>>;
  async fn health_check(&self) -> Result<bool>;
}

#[derive(Default)]
pub struct MemoryLeaseStore {
  inner: RwLock<StoreInner>,
  default_ttl: u64,
  max_ttl: u64,
}

impl MemoryLeaseStore {
  pub fn new(default_ttl: u64, max_ttl: u64) -> Self {
    Self {
      inner: RwLock::new(StoreInner::default()),
      default_ttl,
      max_ttl: max_ttl.max(default_ttl),
    }
  }

  fn normalize_ttl(&self, ttl: u64) -> u64 {
    let ttl = if ttl == 0 { self.default_ttl } else { ttl };
    ttl.min(self.max_ttl).max(5)
  }

  fn now_epoch_secs() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  fn purge_expired(inner: &mut StoreInner, now: u64) {
    let mut stale = Vec::new();
    for (resource, record) in inner.by_resource.iter() {
      if record.expires_at_epoch_secs <= now {
        stale.push((resource.clone(), record.lease_id.clone()));
      }
    }
    for (resource, lease_id) in stale {
      inner.by_resource.remove(&resource);
      inner.lease_to_resource.remove(&lease_id);
    }
  }
}

#[derive(Default)]
struct StoreInner {
  by_resource: HashMap<String, LeaseRecord>,
  lease_to_resource: HashMap<String, String>,
  version_counter: u64,
}

#[async_trait]
impl LeaseStore for MemoryLeaseStore {
  async fn acquire(&self, request: LeaseAcquireRequest) -> Result<LeaseAcquireResponse> {
    let ttl = self.normalize_ttl(request.ttl_secs);
    let mut inner = self.inner.write().await;
    let now = Self::now_epoch_secs();
    Self::purge_expired(&mut inner, now);

    let mut remove_lease: Option<String> = None;
    if let Some(existing) = inner.by_resource.get_mut(&request.resource_id) {
      if existing.expires_at_epoch_secs > now {
        if existing.holder_id == request.holder_id {
          existing.expires_at_epoch_secs = now + ttl;
          existing.version += 1;
          return Ok(LeaseAcquireResponse {
            granted: true,
            record: Some(existing.clone()),
          });
        } else {
          return Ok(LeaseAcquireResponse {
            granted: false,
            record: Some(existing.clone()),
          });
        }
      } else {
        // expired; reclaim outside borrow scope
        remove_lease = Some(existing.lease_id.clone());
      }
    }

    if let Some(lease_id) = remove_lease {
      inner.by_resource.remove(&request.resource_id);
      inner.lease_to_resource.remove(&lease_id);
    }

    let lease_id = Uuid::new_v4().to_string();
    inner.version_counter = inner.version_counter.wrapping_add(1);
    let record = LeaseRecord {
      lease_id: lease_id.clone(),
      resource_id: request.resource_id.clone(),
      holder_id: request.holder_id.clone(),
      kind: request.kind,
      expires_at_epoch_secs: now + ttl,
      version: inner.version_counter,
    };

    inner
      .lease_to_resource
      .insert(lease_id.clone(), request.resource_id.clone());
    inner
      .by_resource
      .insert(request.resource_id, record.clone());

    Ok(LeaseAcquireResponse {
      granted: true,
      record: Some(record),
    })
  }

  async fn renew(&self, request: LeaseRenewRequest) -> Result<LeaseRenewResponse> {
    let ttl = self.normalize_ttl(request.ttl_secs);
    let mut inner = self.inner.write().await;
    let now = Self::now_epoch_secs();
    Self::purge_expired(&mut inner, now);

    let Some(resource_id) = inner.lease_to_resource.get(&request.lease_id).cloned() else {
      return Ok(LeaseRenewResponse {
        renewed: false,
        record: None,
      });
    };

    if let Some(record) = inner.by_resource.get_mut(&resource_id) {
      if record.expires_at_epoch_secs <= now {
        inner.by_resource.remove(&resource_id);
        inner.lease_to_resource.remove(&request.lease_id);
        return Ok(LeaseRenewResponse {
          renewed: false,
          record: None,
        });
      }

      record.expires_at_epoch_secs = now + ttl;
      record.version += 1;
      return Ok(LeaseRenewResponse {
        renewed: true,
        record: Some(record.clone()),
      });
    }

    Ok(LeaseRenewResponse {
      renewed: false,
      record: None,
    })
  }

  async fn release(&self, request: LeaseReleaseRequest) -> Result<LeaseReleaseResponse> {
    let mut inner = self.inner.write().await;
    let now = Self::now_epoch_secs();
    Self::purge_expired(&mut inner, now);

    let Some(resource_id) = inner.lease_to_resource.remove(&request.lease_id) else {
      return Ok(LeaseReleaseResponse { released: false });
    };

    inner.by_resource.remove(&resource_id);
    Ok(LeaseReleaseResponse { released: true })
  }

  async fn list(&self, kind: Option<LeaseKind>) -> Result<Vec<LeaseRecord>> {
    let mut inner = self.inner.write().await;
    let now = Self::now_epoch_secs();
    Self::purge_expired(&mut inner, now);

    let mut out: Vec<LeaseRecord> = inner.by_resource.values().cloned().collect();
    if let Some(kind) = kind {
      out.retain(|r| r.kind == kind);
    }
    out.sort_by(|a, b| a.resource_id.cmp(&b.resource_id));
    Ok(out)
  }

  async fn health_check(&self) -> Result<bool> {
    // Memory store is always healthy if we can acquire the lock
    let _inner = self.inner.read().await;
    Ok(true)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tokio::time::{Duration, sleep};

  fn store() -> MemoryLeaseStore {
    MemoryLeaseStore::new(10, 60)
  }

  #[tokio::test]
  async fn acquire_new_lease() {
    let store = store();
    let req = LeaseAcquireRequest {
      resource_id: "cam1".into(),
      holder_id: "node-a".into(),
      kind: LeaseKind::Stream,
      ttl_secs: 10,
    };
    let resp = store.acquire(req).await.unwrap();
    assert!(resp.granted);
    let record = resp.record.expect("record");
    assert_eq!(record.resource_id, "cam1");
    assert_eq!(record.holder_id, "node-a");
  }

  #[tokio::test]
  async fn acquire_conflict() {
    let store = store();
    let req1 = LeaseAcquireRequest {
      resource_id: "cam1".into(),
      holder_id: "node-a".into(),
      kind: LeaseKind::Stream,
      ttl_secs: 10,
    };
    let req2 = LeaseAcquireRequest {
      resource_id: "cam1".into(),
      holder_id: "node-b".into(),
      kind: LeaseKind::Stream,
      ttl_secs: 10,
    };
    let first = store.acquire(req1).await.unwrap();
    assert!(first.granted);
    let second = store.acquire(req2).await.unwrap();
    assert!(!second.granted);
    assert_eq!(
      second.record.unwrap().holder_id,
      "node-a",
      "existing holder should be returned"
    );
  }

  #[tokio::test]
  async fn renew_extends_ttl() {
    let store = store();
    let req = LeaseAcquireRequest {
      resource_id: "cam1".into(),
      holder_id: "node-a".into(),
      kind: LeaseKind::Stream,
      ttl_secs: 2,
    };
    let resp = store.acquire(req).await.unwrap();
    let lease = resp.record.unwrap();
    sleep(Duration::from_secs(1)).await;
    let renew = store
      .renew(LeaseRenewRequest {
        lease_id: lease.lease_id.clone(),
        ttl_secs: 5,
      })
      .await
      .unwrap();
    assert!(renew.renewed);
    let renewed = renew.record.unwrap();
    assert!(renewed.expires_at_epoch_secs > lease.expires_at_epoch_secs);
  }

  #[tokio::test]
  async fn expired_lease_can_be_reacquired() {
    let store = MemoryLeaseStore::new(5, 60);
    let resp = store
      .acquire(LeaseAcquireRequest {
        resource_id: "cam1".into(),
        holder_id: "node-a".into(),
        kind: LeaseKind::Stream,
        ttl_secs: 5,
      })
      .await
      .unwrap();
    assert!(resp.granted);
    sleep(Duration::from_secs(6)).await;
    let resp2 = store
      .acquire(LeaseAcquireRequest {
        resource_id: "cam1".into(),
        holder_id: "node-b".into(),
        kind: LeaseKind::Stream,
        ttl_secs: 5,
      })
      .await
      .unwrap();
    assert!(resp2.granted);
    assert_eq!(resp2.record.unwrap().holder_id, "node-b");
  }

  #[tokio::test]
  async fn release_frees_lease() {
    let store = store();
    let lease = store
      .acquire(LeaseAcquireRequest {
        resource_id: "cam1".into(),
        holder_id: "node-a".into(),
        kind: LeaseKind::Stream,
        ttl_secs: 10,
      })
      .await
      .unwrap()
      .record
      .unwrap();
    let release = store
      .release(LeaseReleaseRequest {
        lease_id: lease.lease_id.clone(),
      })
      .await
      .unwrap();
    assert!(release.released);
    let reacquire = store
      .acquire(LeaseAcquireRequest {
        resource_id: "cam1".into(),
        holder_id: "node-b".into(),
        kind: LeaseKind::Stream,
        ttl_secs: 10,
      })
      .await
      .unwrap();
    assert!(reacquire.granted);
    assert_eq!(reacquire.record.unwrap().holder_id, "node-b");
  }
}

pub struct PostgresLeaseStore {
  pool: PgPool,
  default_ttl: u64,
  max_ttl: u64,
}

impl PostgresLeaseStore {
  pub async fn new(database_url: &str, default_ttl: u64, max_ttl: u64) -> Result<Self> {
    let pool = PgPoolOptions::new()
      .max_connections(10)
      .connect(database_url)
      .await
      .context("failed to connect to PostgreSQL")?;

    sqlx::migrate!("./migrations")
      .run(&pool)
      .await
      .context("failed to run database migrations")?;

    Ok(Self {
      pool,
      default_ttl,
      max_ttl: max_ttl.max(default_ttl),
    })
  }

  /// Expose the database pool for use by StateStore
  pub fn pool(&self) -> &PgPool {
    &self.pool
  }

  fn normalize_ttl(&self, ttl: u64) -> u64 {
    let ttl = if ttl == 0 { self.default_ttl } else { ttl };
    ttl.min(self.max_ttl).max(5)
  }

  fn now_epoch_secs() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  async fn purge_expired(&self) -> Result<()> {
    let now = Self::now_epoch_secs() as i64;
    sqlx::query("DELETE FROM leases WHERE expires_at_epoch_secs <= $1")
      .bind(now)
      .execute(&self.pool)
      .await
      .context("failed to purge expired leases")?;
    Ok(())
  }
}

#[async_trait]
impl LeaseStore for PostgresLeaseStore {
  async fn acquire(&self, request: LeaseAcquireRequest) -> Result<LeaseAcquireResponse> {
    let ttl = self.normalize_ttl(request.ttl_secs);
    let now = Self::now_epoch_secs();

    self.purge_expired().await?;

    let mut tx = self.pool.begin().await.context("failed to begin transaction")?;

    let existing: Option<(String, String, String, i64, i64)> = sqlx::query_as(
      "SELECT lease_id, holder_id, kind, expires_at_epoch_secs, version
       FROM leases
       WHERE resource_id = $1
       FOR UPDATE"
    )
    .bind(&request.resource_id)
    .fetch_optional(&mut *tx)
    .await
    .context("failed to query existing lease")?;

    if let Some((lease_id, holder_id, kind_str, expires_at, version)) = existing {
      if expires_at as u64 > now {
        if holder_id == request.holder_id {
          let new_expires = (now + ttl) as i64;
          let new_version = version + 1;

          sqlx::query(
            "UPDATE leases
             SET expires_at_epoch_secs = $1, version = $2, updated_at = NOW()
             WHERE lease_id = $3"
          )
          .bind(new_expires)
          .bind(new_version)
          .bind(&lease_id)
          .execute(&mut *tx)
          .await
          .context("failed to update lease")?;

          tx.commit().await.context("failed to commit transaction")?;

          let kind = kind_str.parse().unwrap_or(LeaseKind::Stream);
          return Ok(LeaseAcquireResponse {
            granted: true,
            record: Some(LeaseRecord {
              lease_id,
              resource_id: request.resource_id,
              holder_id,
              kind,
              expires_at_epoch_secs: new_expires as u64,
              version: new_version as u64,
            }),
          });
        } else {
          tx.rollback().await.ok();
          let kind = kind_str.parse().unwrap_or(LeaseKind::Stream);
          return Ok(LeaseAcquireResponse {
            granted: false,
            record: Some(LeaseRecord {
              lease_id,
              resource_id: request.resource_id,
              holder_id,
              kind,
              expires_at_epoch_secs: expires_at as u64,
              version: version as u64,
            }),
          });
        }
      } else {
        sqlx::query("DELETE FROM leases WHERE lease_id = $1")
          .bind(&lease_id)
          .execute(&mut *tx)
          .await
          .context("failed to delete expired lease")?;
      }
    }

    let lease_id = Uuid::new_v4().to_string();
    let expires_at = (now + ttl) as i64;
    let kind_str = request.kind.to_string();

    sqlx::query(
      "INSERT INTO leases (lease_id, resource_id, holder_id, kind, expires_at_epoch_secs, version)
       VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(&lease_id)
    .bind(&request.resource_id)
    .bind(&request.holder_id)
    .bind(&kind_str)
    .bind(expires_at)
    .bind(1i64)
    .execute(&mut *tx)
    .await
    .context("failed to insert new lease")?;

    tx.commit().await.context("failed to commit transaction")?;

    Ok(LeaseAcquireResponse {
      granted: true,
      record: Some(LeaseRecord {
        lease_id,
        resource_id: request.resource_id,
        holder_id: request.holder_id,
        kind: request.kind,
        expires_at_epoch_secs: expires_at as u64,
        version: 1,
      }),
    })
  }

  async fn renew(&self, request: LeaseRenewRequest) -> Result<LeaseRenewResponse> {
    let ttl = self.normalize_ttl(request.ttl_secs);
    let now = Self::now_epoch_secs();

    self.purge_expired().await?;

    let result: Option<(String, String, String, i64, i64)> = sqlx::query_as(
      "SELECT resource_id, holder_id, kind, expires_at_epoch_secs, version
       FROM leases
       WHERE lease_id = $1"
    )
    .bind(&request.lease_id)
    .fetch_optional(&self.pool)
    .await
    .context("failed to query lease")?;

    let Some((resource_id, holder_id, kind_str, expires_at, version)) = result else {
      return Ok(LeaseRenewResponse {
        renewed: false,
        record: None,
      });
    };

    if expires_at as u64 <= now {
      sqlx::query("DELETE FROM leases WHERE lease_id = $1")
        .bind(&request.lease_id)
        .execute(&self.pool)
        .await
        .ok();

      return Ok(LeaseRenewResponse {
        renewed: false,
        record: None,
      });
    }

    let new_expires = (now + ttl) as i64;
    let new_version = version + 1;

    sqlx::query(
      "UPDATE leases
       SET expires_at_epoch_secs = $1, version = $2, updated_at = NOW()
       WHERE lease_id = $3"
    )
    .bind(new_expires)
    .bind(new_version)
    .bind(&request.lease_id)
    .execute(&self.pool)
    .await
    .context("failed to renew lease")?;

    let kind = kind_str.parse().unwrap_or(LeaseKind::Stream);

    Ok(LeaseRenewResponse {
      renewed: true,
      record: Some(LeaseRecord {
        lease_id: request.lease_id,
        resource_id,
        holder_id,
        kind,
        expires_at_epoch_secs: new_expires as u64,
        version: new_version as u64,
      }),
    })
  }

  async fn release(&self, request: LeaseReleaseRequest) -> Result<LeaseReleaseResponse> {
    self.purge_expired().await?;

    let result = sqlx::query("DELETE FROM leases WHERE lease_id = $1")
      .bind(&request.lease_id)
      .execute(&self.pool)
      .await
      .context("failed to release lease")?;

    Ok(LeaseReleaseResponse {
      released: result.rows_affected() > 0,
    })
  }

  async fn list(&self, kind: Option<LeaseKind>) -> Result<Vec<LeaseRecord>> {
    self.purge_expired().await?;

    let records: Vec<(String, String, String, String, i64, i64)> = if let Some(kind) = kind {
      let kind_str = kind.to_string();
      sqlx::query_as(
        "SELECT lease_id, resource_id, holder_id, kind, expires_at_epoch_secs, version
         FROM leases
         WHERE kind = $1
         ORDER BY resource_id"
      )
      .bind(&kind_str)
      .fetch_all(&self.pool)
      .await
      .context("failed to list leases")?
    } else {
      sqlx::query_as(
        "SELECT lease_id, resource_id, holder_id, kind, expires_at_epoch_secs, version
         FROM leases
         ORDER BY resource_id"
      )
      .fetch_all(&self.pool)
      .await
      .context("failed to list leases")?
    };

    let mut out = Vec::new();
    for (lease_id, resource_id, holder_id, kind_str, expires_at, version) in records {
      let kind = kind_str.parse().unwrap_or(LeaseKind::Stream);
      out.push(LeaseRecord {
        lease_id,
        resource_id,
        holder_id,
        kind,
        expires_at_epoch_secs: expires_at as u64,
        version: version as u64,
      });
    }

    Ok(out)
  }

  async fn health_check(&self) -> Result<bool> {
    // Verify database connectivity with a simple query
    match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
      Ok(_) => Ok(true),
      Err(e) => {
        tracing::warn!(error = %e, "PostgreSQL health check failed");
        Ok(false)
      }
    }
  }
}
