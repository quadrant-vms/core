#[derive(Debug, Clone)]
pub struct ProbeResult {
  pub vendor_hint: Option<String>,
  pub has_h264: bool,
  pub has_h265: bool,
}

impl Default for ProbeResult {
  fn default() -> Self {
    Self {
      vendor_hint: None,
      has_h264: true,
      has_h265: false,
    }
  }
}

/// TODO: Implement real RTSP/SDP probing. For now, return a safe default.
pub async fn probe(_uri: &str) -> anyhow::Result<ProbeResult> {
  // Future: OPTIONS/DESCRIBE → parse SDP, infer codecs & vendor.
  Ok(ProbeResult::default())
}
