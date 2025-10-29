use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct CameraProfile {
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub firmware: Option<String>,
    pub auth: Option<AuthKind>,
    pub rtsp_params: Option<Vec<(String, String)>>,
    /// Ordered preset names to try as fallbacks.
    pub presets: Vec<String>,

    /// Filled by loader: the source file path (for debugging).
    #[serde(skip)]
    pub source_file: Option<String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum AuthKind {
    Basic,
    Digest,
}

impl Default for CameraProfile {
    fn default() -> Self {
        Self {
            vendor: None,
            model: None,
            firmware: None,
            auth: None,
            rtsp_params: None,
            presets: vec![
                "h264_ts_lowlat".into(),
                "h264_ts_default".into(),
                "h265_fmp4".into(),
            ],
            source_file: None,
        }
    }
}
