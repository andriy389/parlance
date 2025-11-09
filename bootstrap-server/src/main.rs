//! Bootstrap server for Parlance P2P messaging.
//!
//! This server helps peers discover each other across the internet by maintaining
//! a registry of connected peers and their addresses.

mod protocol;
mod registry;
mod server;

use clap::Parser;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Bootstrap server for Parlance peer discovery.
#[derive(Parser, Debug)]
#[command(name = "bootstrap-server")]
#[command(about = "Bootstrap server for Parlance P2P messaging", long_about = None)]
struct Args {
    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    host: String,

    /// Port to listen on
    #[arg(long, default_value = "8080")]
    port: u16,

    /// Path to TLS certificate file (optional, for WSS support)
    #[arg(long)]
    cert: Option<String>,

    /// Path to TLS private key file (optional, for WSS support)
    #[arg(long)]
    key: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    let bind_addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;

    if args.cert.is_some() || args.key.is_some() {
        if args.cert.is_none() || args.key.is_none() {
            return Err("Both --cert and --key must be provided for TLS support".into());
        }
        tracing::warn!("TLS support is not yet implemented in this version");
        tracing::warn!("Server will run without TLS (WS instead of WSS)");
    }

    tracing::info!("Starting bootstrap server on {}", bind_addr);

    let server = server::BootstrapServer::new(bind_addr).await?;

    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        tracing::info!("Received shutdown signal");
    };

    tokio::select! {
        result = server.run() => {
            if let Err(e) = result {
                tracing::error!(error = %e, "Server error");
                return Err(e.into());
            }
        }
        _ = shutdown => {
            tracing::info!("Shutting down gracefully");
        }
    }

    Ok(())
}
