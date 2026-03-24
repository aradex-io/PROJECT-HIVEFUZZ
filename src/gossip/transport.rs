use std::net::SocketAddr;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::GossipMessage;

/// Transport layer for gossip protocol communication.
///
/// - UDP: Used for lightweight gossip messages (pings, coverage digests, membership).
/// - TCP: Used for bulk transfers (corpus entries, crash reproducers).
pub struct Transport {
    config: TransportConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// UDP socket bind address.
    pub udp_addr: SocketAddr,
    /// TCP listener bind address.
    pub tcp_addr: SocketAddr,
    /// Maximum UDP message size.
    pub max_udp_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            udp_addr: "0.0.0.0:7878".parse().unwrap(),
            tcp_addr: "0.0.0.0:7879".parse().unwrap(),
            max_udp_size: 65507, // max UDP payload
        }
    }
}

impl Transport {
    pub fn new(config: TransportConfig) -> Self {
        Self { config }
    }

    /// Start listening for incoming gossip messages.
    ///
    /// Returns a channel receiver that yields incoming messages.
    pub async fn start(&self) -> Result<()> {
        // TODO: Implement UDP + TCP listeners
        // 1. Bind UDP socket for gossip protocol messages
        // 2. Bind TCP listener for bulk transfers
        // 3. Spawn receiver tasks that deserialize and forward messages
        tracing::info!(
            "Transport starting on UDP={} TCP={}",
            self.config.udp_addr,
            self.config.tcp_addr
        );
        Ok(())
    }

    /// Send a gossip message to a specific peer via UDP.
    pub async fn send_udp(&self, _msg: &GossipMessage, _target: SocketAddr) -> Result<()> {
        // TODO: Serialize message and send via UDP socket
        Ok(())
    }

    /// Send a bulk transfer to a peer via TCP.
    pub async fn send_tcp(&self, _data: &[u8], _target: SocketAddr) -> Result<()> {
        // TODO: Establish TCP connection and send data
        Ok(())
    }
}
