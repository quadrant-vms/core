use anyhow::Result;
use async_trait::async_trait;
use common::leases::{
  LeaseAcquireRequest, LeaseAcquireResponse, LeaseKind, LeaseRecord, LeaseReleaseRequest,
  LeaseReleaseResponse, LeaseRenewRequest, LeaseRenewResponse,
};
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
