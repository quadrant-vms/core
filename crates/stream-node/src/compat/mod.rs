pub mod adapter;
pub mod preset;
pub mod probe;
pub mod profile;

pub use preset::*;
pub use probe::*;

use std::path::{Path, PathBuf};

/// Resolve the profiles directory from env or default (`./profiles/cameras`)
pub fn profiles_dir() -> PathBuf {
  std::env::var("CAMERA_PROFILES_DIR")
    .map(PathBuf::from)
    .unwrap_or_else(|_| PathBuf::from("./profiles/cameras"))
}

/// Load all CameraProfile from the given directory (non-recursive).
/// Silently skips files that fail to parse, but returns those that succeed.
pub fn load_profiles_from_dir(dir: &Path) -> Vec<profile::CameraProfile> {
  let mut out = Vec::new();
  if let Ok(read) = std::fs::read_dir(dir) {
    for entry in read.flatten() {
      let path = entry.path();
      if !path.is_file() {
        continue;
      }
      let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
      if ext != "yaml" && ext != "yml" {
        continue;
      }

      match std::fs::read_to_string(&path) {
        Ok(s) => {
          match serde_yaml::from_str::<profile::CameraProfile>(&s) {
            Ok(mut p) => {
              // attach source file hint for debugging
              p.source_file = Some(path.to_string_lossy().to_string());
              out.push(p);
            }
            Err(e) => {
              tracing::warn!("skip invalid profile {:?}: {}", path, e);
            }
          }
        }
        Err(e) => {
          tracing::warn!("read profile {:?} failed: {}", path, e);
        }
      }
    }
  }
  out
}
