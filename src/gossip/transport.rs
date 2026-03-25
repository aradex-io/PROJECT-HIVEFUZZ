use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use super::GossipMessage;
use crate::node::identity::NodeIdentity;

/// Transport layer for gossip protocol communication.
///
/// - UDP: Used for lightweight gossip messages (pings, coverage digests, membership).
/// - TCP: Used for bulk transfers (corpus entries, crash reproducers) — deferred to Phase 2.
pub struct Transport {
    config: TransportConfig,
    /// UDP socket, initialized after start().
    udp_socket: Option<Arc<UdpSocket>>,
}

/// Configuration for the transport layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportConfig {
    /// UDP socket bind address.
    pub udp_addr: SocketAddr,
    /// Maximum UDP message size.
    pub max_udp_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            udp_addr: "0.0.0.0:7878".parse().unwrap(),
            max_udp_size: 65507, // max UDP payload
        }
    }
}

/// An incoming message from a peer.
#[derive(Debug)]
pub struct IncomingMessage {
    /// The deserialized gossip message.
    pub message: GossipMessage,
    /// Source address of the sender.
    pub source: SocketAddr,
}

impl Transport {
    pub fn new(config: TransportConfig) -> Self {
        Self {
            config,
            udp_socket: None,
        }
    }

    /// Start the transport layer.
    ///
    /// Binds the UDP socket and returns a receiver channel for incoming messages.
    pub async fn start(&mut self) -> Result<mpsc::Receiver<IncomingMessage>> {
        let socket = UdpSocket::bind(self.config.udp_addr)
            .await
            .with_context(|| format!("Failed to bind UDP socket on {}", self.config.udp_addr))?;

        tracing::info!("Transport listening on UDP {}", self.config.udp_addr);

        let socket = Arc::new(socket);
        self.udp_socket = Some(Arc::clone(&socket));

        // Create channel for incoming messages
        let (tx, rx) = mpsc::channel::<IncomingMessage>(1024);
        let max_size = self.config.max_udp_size;

        // Spawn receiver task
        tokio::spawn(async move {
            let mut buf = vec![0u8; max_size];
            loop {
                match socket.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        match bincode::deserialize::<GossipMessage>(&buf[..len]) {
                            Ok(msg) => {
                                if tx
                                    .send(IncomingMessage {
                                        message: msg,
                                        source: src,
                                    })
                                    .await
                                    .is_err()
                                {
                                    tracing::debug!("Message channel closed, stopping receiver");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to deserialize message from {}: {}",
                                    src,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("UDP recv error: {}", e);
                    }
                }
            }
        });

        Ok(rx)
    }

    /// Send a gossip message to a specific peer via UDP.
    pub async fn send_udp(&self, msg: &GossipMessage, target: SocketAddr) -> Result<()> {
        let socket = self
            .udp_socket
            .as_ref()
            .context("Transport not started — call start() first")?;

        let data = bincode::serialize(msg).context("Failed to serialize gossip message")?;

        if data.len() > self.config.max_udp_size {
            anyhow::bail!(
                "Message too large for UDP: {} bytes (max {})",
                data.len(),
                self.config.max_udp_size
            );
        }

        socket
            .send_to(&data, target)
            .await
            .with_context(|| format!("Failed to send UDP to {}", target))?;

        Ok(())
    }

    /// Send a signed gossip message (signs with node identity before sending).
    pub async fn send_signed(
        &self,
        msg: &GossipMessage,
        target: SocketAddr,
        _identity: &NodeIdentity,
    ) -> Result<()> {
        // TODO: Add signature envelope around message
        // For now, send unsigned (signing will be added when we implement
        // a SignedEnvelope wrapper type)
        self.send_udp(msg, target).await
    }

    /// Get the local address the transport is bound to.
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.udp_socket
            .as_ref()
            .and_then(|s| s.local_addr().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_transport_send_receive() {
        // Bind two transports on random ports
        let mut transport1 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });
        let mut transport2 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });

        let _rx1 = transport1.start().await.unwrap();
        let mut rx2 = transport2.start().await.unwrap();

        let addr2 = transport2.local_addr().unwrap();

        // Send a ping from transport1 to transport2
        let ping = GossipMessage::Ping {
            sender: Uuid::new_v4(),
            seq: 42,
        };

        transport1.send_udp(&ping, addr2).await.unwrap();

        // Receive on transport2
        let incoming = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            rx2.recv(),
        )
        .await
        .expect("Timeout waiting for message")
        .expect("Channel closed");

        match incoming.message {
            GossipMessage::Ping { seq, .. } => assert_eq!(seq, 42),
            other => panic!("Expected Ping, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_transport_ping_pong() {
        let mut transport1 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });
        let mut transport2 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });

        let mut rx1 = transport1.start().await.unwrap();
        let mut rx2 = transport2.start().await.unwrap();

        let addr1 = transport1.local_addr().unwrap();
        let addr2 = transport2.local_addr().unwrap();

        let sender_id = Uuid::new_v4();

        // Node 1 sends Ping to Node 2
        transport1
            .send_udp(
                &GossipMessage::Ping {
                    sender: sender_id,
                    seq: 1,
                },
                addr2,
            )
            .await
            .unwrap();

        // Node 2 receives Ping
        let incoming = tokio::time::timeout(std::time::Duration::from_secs(2), rx2.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(incoming.message, GossipMessage::Ping { seq: 1, .. }));

        // Node 2 sends PingAck back to Node 1
        let responder_id = Uuid::new_v4();
        transport2
            .send_udp(
                &GossipMessage::PingAck {
                    sender: responder_id,
                    seq: 1,
                },
                addr1,
            )
            .await
            .unwrap();

        // Node 1 receives PingAck
        let ack = tokio::time::timeout(std::time::Duration::from_secs(2), rx1.recv())
            .await
            .unwrap()
            .unwrap();

        match ack.message {
            GossipMessage::PingAck { seq, sender } => {
                assert_eq!(seq, 1);
                assert_eq!(sender, responder_id);
            }
            other => panic!("Expected PingAck, got {:?}", other),
        }
    }
}
