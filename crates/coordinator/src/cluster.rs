use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
  sync::RwLock,
  time::interval,
};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRole {
  Leader,
  Follower,
  Candidate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
  pub id: String,
  pub addr: String,
  pub last_heartbeat: u64,
  pub is_healthy: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterStatus {
  pub node_id: String,
  pub role: NodeRole,
  pub leader_id: Option<String>,
  pub peers: Vec<PeerInfo>,
  pub term: u64,
}

struct ClusterInner {
  role: NodeRole,
  leader_id: Option<String>,
  peers: HashMap<String, PeerInfo>,
  term: u64,
  last_heartbeat: u64,
  votes_received: usize,
}

pub struct ClusterManager {
  node_id: String,
  node_addr: String,
  peer_addrs: Vec<String>,
  inner: Arc<RwLock<ClusterInner>>,
  http_client: reqwest::Client,
  election_timeout_ms: u64,
  heartbeat_interval_ms: u64,
}

impl ClusterManager {
  pub fn new(
    node_id: String,
    node_addr: String,
    peer_addrs: Vec<String>,
    election_timeout_ms: u64,
    heartbeat_interval_ms: u64,
  ) -> Self {
    let peers: HashMap<String, PeerInfo> = peer_addrs
      .iter()
      .enumerate()
      .map(|(i, addr)| {
        let peer_id = format!("peer-{}", i);
        (
          peer_id.clone(),
          PeerInfo {
            id: peer_id,
            addr: addr.clone(),
            last_heartbeat: Self::now_epoch_secs(),
            is_healthy: true,
          },
        )
      })
      .collect();

    let inner = ClusterInner {
      role: NodeRole::Follower,
      leader_id: None,
      peers,
      term: 0,
      last_heartbeat: Self::now_epoch_secs(),
      votes_received: 0,
    };

    Self {
      node_id,
      node_addr,
      peer_addrs,
      inner: Arc::new(RwLock::new(inner)),
      http_client: reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build HTTP client"),
      election_timeout_ms,
      heartbeat_interval_ms,
    }
  }

  fn now_epoch_secs() -> u64 {
    SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
  }

  pub async fn status(&self) -> ClusterStatus {
    let inner = self.inner.read().await;
    ClusterStatus {
      node_id: self.node_id.clone(),
      role: inner.role.clone(),
      leader_id: inner.leader_id.clone(),
      peers: inner.peers.values().cloned().collect(),
      term: inner.term,
    }
  }

  pub async fn is_leader(&self) -> bool {
    let inner = self.inner.read().await;
    inner.role == NodeRole::Leader
  }

  pub async fn leader_addr(&self) -> Option<String> {
    let inner = self.inner.read().await;
    if let Some(leader_id) = &inner.leader_id {
      if leader_id == &self.node_id {
        Some(self.node_addr.clone())
      } else {
        inner
          .peers
          .get(leader_id)
          .map(|peer| peer.addr.clone())
      }
    } else {
      None
    }
  }

  pub async fn start_election_monitor(self: Arc<Self>) {
    // Add randomized delay to prevent split votes
    use rand::Rng;
    let jitter = rand::thread_rng().gen_range(0..500);
    tokio::time::sleep(Duration::from_millis(100 + jitter)).await;

    if let Err(e) = self.start_election().await {
      warn!(error = %e, "initial election failed");
    }

    let mut ticker = interval(Duration::from_millis(self.election_timeout_ms));

    loop {
      ticker.tick().await;

      let should_start_election = {
        let inner = self.inner.read().await;
        let now = Self::now_epoch_secs();
        let elapsed = now.saturating_sub(inner.last_heartbeat);

        matches!(inner.role, NodeRole::Follower | NodeRole::Candidate)
          && elapsed > (self.election_timeout_ms / 1000)
      };

      if should_start_election {
        info!(node_id = %self.node_id, "starting leader election");
        if let Err(e) = self.start_election().await {
          warn!(error = %e, "election failed");
        }
      }
    }
  }

  async fn start_election(&self) -> Result<()> {
    {
      let mut inner = self.inner.write().await;
      inner.role = NodeRole::Candidate;
      inner.term += 1;
      inner.votes_received = 1; // vote for self
      inner.leader_id = None;
      debug!(
        node_id = %self.node_id,
        term = inner.term,
        "became candidate"
      );
    }

    let current_term = self.inner.read().await.term;
    let mut votes = 1;

    for peer in self.peer_addrs.iter() {
      match self
        .request_vote(peer, current_term)
        .await
      {
        Ok(granted) => {
          if granted {
            votes += 1;
          }
        }
        Err(e) => {
          debug!(peer = %peer, error = %e, "vote request failed");
        }
      }
    }

    let majority = (self.peer_addrs.len() + 1) / 2 + 1;

    if votes >= majority {
      let mut inner = self.inner.write().await;
      inner.role = NodeRole::Leader;
      inner.leader_id = Some(self.node_id.clone());
      inner.last_heartbeat = Self::now_epoch_secs();
      info!(
        node_id = %self.node_id,
        term = current_term,
        votes = votes,
        "became leader"
      );
    } else {
      let mut inner = self.inner.write().await;
      inner.role = NodeRole::Follower;
      debug!(
        node_id = %self.node_id,
        votes = votes,
        majority = majority,
        "election failed, reverting to follower"
      );
    }

    Ok(())
  }

  async fn request_vote(&self, peer_addr: &str, term: u64) -> Result<bool> {
    #[derive(Serialize)]
    struct VoteRequest {
      candidate_id: String,
      term: u64,
    }

    #[derive(Deserialize)]
    struct VoteResponse {
      vote_granted: bool,
    }

    let url = format!("http://{}/cluster/vote", peer_addr);
    let req = VoteRequest {
      candidate_id: self.node_id.clone(),
      term,
    };

    let response = self
      .http_client
      .post(&url)
      .json(&req)
      .send()
      .await
      .context("failed to send vote request")?;

    let vote_resp: VoteResponse = response
      .json()
      .await
      .context("failed to parse vote response")?;

    Ok(vote_resp.vote_granted)
  }

  pub async fn handle_vote_request(&self, candidate_id: String, term: u64) -> bool {
    let mut inner = self.inner.write().await;

    if term > inner.term {
      inner.term = term;
      inner.role = NodeRole::Follower;
      inner.leader_id = None;
      info!(
        node_id = %self.node_id,
        candidate = %candidate_id,
        term = term,
        "granting vote"
      );
      true
    } else {
      debug!(
        node_id = %self.node_id,
        candidate = %candidate_id,
        term = term,
        current_term = inner.term,
        "rejecting vote"
      );
      false
    }
  }

  pub async fn start_heartbeat_sender(self: Arc<Self>) {
    let mut ticker = interval(Duration::from_millis(self.heartbeat_interval_ms));

    loop {
      ticker.tick().await;

      let is_leader = self.is_leader().await;
      if !is_leader {
        continue;
      }

      for peer_addr in self.peer_addrs.iter() {
        let peer_addr = peer_addr.clone();
        let self_clone = self.clone();
        tokio::spawn(async move {
          if let Err(e) = self_clone.send_heartbeat(&peer_addr).await {
            debug!(peer = %peer_addr, error = %e, "heartbeat failed");
          }
        });
      }
    }
  }

  async fn send_heartbeat(&self, peer_addr: &str) -> Result<()> {
    #[derive(Serialize)]
    struct HeartbeatRequest {
      leader_id: String,
      term: u64,
    }

    let term = self.inner.read().await.term;
    let url = format!("http://{}/cluster/heartbeat", peer_addr);
    let req = HeartbeatRequest {
      leader_id: self.node_id.clone(),
      term,
    };

    self
      .http_client
      .post(&url)
      .json(&req)
      .timeout(Duration::from_secs(2))
      .send()
      .await
      .context("failed to send heartbeat")?;

    Ok(())
  }

  pub async fn handle_heartbeat(&self, leader_id: String, term: u64) {
    let mut inner = self.inner.write().await;

    if term >= inner.term {
      inner.term = term;
      inner.role = NodeRole::Follower;
      inner.leader_id = Some(leader_id.clone());
      inner.last_heartbeat = Self::now_epoch_secs();
      debug!(
        node_id = %self.node_id,
        leader = %leader_id,
        term = term,
        "received heartbeat"
      );
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn cluster_manager_initialization() {
    let cm = ClusterManager::new(
      "node-1".to_string(),
      "127.0.0.1:8082".to_string(),
      vec!["127.0.0.1:8083".to_string(), "127.0.0.1:8084".to_string()],
      5000,
      1000,
    );

    let status = cm.status().await;
    assert_eq!(status.node_id, "node-1");
    assert_eq!(status.role, NodeRole::Follower);
    assert_eq!(status.peers.len(), 2);
  }

  #[tokio::test]
  async fn single_node_becomes_leader() {
    let cm = Arc::new(ClusterManager::new(
      "node-1".to_string(),
      "127.0.0.1:8082".to_string(),
      vec![],
      5000,
      1000,
    ));

    cm.start_election().await.unwrap();

    let status = cm.status().await;
    assert_eq!(status.role, NodeRole::Leader);
    assert_eq!(status.leader_id, Some("node-1".to_string()));
  }

  #[tokio::test]
  async fn follower_grants_vote_to_higher_term() {
    let cm = ClusterManager::new(
      "node-1".to_string(),
      "127.0.0.1:8082".to_string(),
      vec![],
      5000,
      1000,
    );

    let granted = cm.handle_vote_request("node-2".to_string(), 5).await;
    assert!(granted);

    let status = cm.status().await;
    assert_eq!(status.term, 5);
    assert_eq!(status.role, NodeRole::Follower);
  }

  #[tokio::test]
  async fn follower_updates_leader_on_heartbeat() {
    let cm = ClusterManager::new(
      "node-1".to_string(),
      "127.0.0.1:8082".to_string(),
      vec![],
      5000,
      1000,
    );

    cm.handle_heartbeat("node-2".to_string(), 3).await;

    let status = cm.status().await;
    assert_eq!(status.role, NodeRole::Follower);
    assert_eq!(status.leader_id, Some("node-2".to_string()));
    assert_eq!(status.term, 3);
  }
}
