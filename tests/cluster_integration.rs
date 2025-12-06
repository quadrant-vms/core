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

#[tokio::test]
async fn test_follower_forwards_lease_request_to_leader() {
  let addr1: SocketAddr = "127.0.0.1:18086".parse().unwrap();
  let addr2: SocketAddr = "127.0.0.1:18087".parse().unwrap();

  let peers_for_1 = vec![addr2.to_string()];
  let peers_for_2 = vec![addr1.to_string()];

  let (_h1, url1) =
    start_coordinator("node-1".to_string(), addr1, peers_for_1).await;
  let (_h2, url2) =
    start_coordinator("node-2".to_string(), addr2, peers_for_2).await;

  // Wait for leader election
  sleep(Duration::from_secs(3)).await;

  let client = reqwest::Client::new();

  // Find leader and follower
  let status1: serde_json::Value = client
    .get(format!("{}/cluster/status", url1))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

  let _status2: serde_json::Value = client
    .get(format!("{}/cluster/status", url2))
    .send()
    .await
    .unwrap()
    .json()
    .await
    .unwrap();

  let (leader_url, follower_url) = if status1["role"] == "Leader" {
    (&url1, &url2)
  } else {
    (&url2, &url1)
  };

  // Send lease acquisition request to the follower
  let acquire_request = serde_json::json!({
    "resource_id": "test-resource",
    "holder_id": "test-holder",
    "kind": "stream",
    "ttl_secs": 30
  });

  let resp = client
    .post(format!("{}/v1/leases/acquire", follower_url))
    .json(&acquire_request)
    .send()
    .await
    .unwrap();

  assert_eq!(resp.status(), 200, "follower should forward and return success");

  let acquire_response: serde_json::Value = resp.json().await.unwrap();
  assert!(acquire_response["record"].is_object());
  assert_eq!(acquire_response["record"]["resource_id"], "test-resource");

  // Verify the lease exists on the leader
  let list_resp = client
    .get(format!("{}/v1/leases", leader_url))
    .send()
    .await
    .unwrap();

  assert_eq!(list_resp.status(), 200);
  let leases: Vec<serde_json::Value> = list_resp.json().await.unwrap();
  assert_eq!(leases.len(), 1);
  assert_eq!(leases[0]["resource_id"], "test-resource");

  // Test lease renewal through follower
  let lease_id = acquire_response["record"]["lease_id"].as_str().unwrap();
  let renew_request = serde_json::json!({
    "lease_id": lease_id,
    "ttl_secs": 30
  });

  let renew_resp = client
    .post(format!("{}/v1/leases/renew", follower_url))
    .json(&renew_request)
    .send()
    .await
    .unwrap();

  assert_eq!(renew_resp.status(), 200, "follower should forward renewal");

  // Test lease release through follower
  let release_request = serde_json::json!({
    "lease_id": lease_id
  });

  let release_resp = client
    .post(format!("{}/v1/leases/release", follower_url))
    .json(&release_request)
    .send()
    .await
    .unwrap();

  assert_eq!(release_resp.status(), 200, "follower should forward release");

  let release_response: serde_json::Value = release_resp.json().await.unwrap();
  assert_eq!(release_response["released"], true);

  // Verify lease is gone on leader
  let list_resp = client
    .get(format!("{}/v1/leases", leader_url))
    .send()
    .await
    .unwrap();

  let leases: Vec<serde_json::Value> = list_resp.json().await.unwrap();
  assert_eq!(leases.len(), 0);
}
