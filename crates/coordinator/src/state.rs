use crate::{cluster::ClusterManager, config::CoordinatorConfig, store::LeaseStore};
use common::state_store::StateStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoordinatorState {
  inner: Arc<StateInner>,
}

struct StateInner {
  config: CoordinatorConfig,
  store: Arc<dyn LeaseStore>,
  state_store: Option<Arc<dyn StateStore>>,
  cluster: Option<Arc<ClusterManager>>,
}

impl CoordinatorState {
  pub fn new(config: CoordinatorConfig, store: Arc<dyn LeaseStore>, state_store: Option<Arc<dyn StateStore>>) -> Self {
    Self {
      inner: Arc::new(StateInner {
        config,
        store,
        state_store,
        cluster: None,
      }),
    }
  }

  pub fn with_cluster(
    config: CoordinatorConfig,
    store: Arc<dyn LeaseStore>,
    state_store: Option<Arc<dyn StateStore>>,
    cluster: Arc<ClusterManager>,
  ) -> Self {
    Self {
      inner: Arc::new(StateInner {
        config,
        store,
        state_store,
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

  pub fn state_store(&self) -> Option<Arc<dyn StateStore>> {
    self.inner.state_store.clone()
  }

  pub fn cluster(&self) -> Option<Arc<ClusterManager>> {
    self.inner.cluster.clone()
  }
}
