use crate::stream::{Codec, Container};

/// A reusable named pipeline recipe (will be tried in order for fallback).
#[derive(Debug, Clone)]
pub struct PipelinePreset {
    pub name: String,
    pub codec: Codec,          // H264 | H265
    pub container: Container,  // Ts  | Fmp4
    pub latency_ms: u32,       // rtspsrc latency
    pub parse_opts: Vec<String>, // e.g. ["config-interval=-1"]
}

pub fn builtin_presets() -> Vec<PipelinePreset> {
    vec![
        PipelinePreset {
            name: "h264_ts_lowlat".into(),
            codec: Codec::H264,
            container: Container::Ts,
            latency_ms: 0,
            parse_opts: vec!["config-interval=-1".into()],
        },
        PipelinePreset {
            name: "h264_ts_default".into(),
            codec: Codec::H264,
            container: Container::Ts,
            latency_ms: 200,
            parse_opts: vec!["config-interval=-1".into()],
        },
        PipelinePreset {
            name: "h265_fmp4".into(),
            codec: Codec::H265,
            container: Container::Fmp4,
            latency_ms: 200,
            parse_opts: vec!["config-interval=-1".into()],
        },
    ]
}

/// Lookup a builtin preset by name.
pub fn get_preset(name: &str) -> Option<PipelinePreset> {
    builtin_presets()
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name))
}