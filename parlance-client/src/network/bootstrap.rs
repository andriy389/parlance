//! Bootstrap client for internet-scale peer discovery.
//!
//! This module implements a WebSocket client that connects to a bootstrap server
//! to discover peers across the internet, complementing local network discovery.

use crate::core::error::{ParlanceError, Result};
use crate::core::peer::{Peer, PeerRegistry};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// Heartbeat interval in seconds.
const HEARTBEAT_INTERVAL_SECS: u64 = 10;

/// Initial reconnection delay in seconds.
const INITIAL_RECONNECT_DELAY_SECS: u64 = 1;

/// Maximum reconnection delay in seconds.
const MAX_RECONNECT_DELAY_SECS: u64 = 30;

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Register {
        nickname: String,
        local_addr: String,
    },
    ListPeers,
    Heartbeat,
    Unregister,
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage {
    Registered {
        peer_id: String,
        public_addr: String,
    },
    PeerList {
        peers: Vec<PeerInfo>,
    },
    Error {
        message: String,
    },
}

/// Information about a peer from the bootstrap server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PeerInfo {
    peer_id: String,
    nickname: String,
    public_addr: String,
    local_addr: String,
    last_seen: i64,
}

/// Bootstrap client for connecting to the bootstrap server.
pub struct BootstrapClient {
    server_url: String,
    nickname: String,
    local_addr: SocketAddr,
    peer_registry: Arc<PeerRegistry>,
    ws_stream: Option<WsStream>,
    peer_id: Option<String>,
    public_addr: Option<String>,
}

impl BootstrapClient {
    /// Creates a new bootstrap client.
    pub fn new(
        server_url: String,
        nickname: String,
        local_addr: SocketAddr,
        peer_registry: Arc<PeerRegistry>,
    ) -> Self {
        Self {
            server_url,
            nickname,
            local_addr,
            peer_registry,
            ws_stream: None,
            peer_id: None,
            public_addr: None,
        }
    }

    /// Connects to the bootstrap server.
    pub async fn connect(&mut self) -> Result<()> {
        tracing::info!(url = %self.server_url, "Connecting to bootstrap server");

        let (ws_stream, _) = connect_async(&self.server_url)
            .await
            .map_err(|e| ParlanceError::BootstrapConnection(e.to_string()))?;

        self.ws_stream = Some(ws_stream);

        tracing::info!("Connected to bootstrap server");
        Ok(())
    }

    /// Registers with the bootstrap server.
    async fn register(&mut self) -> Result<()> {
        let msg = ClientMessage::Register {
            nickname: self.nickname.clone(),
            local_addr: self.local_addr.to_string(),
        };

        self.send_message(&msg).await?;

        tracing::info!("Sent registration to bootstrap server");
        Ok(())
    }

    /// Sends a message to the bootstrap server.
    async fn send_message(&mut self, msg: &ClientMessage) -> Result<()> {
        let ws = self
            .ws_stream
            .as_mut()
            .ok_or_else(|| ParlanceError::BootstrapConnection("Not connected".to_string()))?;

        let json = serde_json::to_string(msg)?;
        ws.send(Message::Text(json))
            .await
            .map_err(|e| ParlanceError::WebSocket(e.to_string()))?;

        Ok(())
    }

    /// Receives and processes a message from the bootstrap server.
    async fn receive_message(&mut self) -> Result<Option<ServerMessage>> {
        let ws = self
            .ws_stream
            .as_mut()
            .ok_or_else(|| ParlanceError::BootstrapConnection("Not connected".to_string()))?;

        match ws.next().await {
            Some(Ok(Message::Text(text))) => {
                let server_msg: ServerMessage = serde_json::from_str(&text)?;
                Ok(Some(server_msg))
            }
            Some(Ok(Message::Close(_))) => {
                tracing::info!("Server closed connection");
                Ok(None)
            }
            Some(Ok(Message::Ping(data))) => {
                ws.send(Message::Pong(data))
                    .await
                    .map_err(|e| ParlanceError::WebSocket(e.to_string()))?;
                Ok(None)
            }
            Some(Ok(_)) => Ok(None),
            Some(Err(e)) => Err(ParlanceError::WebSocket(e.to_string())),
            None => {
                tracing::info!("Connection closed");
                Ok(None)
            }
        }
    }

    /// Processes a server message.
    async fn process_server_message(&mut self, msg: ServerMessage) -> Result<()> {
        match msg {
            ServerMessage::Registered {
                peer_id,
                public_addr,
            } => {
                tracing::info!(
                    peer_id = %peer_id,
                    public_addr = %public_addr,
                    "Registered with bootstrap server"
                );
                self.peer_id = Some(peer_id);
                self.public_addr = Some(public_addr);
            }
            ServerMessage::PeerList { peers } => {
                tracing::debug!(count = peers.len(), "Received peer list from bootstrap server");
                self.update_peer_registry(peers).await?;
            }
            ServerMessage::Error { message } => {
                tracing::error!(error = %message, "Bootstrap server error");
                return Err(ParlanceError::BootstrapServerError(message));
            }
        }
        Ok(())
    }

