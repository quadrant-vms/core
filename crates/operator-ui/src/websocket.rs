use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time;
use tracing::{error, info};

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    Ping,
    Pong,
    Subscribe { topics: Vec<String> },
    Unsubscribe { topics: Vec<String> },
    Update { topic: String, data: serde_json::Value },
    Error { message: String },
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();

    // Spawn a task to send periodic updates
    let mut update_interval = time::interval(Duration::from_secs(5));
    let send_task = tokio::spawn(async move {
        loop {
            update_interval.tick().await;

            // Send dashboard stats update
            match fetch_dashboard_update(&state).await {
                Ok(update) => {
                    let msg = WsMessage::Update {
                        topic: "dashboard".to_string(),
                        data: serde_json::to_value(update).unwrap_or_default(),
                    };

                    if let Ok(json) = serde_json::to_string(&msg) {
                        if sender.send(Message::Text(json)).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch dashboard update: {}", e);
                }
            }
        }
    });

    // Handle incoming messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                        match ws_msg {
                            WsMessage::Ping => {
                                info!("Received ping");
                            }
                            WsMessage::Subscribe { topics } => {
                                info!("Client subscribed to topics: {:?}", topics);
                            }
                            WsMessage::Unsubscribe { topics } => {
                                info!("Client unsubscribed from topics: {:?}", topics);
                            }
                            _ => {}
                        }
                    }
                }
                Message::Close(_) => {
                    info!("Client disconnected");
                    break;
                }
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

async fn fetch_dashboard_update(state: &AppState) -> anyhow::Result<serde_json::Value> {
    // Fetch quick stats for real-time updates
    let device_url = format!("{}/devices", state.config.device_manager_url);
    let stream_url = format!("{}/streams", state.config.admin_gateway_url);

    let (devices_result, streams_result) = tokio::join!(
        state.http_client.get(&device_url).send(),
        state.http_client.get(&stream_url).send()
    );

    let devices_count = if let Ok(resp) = devices_result {
        if resp.status().is_success() {
            resp.json::<Vec<serde_json::Value>>()
                .await
                .map(|v| v.len())
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    let streams_count = if let Ok(resp) = streams_result {
        if resp.status().is_success() {
            resp.json::<Vec<serde_json::Value>>()
                .await
                .map(|v| v.len())
                .unwrap_or(0)
        } else {
            0
        }
    } else {
        0
    };

    Ok(serde_json::json!({
        "devices": devices_count,
        "streams": streams_count,
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
