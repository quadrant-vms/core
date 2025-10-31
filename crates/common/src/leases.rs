use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr, time::{Duration, SystemTime}};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LeaseKind {
    Stream,
    Recorder,
    Pipeline,
    Ai,
}

impl LeaseKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            LeaseKind::Stream => "stream",
            LeaseKind::Recorder => "recorder",
            LeaseKind::Pipeline => "pipeline",
            LeaseKind::Ai => "ai",
        }
    }
}

impl fmt::Display for LeaseKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LeaseKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.to_ascii_lowercase();
        match normalized.as_str() {
            "stream" => Ok(LeaseKind::Stream),
            "recorder" => Ok(LeaseKind::Recorder),
            "pipeline" => Ok(LeaseKind::Pipeline),
            "ai" => Ok(LeaseKind::Ai),
            _ => Err(format!("unknown lease kind '{s}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseAcquireRequest {
    pub resource_id: String,
    pub holder_id: String,
    pub kind: LeaseKind,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

fn default_ttl_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRenewRequest {
    pub lease_id: String,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseReleaseRequest {
    pub lease_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRecord {
    pub lease_id: String,
    pub resource_id: String,
    pub holder_id: String,
    pub kind: LeaseKind,
    pub expires_at_epoch_secs: u64,
    pub version: u64,
}

impl LeaseRecord {
    pub fn ttl(&self) -> Duration {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0));
        let expires = Duration::from_secs(self.expires_at_epoch_secs);
        if expires > now {
            expires - now
        } else {
            Duration::from_secs(0)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseAcquireResponse {
    pub granted: bool,
    pub record: Option<LeaseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRenewResponse {
    pub renewed: bool,
    pub record: Option<LeaseRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseReleaseResponse {
    pub released: bool,
}
