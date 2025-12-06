use coordinator::{
  cluster::ClusterManager,
  config::{CoordinatorConfig, LeaseStoreType},
  routes,
  state::CoordinatorState,
  store::MemoryLeaseStore,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{net::TcpListener, time::sleep};

async fn start_coordinator(
  node_id: String,
  bind_addr: SocketAddr,
  peer_addrs: Vec<String>,
) -> (tokio::task::JoinHandle<()>, String) {
  let config = CoordinatorConfig {
    bind_addr,
    default_ttl_secs: 10,
    max_ttl_secs: 60,
    store_type: LeaseStoreType::Memory,
    database_url: None,
    cluster_enabled: true,
    node_id: Some(node_id.clone()),
    peer_addrs: peer_addrs.clone(),
    election_timeout_ms: 2000,
    heartbeat_interval_ms: 500,
  };

  let store = Arc::new(MemoryLeaseStore::new(10, 60));
  let cluster = Arc::new(ClusterManager::new(
    node_id,
    bind_addr.to_string(),
    peer_addrs,
    2000,
    500,
  ));

  let election_monitor = cluster.clone();
  tokio::spawn(async move {
    election_monitor.start_election_monitor().await;
  });

  let heartbeat_sender = cluster.clone();
  tokio::spawn(async move {
    heartbeat_sender.start_heartbeat_sender().await;
  });

  let state = CoordinatorState::with_cluster(config, store, cluster);
  let app = routes::router(state);
  let listener = TcpListener::bind(bind_addr).await.unwrap();
  let url = format!("http://{}", bind_addr);

  let handle = tokio::spawn(async move {
    axum::serve(listener, app.into_make_service())
      .await
      .unwrap();
  });

  (handle, url)
}

#[tokio::test]
async fn test_single_node_cluster_elects_leader() {
  let addr: SocketAddr = "127.0.0.1:18082".parse().unwrap();
  let (_handle, url) = start_coordinator("node-1".to_string(), addr, vec![]).await;

  sleep(Duration::from_secs(3)).await;

  let client = reqwest::Client::new();
  let resp = client
    .get(format!("{}/cluster/status", url))
    .send()
    .await
    .unwrap();

  assert_eq!(resp.status(), 200);
  let status: serde_json::Value = resp.json().await.unwrap();
  assert_eq!(status["role"], "Leader");
  assert_eq!(status["node_id"], "node-1");
}

#[tokio::test]
async fn test_three_node_cluster_elects_leader() {
  let addr1: SocketAddr = "127.0.0.1:18083".parse().unwrap();
  let addr2: SocketAddr = "127.0.0.1:18084".parse().unwrap();
  let addr3: SocketAddr = "127.0.0.1:18085".parse().unwrap();

  let peers_for_1 = vec![addr2.to_string(), addr3.to_string()];
  let peers_for_2 = vec![addr1.to_string(), addr3.to_string()];
  let peers_for_3 = vec![addr1.to_string(), addr2.to_string()];

  let (_h1, url1) =
    start_coordinator("node-1".to_string(), addr1, peers_for_1).await;
  let (_h2, url2) =
    start_coordinator("node-2".to_string(), addr2, peers_for_2).await;
  let (_h3, url3) =
    start_coordinator("node-3".to_string(), addr3, peers_for_3).await;

  sleep(Duration::from_secs(4)).await;

  let client = reqwest::Client::new();

  let status1: serde_json::Value = client
    .get(format!("{}/cluster/status", url1))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

  let status2: serde_json::Value = client
    .get(format!("{}/cluster/status", url2))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

  let status3: serde_json::Value = client
    .get(format!("{}/cluster/status", url3))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

  let mut leader_count = 0;
  let mut follower_count = 0;

  for status in [&status1, &status2, &status3] {
    match status["role"].as_str().unwrap() {
      "Leader" => leader_count += 1,
      "Follower" => follower_count += 1,
      _ => {}
    }
  }

  assert_eq!(leader_count, 1, "exactly one node should be leader");
  assert_eq!(follower_count, 2, "two nodes should be followers");

  let leader_id = if status1["role"] == "Leader" {
    status1["leader_id"].as_str().unwrap()
  } else if status2["role"] == "Leader" {
    status2["leader_id"].as_str().unwrap()
  } else {
    status3["leader_id"].as_str().unwrap()
  };

  assert_eq!(
    status1["leader_id"].as_str().unwrap(),
    leader_id,
    "all nodes should agree on leader"
  );
  assert_eq!(
    status2["leader_id"].as_str().unwrap(),
    leader_id,
    "all nodes should agree on leader"
  );
  assert_eq!(
    status3["leader_id"].as_str().unwrap(),
    leader_id,
    "all nodes should agree on leader"
  );
}
