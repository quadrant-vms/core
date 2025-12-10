use crate::prober::DeviceProber;
use crate::store::DeviceStore;
use std::sync::Arc;

#[derive(Clone)]
pub struct DeviceManagerState {
    pub store: Arc<DeviceStore>,
    pub prober: Arc<DeviceProber>,
}

impl DeviceManagerState {
    pub fn new(store: Arc<DeviceStore>, prober: Arc<DeviceProber>) -> Self {
        Self { store, prober }
    }
}
