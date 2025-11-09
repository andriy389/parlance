//! Parlance - P2P Messaging Library
pub mod app;
pub mod core;
pub mod network;

pub use core::{error, peer};
pub use network::{discovery, messaging};
