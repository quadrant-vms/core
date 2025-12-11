use crate::discovery::OnvifDiscoveryClient;
use crate::firmware_executor::FirmwareExecutor;
use crate::firmware_storage::FirmwareStorage;
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
    pub firmware_executor: Arc<FirmwareExecutor>,
    pub firmware_storage: Arc<FirmwareStorage>,
}

impl DeviceManagerState {
    pub fn new(
        store: Arc<DeviceStore>,
        prober: Arc<DeviceProber>,
        tour_executor: Arc<TourExecutor>,
        discovery_client: Arc<OnvifDiscoveryClient>,
        firmware_executor: Arc<FirmwareExecutor>,
        firmware_storage: Arc<FirmwareStorage>,
    ) -> Self {
        Self {
            store,
            prober,
            tour_executor,
            discovery_client,
            firmware_executor,
            firmware_storage,
        }
    }
}
