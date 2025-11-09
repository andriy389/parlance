//! WebSocket server implementation for the bootstrap server.
//!
//! This module handles incoming WebSocket connections, processes client messages,
//! and manages peer state through the registry.

use crate::protocol::{ClientMessage, ServerMessage};
use crate::registry::PeerRegistry;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

/// Bootstrap server that handles WebSocket connections.
pub struct BootstrapServer {
    registry: PeerRegistry,
    listener: TcpListener,
}

impl BootstrapServer {
    /// Creates a new bootstrap server bound to the given address.
    pub async fn new(addr: SocketAddr) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Bootstrap server listening on {}", addr);

        Ok(Self {
            registry: PeerRegistry::new(),
            listener,
        })
    }

    /// Runs the bootstrap server, accepting and handling connections.
    pub async fn run(self) -> Result<(), std::io::Error> {
        let registry = Arc::new(self.registry);

        let cleanup_registry = registry.clone();
        tokio::spawn(async move {
            cleanup_task(cleanup_registry).await;
        });

        loop {
            match self.listener.accept().await {
                Ok((stream, addr)) => {
                    let registry = registry.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, addr, registry).await {
                            tracing::error!(addr = %addr, error = %e, "Connection handler error");
                        }
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to accept connection");
                }
            }
        }
    }

    /// Returns the local address the server is bound to.
    pub fn local_addr(&self) -> Result<SocketAddr, std::io::Error> {
        self.listener.local_addr()
    }
}

/// Handles a single WebSocket connection.
async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    registry: Arc<PeerRegistry>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!(addr = %addr, "New connection");

    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    let peer_id: Arc<RwLock<Option<Uuid>>> = Arc::new(RwLock::new(None));
    let connection_peer_id = peer_id.clone();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                let response = process_message(&text, addr, &registry, &peer_id).await;

                if let Some(response_msg) = response {
                    let json = serde_json::to_string(&response_msg)?;
                    write.send(Message::Text(json)).await?;
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!(addr = %addr, "Connection closed by client");
                break;
            }
            Ok(Message::Ping(data)) => {
                write.send(Message::Pong(data)).await?;
            }
            Ok(_) => {
                // Ignore other message types
            }
            Err(e) => {
                tracing::error!(addr = %addr, error = %e, "WebSocket error");
                break;
            }
        }
    }

    if let Some(id) = *connection_peer_id.read().await {
        registry.unregister(id).await;
        tracing::info!(addr = %addr, peer_id = %id, "Connection closed, peer unregistered");
    }

    Ok(())
}

/// Processes a client message and returns an optional response.
async fn process_message(
    text: &str,
    addr: SocketAddr,
    registry: &PeerRegistry,
    peer_id: &Arc<RwLock<Option<Uuid>>>,
) -> Option<ServerMessage> {
    let client_msg: ClientMessage = match serde_json::from_str(text) {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!(error = %e, "Failed to parse client message");
            return Some(ServerMessage::Error {
                message: format!("Invalid message format: {}", e),
            });
        }
    };

    match client_msg {
        ClientMessage::Register {
            nickname,
            local_addr,
        } => {
            let public_addr = if let Ok(local_socket_addr) = local_addr.parse::<std::net::SocketAddr>() {
                let public_ip = addr.ip();
                let tcp_port = local_socket_addr.port();
                format!("{}:{}", public_ip, tcp_port)
            } else {
                tracing::warn!("Failed to parse local_addr: {}, using WebSocket addr", local_addr);
                addr.to_string()
            };

            let id = registry
                .register(nickname, local_addr, public_addr.clone())
                .await;

            *peer_id.write().await = Some(id);

            Some(ServerMessage::Registered {
                peer_id: id.to_string(),
                public_addr,
            })
        }
        ClientMessage::ListPeers => {
            let peers = registry.list_peers().await;
            Some(ServerMessage::PeerList { peers })
        }
        ClientMessage::Heartbeat => {
            if let Some(id) = *peer_id.read().await {
                if registry.update_heartbeat(id).await {
                    None
                } else {
                    Some(ServerMessage::Error {
                        message: "Peer not registered".to_string(),
                    })
                }
            } else {
                Some(ServerMessage::Error {
                    message: "Not registered".to_string(),
                })
            }
        }
        ClientMessage::Unregister => {
            if let Some(id) = *peer_id.write().await {
                registry.unregister(id).await;
                *peer_id.write().await = None;
                None
            } else {
                Some(ServerMessage::Error {
                    message: "Not registered".to_string(),
                })
            }
        }
    }
}

/// Background task that periodically removes stale peers.
async fn cleanup_task(registry: Arc<PeerRegistry>) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));

    loop {
        interval.tick().await;
        let removed = registry.remove_stale_peers().await;
        if removed > 0 {
            tracing::info!(count = removed, "Removed stale peers");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = BootstrapServer::new("127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        assert!(server.local_addr().is_ok());
    }

    #[tokio::test]
    async fn test_process_register_message() {
        let registry = PeerRegistry::new();
        let peer_id = Arc::new(RwLock::new(None));
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let msg = ClientMessage::Register {
            nickname: "test".to_string(),
            local_addr: "192.168.1.1:5000".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();

        let response = process_message(&json, addr, &registry, &peer_id).await;

        assert!(response.is_some());
        match response.unwrap() {
            ServerMessage::Registered {
                peer_id: id,
                public_addr,
            } => {
                assert!(!id.is_empty());
                assert_eq!(public_addr, "127.0.0.1:5000");
            }
            _ => panic!("Expected Registered message"),
        }

        assert!(peer_id.read().await.is_some());
    }

    #[tokio::test]
    async fn test_process_list_peers_message() {
        let registry = PeerRegistry::new();
        let peer_id = Arc::new(RwLock::new(None));
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        registry
            .register(
                "peer1".to_string(),
                "192.168.1.1:5000".to_string(),
                "1.1.1.1:5000".to_string(),
            )
            .await;

        let msg = ClientMessage::ListPeers;
        let json = serde_json::to_string(&msg).unwrap();

        let response = process_message(&json, addr, &registry, &peer_id).await;

        assert!(response.is_some());
        match response.unwrap() {
            ServerMessage::PeerList { peers } => {
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0].nickname, "peer1");
            }
            _ => panic!("Expected PeerList message"),
        }
    }

    #[tokio::test]
    async fn test_process_heartbeat_not_registered() {
        let registry = PeerRegistry::new();
        let peer_id = Arc::new(RwLock::new(None));
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let msg = ClientMessage::Heartbeat;
        let json = serde_json::to_string(&msg).unwrap();

        let response = process_message(&json, addr, &registry, &peer_id).await;

        assert!(response.is_some());
        match response.unwrap() {
            ServerMessage::Error { message } => {
                assert_eq!(message, "Not registered");
            }
            _ => panic!("Expected Error message"),
        }
    }

    #[tokio::test]
    async fn test_process_invalid_message() {
        let registry = PeerRegistry::new();
        let peer_id = Arc::new(RwLock::new(None));
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();

        let response = process_message("invalid json", addr, &registry, &peer_id).await;

        assert!(response.is_some());
        match response.unwrap() {
            ServerMessage::Error { message } => {
                assert!(message.contains("Invalid message format"));
            }
            _ => panic!("Expected Error message"),
        }
    }
}
