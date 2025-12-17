use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct StartRequest {
  pub id: String,
  pub uri: String,
  #[serde(default = "default_codec")]
  pub codec: String, // "h264" | "h265" | "hevc" | "h265+"
  #[serde(default = "default_container")]
  pub container: String, // "ts" | "fmp4"
}
pub fn default_codec() -> String {
  "h264".into()
}
pub fn default_container() -> String {
  "ts".into()
}

#[derive(Deserialize)]
pub struct StopRequest {
  pub id: String,
}

// Legacy query support (deprecated)
#[derive(Deserialize)]
pub struct StartQuery {
  pub id: String,
  pub uri: String,
  #[serde(default = "default_codec")]
  pub codec: String,
  #[serde(default = "default_container")]
  pub container: String,
}

#[derive(Deserialize)]
pub struct StopQuery {
  pub id: String,
}

#[derive(Serialize)]
pub struct StreamDto {
  pub id: String,
  pub uri: String,
  pub codec: String,
  pub container: String,
  pub running: bool,
  pub playlist: String,
  pub output_dir: String,
}
