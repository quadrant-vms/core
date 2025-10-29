use super::{PipelinePreset, ProbeResult};
use std::collections::HashMap;

/// Vendor-specific tuning hook. Allows changing the chosen preset or its fields.
pub trait CameraAdapter: Send + Sync + 'static {
    /// Given a tentative preset and probe/profile info, return an adjusted preset.
    fn adjust(&self, preset: PipelinePreset, probe: &ProbeResult) -> PipelinePreset;
}

/// Default adapter: no-op.
pub struct DefaultAdapter;
impl CameraAdapter for DefaultAdapter {
    fn adjust(&self, preset: PipelinePreset, _probe: &ProbeResult) -> PipelinePreset {
        preset
    }
}

/// Axis example: often okay with low latency; keep as no-op for now.
pub struct AxisAdapter;
impl CameraAdapter for AxisAdapter {
    fn adjust(&self, mut preset: PipelinePreset, _probe: &ProbeResult) -> PipelinePreset {
        // Example tweak: if low-lat preset, cap at 100ms instead of 0
        if preset.name.eq_ignore_ascii_case("h264_ts_lowlat") && preset.latency_ms == 0 {
            preset.latency_ms = 100;
        }
        preset
    }
}

/// Hikvision example stub (H.265 sometimes better with fMP4).
pub struct HikvisionAdapter;
impl CameraAdapter for HikvisionAdapter {
    fn adjust(&self, preset: PipelinePreset, _probe: &ProbeResult) -> PipelinePreset {
        // Future: prefer fMP4 for H.265, tweak parse options, etc.
        preset
    }
}

/// Adapter registry mapping lowercase vendor name → adapter
pub fn adapter_registry() -> HashMap<&'static str, Box<dyn CameraAdapter>> {
    let mut map: HashMap<&'static str, Box<dyn CameraAdapter>> = HashMap::new();
    map.insert("axis", Box::new(AxisAdapter));
    map.insert("hikvision", Box::new(HikvisionAdapter));
    map
}

/// Get an adapter by vendor name; falls back to DefaultAdapter.
pub fn find_adapter(vendor_hint: Option<&str>) -> Box<dyn CameraAdapter> {
    if let Some(vendor) = vendor_hint {
        let key = vendor.to_ascii_lowercase();
        if let Some(a) = adapter_registry().remove(key.as_str()) {
            return a;
        }
    }
    Box::new(DefaultAdapter)
}