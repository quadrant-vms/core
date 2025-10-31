use crate::{config::CoordinatorConfig, store::LeaseStore};
use std::sync::Arc;

#[derive(Clone)]
pub struct CoordinatorState {
    inner: Arc<StateInner>,
}

struct StateInner {
    config: CoordinatorConfig,
    store: Arc<dyn LeaseStore>,
}

impl CoordinatorState {
    pub fn new(config: CoordinatorConfig, store: Arc<dyn LeaseStore>) -> Self {
        Self {
            inner: Arc::new(StateInner { config, store }),
        }
    }

    pub fn config(&self) -> &CoordinatorConfig {
        &self.inner.config
    }

    pub fn store(&self) -> Arc<dyn LeaseStore> {
        self.inner.store.clone()
    }
}
