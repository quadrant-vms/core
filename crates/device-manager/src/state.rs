use crate::discovery::OnvifDiscoveryClient;
use crate::prober::DeviceProber;
use crate::store::DeviceStore;
use crate::tour_executor::TourExecutor;
use std::sync::Arc;

#[derive(Clone)]
pub struct DeviceManagerState {
    pub store: Arc<DeviceStore>,
    pub prober: Arc<DeviceProber>,
    pub tour_executor: Arc<TourExecutor>,
    pub discovery_client: Arc<OnvifDiscoveryClient>,
}

impl DeviceManagerState {
    pub fn new(
        store: Arc<DeviceStore>,
        prober: Arc<DeviceProber>,
        tour_executor: Arc<TourExecutor>,
        discovery_client: Arc<OnvifDiscoveryClient>,
    ) -> Self {
        Self {
            store,
            prober,
            tour_executor,
            discovery_client,
        }
    }
}
