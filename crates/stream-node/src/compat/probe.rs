use std::process::Command;
use tracing::{debug, warn};

#[derive(Debug, Clone)]
#[allow(dead_code)]
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

/// Probe RTSP stream to detect codecs and vendor information using ffprobe
///
/// Uses ffprobe to inspect the RTSP stream and extract:
/// - Video codec information (H.264/H.265)
/// - Vendor hints from user-agent or metadata
pub async fn probe(uri: &str) -> anyhow::Result<ProbeResult> {
  debug!(uri = %uri, "probing RTSP stream");

  // Use ffprobe to inspect the RTSP stream
  let output = Command::new("ffprobe")
    .args(&[
      "-v",
      "error",
      "-select_streams",
      "v:0",
      "-show_entries",
      "stream=codec_name",
      "-of",
      "json",
      "-rtsp_transport",
      "tcp", // Use TCP for more reliable probing
      "-timeout",
      "5000000", // 5 second timeout (in microseconds)
      uri,
    ])
    .output();

  let output = match output {
    Ok(output) => output,
    Err(e) => {
      warn!(uri = %uri, error = %e, "failed to execute ffprobe, using defaults");
      return Ok(ProbeResult::default());
    }
  };

  if !output.status.success() {
    warn!(
      uri = %uri,
      status = ?output.status,
      "ffprobe failed to probe stream, using defaults"
    );
    return Ok(ProbeResult::default());
  }

  // Parse ffprobe JSON output
  let json: serde_json::Value = match serde_json::from_slice(&output.stdout) {
    Ok(json) => json,
    Err(e) => {
      warn!(uri = %uri, error = %e, "failed to parse ffprobe output, using defaults");
      return Ok(ProbeResult::default());
    }
  };

  let mut result = ProbeResult {
    vendor_hint: None,
    has_h264: false,
    has_h265: false,
  };

  // Extract codec information from streams
  if let Some(streams) = json.get("streams").and_then(|s| s.as_array()) {
    for stream in streams {
      if let Some(codec_name) = stream.get("codec_name").and_then(|c| c.as_str()) {
        match codec_name {
          "h264" => {
            result.has_h264 = true;
            debug!(uri = %uri, "detected H.264 codec");
          }
          "hevc" | "h265" => {
            result.has_h265 = true;
            debug!(uri = %uri, "detected H.265/HEVC codec");
          }
          _ => {
            debug!(uri = %uri, codec = %codec_name, "detected other codec");
          }
        }
      }
    }
  }

  // Try to infer vendor from URI patterns
  result.vendor_hint = infer_vendor_from_uri(uri);

  debug!(uri = %uri, result = ?result, "probe completed");

  Ok(result)
}

/// Infer camera vendor from URI patterns
fn infer_vendor_from_uri(uri: &str) -> Option<String> {
  let uri_lower = uri.to_lowercase();

  // Common vendor-specific URI patterns
  if uri_lower.contains("axis-media") || uri_lower.contains("/axis-cgi/") {
    return Some("Axis".to_string());
  }
  if uri_lower.contains("hikvision") || uri_lower.contains("/onvif") {
    return Some("Hikvision".to_string());
  }
  if uri_lower.contains("dahua") {
    return Some("Dahua".to_string());
  }
  if uri_lower.contains("hanwha") || uri_lower.contains("wisenet") {
    return Some("Hanwha".to_string());
  }
  if uri_lower.contains("bosch") {
    return Some("Bosch".to_string());
  }
  if uri_lower.contains("vivotek") {
    return Some("Vivotek".to_string());
  }

  None
}