    /// Updates the peer registry with peers from the bootstrap server.
    async fn update_peer_registry(&self, peers: Vec<PeerInfo>) -> Result<()> {
        for peer_info in peers {
            if let Some(ref my_id) = self.peer_id {
                if peer_info.peer_id == *my_id {
                    continue;
                }
            }

            let addr = if let Ok(addr) = peer_info.public_addr.parse::<SocketAddr>() {
                addr
            } else if let Ok(addr) = peer_info.local_addr.parse::<SocketAddr>() {
                addr
            } else {
                tracing::warn!(
                    peer_id = %peer_info.peer_id,
                    "Failed to parse peer addresses, skipping"
                );
                continue;
            };

            let peer = Peer::new(peer_info.nickname, addr);
            self.peer_registry.upsert(peer).await;
        }

        Ok(())
    }

    /// Sends a heartbeat to the bootstrap server.
    async fn send_heartbeat(&mut self) -> Result<()> {
        self.send_message(&ClientMessage::Heartbeat).await
    }

    /// Requests the peer list from the bootstrap server.
    async fn request_peer_list(&mut self) -> Result<()> {
        self.send_message(&ClientMessage::ListPeers).await
    }

    /// Runs the bootstrap client with reconnection logic.
    pub async fn run(&mut self) -> Result<()> {
        let mut reconnect_delay = INITIAL_RECONNECT_DELAY_SECS;

        loop {
            match self.connect().await {
                Ok(()) => {
                    if let Err(e) = self.register().await {
                        tracing::error!(error = %e, "Failed to register");
                        sleep(Duration::from_secs(reconnect_delay)).await;
                        continue;
                    }

                    reconnect_delay = INITIAL_RECONNECT_DELAY_SECS;

                    if let Err(e) = self.run_loop().await {
                        tracing::error!(error = %e, "Bootstrap client error");
                    }
                }
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        delay_secs = reconnect_delay,
                        "Failed to connect to bootstrap server, retrying"
                    );
                }
            }

            sleep(Duration::from_secs(reconnect_delay)).await;

            reconnect_delay = (reconnect_delay * 2).min(MAX_RECONNECT_DELAY_SECS);

            self.ws_stream = None;
            self.peer_id = None;
            self.public_addr = None;
        }
    }

    /// Main event loop for the bootstrap client.
    async fn run_loop(&mut self) -> Result<()> {
        let mut heartbeat_interval = tokio::time::interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        let mut peer_list_interval = tokio::time::interval(Duration::from_secs(5));

        heartbeat_interval.tick().await;
        peer_list_interval.tick().await;

        self.request_peer_list().await?;

        loop {
            tokio::select! {
                _ = heartbeat_interval.tick() => {
                    if let Err(e) = self.send_heartbeat().await {
                        tracing::error!(error = %e, "Failed to send heartbeat");
                        return Err(e);
                    }
                }

                _ = peer_list_interval.tick() => {
                    if let Err(e) = self.request_peer_list().await {
                        tracing::error!(error = %e, "Failed to request peer list");
                        return Err(e);
                    }
                }

                result = self.receive_message() => {
                    match result {
                        Ok(Some(msg)) => {
                            if let Err(e) = self.process_server_message(msg).await {
                                tracing::error!(error = %e, "Failed to process server message");
                                return Err(e);
                            }
                        }
                        Ok(None) => {
                            // Connection closed or ignorable message
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "Failed to receive message");
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    /// Disconnects from the bootstrap server.
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(mut ws) = self.ws_stream.take() {
            let _ = self.send_message(&ClientMessage::Unregister).await;
            let _ = ws.close(None).await;
            tracing::info!("Disconnected from bootstrap server");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_serialization() {
        let msg = ClientMessage::Register {
            nickname: "test".to_string(),
            local_addr: "192.168.1.1:5000".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"register\""));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_server_message_deserialization() {
        let json = r#"{"type":"registered","peer_id":"123","public_addr":"1.2.3.4:5000"}"#;
        let msg: ServerMessage = serde_json::from_str(json).unwrap();

        match msg {
            ServerMessage::Registered {
                peer_id,
                public_addr,
            } => {
                assert_eq!(peer_id, "123");
                assert_eq!(public_addr, "1.2.3.4:5000");
            }
            _ => panic!("Expected Registered message"),
        }
    }

    #[test]
    fn test_peer_info_deserialization() {
        let json = r#"{
            "peer_id": "abc-123",
            "nickname": "alice",
            "public_addr": "1.2.3.4:5000",
            "local_addr": "192.168.1.100:5000",
            "last_seen": 1699564800
        }"#;

        let peer_info: PeerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(peer_info.peer_id, "abc-123");
        assert_eq!(peer_info.nickname, "alice");
        assert_eq!(peer_info.public_addr, "1.2.3.4:5000");
        assert_eq!(peer_info.local_addr, "192.168.1.100:5000");
        assert_eq!(peer_info.last_seen, 1699564800);
    }

    #[tokio::test]
    async fn test_bootstrap_client_creation() {
        let registry = Arc::new(PeerRegistry::new());
        let client = BootstrapClient::new(
            "ws://localhost:8080".to_string(),
            "test".to_string(),
            "127.0.0.1:5000".parse().unwrap(),
            registry,
        );

        assert_eq!(client.server_url, "ws://localhost:8080");
        assert_eq!(client.nickname, "test");
        assert!(client.peer_id.is_none());
        assert!(client.public_addr.is_none());
    }
}
