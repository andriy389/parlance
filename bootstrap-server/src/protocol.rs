//! Bootstrap server protocol definitions.
//!
//! This module defines the message types and peer information structures
//! used for communication between the bootstrap server and clients.

use serde::{Deserialize, Serialize};

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// Register a new peer with the server.
    Register {
        /// The peer's chosen nickname.
        nickname: String,
        /// The peer's local network address (e.g., "192.168.1.100:5000").
        local_addr: String,
    },
    /// Request the current list of registered peers.
    ListPeers,
    /// Heartbeat to keep the connection alive and update last_seen timestamp.
    Heartbeat,
    /// Unregister from the server.
    Unregister,
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Confirmation of successful registration.
    Registered {
        /// Unique identifier assigned to this peer.
        peer_id: String,
        /// The peer's public address as seen by the server.
        public_addr: String,
    },
    /// List of currently registered peers.
    PeerList {
        /// Vector of peer information.
        peers: Vec<PeerInfo>,
    },
    /// Error message.
    Error {
        /// Description of the error.
        message: String,
    },
}

/// Information about a registered peer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PeerInfo {
    /// Unique peer identifier.
    pub peer_id: String,
    /// Peer's nickname.
    pub nickname: String,
    /// Public address as seen by the server.
    pub public_addr: String,
    /// Local address reported by the peer.
    pub local_addr: String,
    /// Unix timestamp of last activity.
    pub last_seen: i64,
}

impl PeerInfo {
    /// Creates a new PeerInfo instance.
    pub fn new(
        peer_id: String,
        nickname: String,
        public_addr: String,
        local_addr: String,
        last_seen: i64,
    ) -> Self {
        Self {
            peer_id,
            nickname,
            public_addr,
            local_addr,
            last_seen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_message_register_serialization() {
        let msg = ClientMessage::Register {
            nickname: "alice".to_string(),
            local_addr: "192.168.1.100:5000".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"register\""));
        assert!(json.contains("\"nickname\":\"alice\""));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_client_message_list_peers_serialization() {
        let msg = ClientMessage::ListPeers;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"list_peers\""));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_client_message_heartbeat_serialization() {
        let msg = ClientMessage::Heartbeat;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"heartbeat\""));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_client_message_unregister_serialization() {
        let msg = ClientMessage::Unregister;
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"unregister\""));

        let deserialized: ClientMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_server_message_registered_serialization() {
        let msg = ServerMessage::Registered {
            peer_id: "test-id".to_string(),
            public_addr: "1.2.3.4:5000".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"registered\""));
        assert!(json.contains("\"peer_id\":\"test-id\""));

        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_server_message_peer_list_serialization() {
        let peers = vec![PeerInfo::new(
            "id1".to_string(),
            "alice".to_string(),
            "1.2.3.4:5000".to_string(),
            "192.168.1.100:5000".to_string(),
            1699564800,
        )];
        let msg = ServerMessage::PeerList { peers };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"peer_list\""));

        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_server_message_error_serialization() {
        let msg = ServerMessage::Error {
            message: "test error".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"message\":\"test error\""));

        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_peer_info_creation() {
        let peer = PeerInfo::new(
            "id1".to_string(),
            "bob".to_string(),
            "5.6.7.8:9000".to_string(),
            "10.0.0.5:9000".to_string(),
            1699564900,
        );
        assert_eq!(peer.peer_id, "id1");
        assert_eq!(peer.nickname, "bob");
        assert_eq!(peer.public_addr, "5.6.7.8:9000");
        assert_eq!(peer.local_addr, "10.0.0.5:9000");
        assert_eq!(peer.last_seen, 1699564900);
    }
}
