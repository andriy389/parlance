//! Parlance Application
//!
//! A peer-to-peer messaging application for local networks using UDP multicast
//! for peer discovery and TCP for direct messaging.

mod app;
mod core;
mod network;

use app::{App, AppConfig};
use clap::Parser;
use core::config::DiscoveryMode;
use core::error::Result;
use core::validation::NicknameValidator;
use std::path::PathBuf;
use tracing_subscriber::fmt;

/// Parlance - Local Network P2P Messaging
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Nickname (1-32 characters, no control characters)
    #[arg(short, long)]
    nickname: String,

    /// Path to configuration file (optional)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Generate a default configuration file
    #[arg(long, value_name = "FILE")]
    generate_config: Option<PathBuf>,

    /// Discovery mode (overrides config file)
    #[arg(short, long, value_name = "MODE")]
    mode: Option<DiscoveryMode>,

    /// Bootstrap server URL (overrides config file)
    #[arg(long, value_name = "URL")]
    bootstrap_server: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let args = Args::parse();

    if let Some(config_path) = args.generate_config {
        core::config::Config::write_default(&config_path)?;
        println!(
            "Generated default configuration at: {}",
            config_path.display()
        );
        return Ok(());
    }

    let mut config = if let Some(config_path) = args.config {
        core::config::Config::from_file(&config_path)
            .map_err(|e| core::error::ParlanceError::ConfigError(e.to_string()))?
    } else {
        let default_path = PathBuf::from("parlance-client/parlance.toml");

        if default_path.exists() {
            tracing::debug!("Loading configuration from {}", default_path.display());
            core::config::Config::from_file(&default_path)
                .map_err(|e| core::error::ParlanceError::ConfigError(e.to_string()))?
        } else {
            tracing::debug!("No parlance.toml found, using default configuration");
            core::config::Config::default()
        }
    };

    if let Some(mode) = args.mode {
        tracing::info!("Overriding discovery mode from CLI: {}", mode);
        config.network.mode = mode;
    }

    if let Some(server) = args.bootstrap_server {
        tracing::info!("Overriding bootstrap server from CLI: {}", server);
        config.network.bootstrap_server = server;
    }

    NicknameValidator::validate(&args.nickname)
        .map_err(|e| core::error::ParlanceError::ConfigError(format!("Invalid nickname: {}", e)))?;

    let app_config = AppConfig::new(args.nickname);
    let app = App::new(app_config, config);

    app.run().await
}
