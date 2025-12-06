use crate::{cluster::ClusterManager, config::CoordinatorConfig, store::LeaseStore};
use std::sync::Arc;

#[derive(Clone)]
pub struct CoordinatorState {
  inner: Arc<StateInner>,
}

struct StateInner {
  config: CoordinatorConfig,
  store: Arc<dyn LeaseStore>,
  cluster: Option<Arc<ClusterManager>>,
}

impl CoordinatorState {
  pub fn new(config: CoordinatorConfig, store: Arc<dyn LeaseStore>) -> Self {
    Self {
      inner: Arc::new(StateInner {
        config,
        store,
        cluster: None,
      }),
    }
  }

  pub fn with_cluster(
    config: CoordinatorConfig,
    store: Arc<dyn LeaseStore>,
    cluster: Arc<ClusterManager>,
  ) -> Self {
    Self {
      inner: Arc::new(StateInner {
        config,
        store,
        cluster: Some(cluster),
      }),
    }
  }

  pub fn config(&self) -> &CoordinatorConfig {
    &self.inner.config
  }

  pub fn store(&self) -> Arc<dyn LeaseStore> {
    self.inner.store.clone()
  }

  pub fn cluster(&self) -> Option<Arc<ClusterManager>> {
    self.inner.cluster.clone()
  }
}
