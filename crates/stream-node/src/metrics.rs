use once_cell::sync::Lazy;
use prometheus::{Encoder, IntCounter, IntGauge, Registry, TextEncoder};

pub static REGISTRY: Lazy<Registry> = Lazy::new(Registry::new);

pub static STREAMS_RUNNING: Lazy<IntGauge> = Lazy::new(|| {
  let g = IntGauge::new("streams_running", "Number of running streams").unwrap();
  REGISTRY.register(Box::new(g.clone())).ok();
  g
});

pub static RESTARTS_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
  let c = IntCounter::new("stream_restarts_total", "Total restarts of pipelines").unwrap();
  REGISTRY.register(Box::new(c.clone())).ok();
  c
});

pub fn render() -> String {
  let mut buf = Vec::new();
  let encoder = TextEncoder::new();
  let mfs = REGISTRY.gather();
  encoder.encode(&mfs, &mut buf).ok();
  String::from_utf8(buf).unwrap_or_default()
}
