//! Application configuration.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// Discovery mode for finding peers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiscoveryMode {
    /// Local network discovery only (UDP multicast)
    Local,
    /// Internet discovery only (bootstrap server)
    Internet,
}

impl Default for DiscoveryMode {
    fn default() -> Self {
        DiscoveryMode::Local
    }
}

impl std::str::FromStr for DiscoveryMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(DiscoveryMode::Local),
            "internet" => Ok(DiscoveryMode::Internet),
            _ => Err(format!(
                "Invalid discovery mode '{}'. Valid options: local, internet",
                s
            )),
        }
    }
}

impl std::fmt::Display for DiscoveryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryMode::Local => write!(f, "local"),
            DiscoveryMode::Internet => write!(f, "internet"),
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Discovery mode
    /// Default: local
    #[serde(default)]
    pub mode: DiscoveryMode,

    /// Bootstrap server URL for internet discovery
    /// Default: ws://localhost:8080
    #[serde(default = "default_bootstrap_server")]
    pub bootstrap_server: String,
}

/// Discovery configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    /// Heartbeat interval in seconds for bootstrap server
    /// Default: 10 seconds
    #[serde(default = "default_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,

    /// Peer timeout in seconds
    /// Default: 30 seconds
    #[serde(default = "default_discovery_peer_timeout_secs")]
    pub peer_timeout_secs: u64,
}

/// Peer behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Timeout in seconds before a peer is considered offline
    /// Default: 15 seconds
    #[serde(default = "default_peer_timeout_secs")]
    pub timeout_secs: u64,

    /// Interval in seconds between peer announcements
    /// Default: 5 seconds
    #[serde(default = "default_announce_interval_secs")]
    pub announce_interval_secs: u64,
}

/// Complete application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub network: NetworkConfig,

    #[serde(default)]
    pub discovery: DiscoveryConfig,

    #[serde(default)]
    pub peer: PeerConfig,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path.as_ref()).map_err(|e| ConfigError::IoError {
            path: path.as_ref().display().to_string(),
            source: e,
        })?;

        toml::from_str(&contents).map_err(|e| ConfigError::ParseError {
            path: path.as_ref().display().to_string(),
            source: e,
        })
    }

    /// Get peer timeout as Duration
    pub fn peer_timeout(&self) -> Duration {
        Duration::from_secs(self.peer.timeout_secs)
    }

    /// Get announcement interval as Duration
    pub fn announce_interval(&self) -> Duration {
        Duration::from_secs(self.peer.announce_interval_secs)
    }

    /// Create a default configuration and write it to a file
    pub fn write_default<P: AsRef<Path>>(path: P) -> Result<(), ConfigError> {
        let config = Config::default();
        let toml = toml::to_string_pretty(&config)
            .map_err(|e| ConfigError::SerializeError { source: e })?;

        fs::write(path.as_ref(), toml).map_err(|e| ConfigError::IoError {
            path: path.as_ref().display().to_string(),
            source: e,
        })?;

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: NetworkConfig::default(),
            discovery: DiscoveryConfig::default(),
            peer: PeerConfig::default(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: DiscoveryMode::default(),
            bootstrap_server: default_bootstrap_server(),
        }
    }
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_secs: default_heartbeat_interval_secs(),
            peer_timeout_secs: default_discovery_peer_timeout_secs(),
        }
    }
}

impl Default for PeerConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_peer_timeout_secs(),
            announce_interval_secs: default_announce_interval_secs(),
        }
    }
}

// Default value functions for serde
fn default_bootstrap_server() -> String {
    "ws://localhost:8080".to_string()
}

fn default_heartbeat_interval_secs() -> u64 {
    10
}

fn default_discovery_peer_timeout_secs() -> u64 {
    30
}

fn default_peer_timeout_secs() -> u64 {
    15
}

fn default_announce_interval_secs() -> u64 {
    5
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to read config file {path}: {source}")]
    IoError {
        path: String,
        source: std::io::Error,
    },

    #[error("Failed to parse config file {path}: {source}")]
    ParseError {
        path: String,
        source: toml::de::Error,
    },

    #[error("Failed to serialize config: {source}")]
    SerializeError { source: toml::ser::Error },
}
