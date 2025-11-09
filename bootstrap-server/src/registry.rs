//! Peer registry management for the bootstrap server.
//!
//! This module provides thread-safe management of registered peers,
//! including registration, lookup, timeout handling, and cleanup.

use crate::protocol::PeerInfo;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Timeout for inactive peers (in seconds).
const PEER_TIMEOUT_SECS: i64 = 30;

/// Internal peer data stored in the registry.
#[derive(Debug, Clone)]
struct Peer {
    info: PeerInfo,
}

/// Thread-safe registry for managing connected peers.
#[derive(Debug, Clone)]
pub struct PeerRegistry {
    peers: Arc<RwLock<HashMap<Uuid, Peer>>>,
}

impl PeerRegistry {
    /// Creates a new empty peer registry.
    pub fn new() -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new peer and returns the assigned peer ID and public address.
    pub async fn register(
        &self,
        nickname: String,
        local_addr: String,
        public_addr: String,
    ) -> Uuid {
        let peer_id = Uuid::new_v4();
        let now = Utc::now().timestamp();

        let peer = Peer {
            info: PeerInfo::new(
                peer_id.to_string(),
                nickname,
                public_addr,
                local_addr,
                now,
            ),
        };

        let mut peers = self.peers.write().await;
        peers.insert(peer_id, peer);

        tracing::info!(
            peer_id = %peer_id,
            "Peer registered"
        );

        peer_id
    }

    /// Updates the last_seen timestamp for a peer.
    pub async fn update_heartbeat(&self, peer_id: Uuid) -> bool {
        let mut peers = self.peers.write().await;
        if let Some(peer) = peers.get_mut(&peer_id) {
            peer.info.last_seen = Utc::now().timestamp();
            true
        } else {
            false
        }
    }

    /// Unregisters a peer by ID.
    pub async fn unregister(&self, peer_id: Uuid) -> bool {
        let mut peers = self.peers.write().await;
        if peers.remove(&peer_id).is_some() {
            tracing::info!(peer_id = %peer_id, "Peer unregistered");
            true
        } else {
            false
        }
    }

    /// Gets the public address for a peer.
    pub async fn get_public_addr(&self, peer_id: Uuid) -> Option<String> {
        let peers = self.peers.read().await;
        peers.get(&peer_id).map(|p| p.info.public_addr.clone())
    }

    /// Returns a list of all registered peers.
    pub async fn list_peers(&self) -> Vec<PeerInfo> {
        let peers = self.peers.read().await;
        peers.values().map(|p| p.info.clone()).collect()
    }

    /// Removes peers that have not sent a heartbeat within the timeout period.
    pub async fn remove_stale_peers(&self) -> usize {
        let now = Utc::now().timestamp();
        let mut peers = self.peers.write().await;

        let stale_peers: Vec<Uuid> = peers
            .iter()
            .filter(|(_, peer)| now - peer.info.last_seen > PEER_TIMEOUT_SECS)
            .map(|(id, _)| *id)
            .collect();

        let count = stale_peers.len();
        for peer_id in stale_peers {
            peers.remove(&peer_id);
            tracing::info!(peer_id = %peer_id, "Removed stale peer");
        }

        count
    }

    /// Returns the total number of registered peers.
    pub async fn peer_count(&self) -> usize {
        let peers = self.peers.read().await;
        peers.len()
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn test_register_peer() {
        let registry = PeerRegistry::new();
        let peer_id = registry
            .register(
                "alice".to_string(),
                "192.168.1.100:5000".to_string(),
                "1.2.3.4:5000".to_string(),
            )
            .await;

        let public_addr = registry.get_public_addr(peer_id).await;
        assert_eq!(public_addr, Some("1.2.3.4:5000".to_string()));

        let peers = registry.list_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].nickname, "alice");
        assert_eq!(peers[0].peer_id, peer_id.to_string());
    }

    #[tokio::test]
    async fn test_unregister_peer() {
        let registry = PeerRegistry::new();
        let peer_id = registry
            .register(
                "bob".to_string(),
                "192.168.1.101:5000".to_string(),
                "5.6.7.8:5000".to_string(),
            )
            .await;

        assert_eq!(registry.peer_count().await, 1);

        let result = registry.unregister(peer_id).await;
        assert!(result);

        assert_eq!(registry.peer_count().await, 0);
        assert_eq!(registry.get_public_addr(peer_id).await, None);
    }

    #[tokio::test]
    async fn test_unregister_nonexistent_peer() {
        let registry = PeerRegistry::new();
        let fake_id = Uuid::new_v4();

        let result = registry.unregister(fake_id).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_update_heartbeat() {
        let registry = PeerRegistry::new();
        let peer_id = registry
            .register(
                "charlie".to_string(),
                "192.168.1.102:5000".to_string(),
                "9.10.11.12:5000".to_string(),
            )
            .await;

        let peers_before = registry.list_peers().await;
        let last_seen_before = peers_before[0].last_seen;

        sleep(Duration::from_secs(1)).await;

        let result = registry.update_heartbeat(peer_id).await;
        assert!(result);

        let peers_after = registry.list_peers().await;
        let last_seen_after = peers_after[0].last_seen;

        assert!(last_seen_after >= last_seen_before);
    }

    #[tokio::test]
    async fn test_update_heartbeat_nonexistent_peer() {
        let registry = PeerRegistry::new();
        let fake_id = Uuid::new_v4();

        let result = registry.update_heartbeat(fake_id).await;
        assert!(!result);
    }

    #[tokio::test]
    async fn test_multiple_peers() {
        let registry = PeerRegistry::new();

        registry
            .register(
                "peer1".to_string(),
                "192.168.1.1:5000".to_string(),
                "1.1.1.1:5000".to_string(),
            )
            .await;

        registry
            .register(
                "peer2".to_string(),
                "192.168.1.2:5000".to_string(),
                "2.2.2.2:5000".to_string(),
            )
            .await;

        registry
            .register(
                "peer3".to_string(),
                "192.168.1.3:5000".to_string(),
                "3.3.3.3:5000".to_string(),
            )
            .await;

        assert_eq!(registry.peer_count().await, 3);

        let peers = registry.list_peers().await;
        assert_eq!(peers.len(), 3);

        let nicknames: Vec<String> = peers.iter().map(|p| p.nickname.clone()).collect();
        assert!(nicknames.contains(&"peer1".to_string()));
        assert!(nicknames.contains(&"peer2".to_string()));
        assert!(nicknames.contains(&"peer3".to_string()));
    }

    #[tokio::test]
    async fn test_list_peers_empty() {
        let registry = PeerRegistry::new();
        let peers = registry.list_peers().await;
        assert_eq!(peers.len(), 0);
    }
}
