//! Integration tests for bootstrap server and client.
//!
//! These tests verify that the bootstrap server and client work together
//! correctly for peer discovery.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

// Import from bootstrap-server (we'd need to expose these as a lib)
// For now, we'll just test the client side with a mock/real server

use parlance::core::peer::{Peer, PeerRegistry};
use parlance::network::bootstrap::BootstrapClient;

/// Test that demonstrates bootstrap client can be created and initialized.
#[tokio::test]
async fn test_bootstrap_client_creation() {
    let registry = Arc::new(PeerRegistry::new());
    let local_addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();

    let _client = BootstrapClient::new(
        "ws://localhost:8080".to_string(),
        "test_peer".to_string(),
        local_addr,
        registry,
    );

}

/// Test that peer registry updates work correctly.
#[tokio::test]
async fn test_peer_registry_integration() {
    let registry = Arc::new(PeerRegistry::new());

    let peer1 = Peer::new("alice".to_string(), "192.168.1.100:5000".parse().unwrap());
    let peer2 = Peer::new("bob".to_string(), "192.168.1.101:5000".parse().unwrap());

    registry.upsert(peer1.clone()).await;
    registry.upsert(peer2.clone()).await;

    let peers = registry.get_all().await;
    assert_eq!(peers.len(), 2);

    let nicknames: Vec<String> = peers.iter().map(|p| p.nickname.clone()).collect();
    assert!(nicknames.contains(&"alice".to_string()));
    assert!(nicknames.contains(&"bob".to_string()));
}

/// Test timeout behavior of peer registry.
#[tokio::test]
async fn test_peer_timeout() {
    let registry = Arc::new(PeerRegistry::new());

    let peer = Peer::new("charlie".to_string(), "192.168.1.102:5000".parse().unwrap());
    registry.upsert(peer).await;

    assert_eq!(registry.count().await, 1);

    sleep(Duration::from_millis(100)).await;
    let removed = registry.remove_timed_out(Duration::from_millis(50)).await;

    assert_eq!(removed.len(), 1);
    assert_eq!(registry.count().await, 0);
}

/// Test that demonstrates dual registry (local + internet) doesn't cause conflicts.
#[tokio::test]
async fn test_dual_mode_peer_registry() {
    let registry = Arc::new(PeerRegistry::new());

    let local_peer = Peer::new("local1".to_string(), "192.168.1.10:5000".parse().unwrap());
    registry.upsert(local_peer).await;

    let internet_peer = Peer::new(
        "internet1".to_string(),
        "203.0.113.10:5000".parse().unwrap(),
    );
    registry.upsert(internet_peer).await;

    let peers = registry.get_all().await;
    assert_eq!(peers.len(), 2);

    let nicknames: Vec<String> = peers.iter().map(|p| p.nickname.clone()).collect();
    assert!(nicknames.contains(&"local1".to_string()));
    assert!(nicknames.contains(&"internet1".to_string()));
}
