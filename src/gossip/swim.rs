use std::collections::HashMap;
use std::net::SocketAddr;

use uuid::Uuid;

use super::membership::MembershipList;
use super::transport::Transport;
use super::{GossipConfig, GossipMessage, PeerInfo, PeerState};

/// Controls the SWIM protocol: periodic pings, failure detection,
/// join/leave handling, and membership synchronization.
pub struct SwimController {
    /// This node's ID.
    node_id: Uuid,
    /// This node's address (for announcing to peers).
    local_addr: SocketAddr,
    /// Ping sequence counter.
    seq: u64,
    /// Outstanding pings waiting for acks: seq -> (target_id, deadline).
    pending_pings: HashMap<u64, PendingPing>,
    /// Configuration.
    config: GossipConfig,
}

struct PendingPing {
    target_id: Uuid,
    target_addr: SocketAddr,
    sent_at: tokio::time::Instant,
}

impl SwimController {
    pub fn new(node_id: Uuid, local_addr: SocketAddr, config: GossipConfig) -> Self {
        Self {
            node_id,
            local_addr,
            seq: 0,
            pending_pings: HashMap::new(),
            config,
        }
    }

    /// Run one SWIM protocol tick:
    /// 1. Check for timed-out pings → mark peers suspected
    /// 2. Check for suspected peers past suspicion timeout → confirm dead
    /// 3. Select a random alive peer and send Ping
    pub async fn tick(
        &mut self,
        membership: &mut MembershipList,
        transport: &Transport,
    ) {
        // 1. Check for timed-out pings
        let now = tokio::time::Instant::now();
        let failure_timeout = self.config.failure_timeout;
        let timed_out: Vec<u64> = self
            .pending_pings
            .iter()
            .filter(|(_, p)| now.duration_since(p.sent_at) > failure_timeout)
            .map(|(&seq, _)| seq)
            .collect();

        for seq in timed_out {
            if let Some(ping) = self.pending_pings.remove(&seq) {
                // Try indirect ping before marking suspected
                let delegates = membership.select_ping_req_targets(&ping.target_id);
                if delegates.is_empty() {
                    // No delegates available — mark suspected directly
                    membership.mark_suspected(&ping.target_id);
                } else {
                    // Send PingReq to delegates
                    for delegate in &delegates {
                        let msg = GossipMessage::PingReq {
                            sender: self.node_id,
                            target: ping.target_id,
                            target_addr: ping.target_addr,
                            seq: self.next_seq(),
                        };
                        if let Err(e) = transport.send_udp(&msg, delegate.addr).await {
                            tracing::warn!("Failed to send PingReq to {}: {}", delegate.id, e);
                        }
                    }
                    // Mark suspected — if PingReq succeeds, peer will be marked alive again
                    membership.mark_suspected(&ping.target_id);
                }
            }
        }

        // 2. Check for suspected peers past suspicion timeout
        let dead_peers = membership.get_suspected_timeout_peers();
        for peer_id in dead_peers {
            membership.confirm_dead(&peer_id);
        }

        // 3. Select a random alive peer and ping it
        let targets = membership.select_gossip_targets();
        if let Some(target) = targets.first() {
            let seq = self.next_seq();
            let msg = GossipMessage::Ping {
                sender: self.node_id,
                seq,
            };

            if let Err(e) = transport.send_udp(&msg, target.addr).await {
                tracing::warn!("Failed to send Ping to {}: {}", target.id, e);
            } else {
                self.pending_pings.insert(seq, PendingPing {
                    target_id: target.id,
                    target_addr: target.addr,
                    sent_at: now,
                });
            }
        }
    }

    /// Handle an incoming gossip message related to SWIM protocol.
    /// Returns true if the message was handled (caller should not process further).
    pub async fn handle_message(
        &mut self,
        msg: &GossipMessage,
        source: SocketAddr,
        membership: &mut MembershipList,
        transport: &Transport,
    ) -> bool {
        match msg {
            GossipMessage::Ping { sender, seq } => {
                // Register the sender as a peer if unknown
                membership.add_peer(*sender, source);
                membership.mark_alive(sender);

                // Reply with PingAck
                let ack = GossipMessage::PingAck {
                    sender: self.node_id,
                    seq: *seq,
                };
                if let Err(e) = transport.send_udp(&ack, source).await {
                    tracing::warn!("Failed to send PingAck to {}: {}", sender, e);
                }
                true
            }

            GossipMessage::PingAck { sender, seq } => {
                // Clear the pending ping and mark peer alive
                self.pending_pings.remove(seq);
                membership.mark_alive(sender);
                true
            }

            GossipMessage::PingReq { sender, target, target_addr, seq } => {
                // Forward a ping to the target on behalf of the requester
                let forward_seq = self.next_seq();
                let ping = GossipMessage::Ping {
                    sender: self.node_id,
                    seq: forward_seq,
                };
                if let Err(e) = transport.send_udp(&ping, *target_addr).await {
                    tracing::warn!("Failed to forward Ping to {}: {}", target, e);
                }
                // We don't track the forwarded ping — if the target responds,
                // the original sender will also be notified via membership sync
                let _ = sender;
                let _ = seq;
                true
            }

            GossipMessage::Join { node_id, addr } => {
                tracing::info!("Peer {} joining from {}", node_id, addr);
                membership.add_peer(*node_id, *addr);

                // Respond with our membership list
                let peers = membership.alive_peers();
                // Include ourselves
                let mut all_peers = vec![PeerInfo {
                    id: self.node_id,
                    addr: self.local_addr,
                    state: PeerState::Alive,
                    last_seen: chrono::Utc::now().timestamp() as u64,
                }];
                all_peers.extend(peers);

                let sync_msg = GossipMessage::MembershipSync {
                    sender: self.node_id,
                    peers: all_peers,
                };
                if let Err(e) = transport.send_udp(&sync_msg, *addr).await {
                    tracing::warn!("Failed to send MembershipSync to {}: {}", node_id, e);
                }
                true
            }

            GossipMessage::Leave { node_id } => {
                tracing::info!("Peer {} leaving voluntarily", node_id);
                membership.remove_peer(node_id);
                true
            }

            GossipMessage::MembershipSync { sender, peers } => {
                membership.mark_alive(sender);
                for peer in peers {
                    if peer.state == PeerState::Alive {
                        membership.add_peer(peer.id, peer.addr);
                    }
                }
                tracing::debug!(
                    "Received MembershipSync from {} with {} peers (now know {})",
                    sender,
                    peers.len(),
                    membership.len()
                );
                true
            }

            // Not a SWIM message — let other handlers process it
            _ => false,
        }
    }

    /// Bootstrap: send Join to all seed nodes and wait for MembershipSync responses.
    pub async fn bootstrap(
        &mut self,
        seed_nodes: &[SocketAddr],
        transport: &Transport,
    ) {
        if seed_nodes.is_empty() {
            tracing::info!("No seed nodes configured — starting as first node");
            return;
        }

        tracing::info!("Bootstrapping with {} seed nodes", seed_nodes.len());

        for seed_addr in seed_nodes {
            let join_msg = GossipMessage::Join {
                node_id: self.node_id,
                addr: self.local_addr,
            };

            if let Err(e) = transport.send_udp(&join_msg, *seed_addr).await {
                tracing::warn!("Failed to send Join to seed {}: {}", seed_addr, e);
            } else {
                tracing::info!("Sent Join to seed {}", seed_addr);
            }
        }
    }

    /// Announce departure to all known alive peers.
    pub async fn leave(
        &self,
        membership: &MembershipList,
        transport: &Transport,
    ) {
        let leave_msg = GossipMessage::Leave {
            node_id: self.node_id,
        };

        for peer in membership.alive_peers() {
            if let Err(e) = transport.send_udp(&leave_msg, peer.addr).await {
                tracing::warn!("Failed to send Leave to {}: {}", peer.id, e);
            }
        }

        tracing::info!("Sent Leave to {} peers", membership.alive_count());
    }

    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use crate::gossip::transport::TransportConfig;

    #[tokio::test]
    async fn test_swim_ping_ack_lifecycle() {
        // Set up two nodes with transports
        let node1_id = Uuid::new_v4();
        let node2_id = Uuid::new_v4();

        let mut t1 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });
        let mut t2 = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });

        let _rx1 = t1.start().await.unwrap();
        let mut rx2 = t2.start().await.unwrap();

        let addr1 = t1.local_addr().unwrap();
        let addr2 = t2.local_addr().unwrap();

        let config = GossipConfig {
            bind_addr: addr1,
            failure_timeout: Duration::from_secs(1),
            suspicion_timeout: Duration::from_secs(2),
            fanout: 1,
            ..Default::default()
        };

        let mut swim1 = SwimController::new(node1_id, addr1, config.clone());
        let mut membership1 = MembershipList::new(node1_id, config.clone());
        let mut membership2 = MembershipList::new(node2_id, config);

        // Node1 knows about Node2
        membership1.add_peer(node2_id, addr2);
        assert_eq!(membership1.alive_count(), 1);

        // Node1 does a SWIM tick — should send Ping to Node2
        swim1.tick(&mut membership1, &t1).await;

        // Node2 receives the Ping
        let incoming = tokio::time::timeout(Duration::from_secs(2), rx2.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(incoming.message, GossipMessage::Ping { .. }));

        // Node2's SWIM controller handles the Ping → sends PingAck
        let mut swim2 = SwimController::new(node2_id, addr2, GossipConfig {
            bind_addr: addr2,
            ..Default::default()
        });
        swim2
            .handle_message(&incoming.message, incoming.source, &mut membership2, &t2)
            .await;

        // Node2 should now know about Node1
        assert_eq!(membership2.alive_count(), 1);
    }

    #[tokio::test]
    async fn test_swim_join_bootstrap() {
        let seed_id = Uuid::new_v4();
        let joiner_id = Uuid::new_v4();

        let mut t_seed = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });
        let mut t_joiner = Transport::new(TransportConfig {
            udp_addr: "127.0.0.1:0".parse().unwrap(),
            max_udp_size: 65507,
        });

        let mut rx_seed = t_seed.start().await.unwrap();
        let mut rx_joiner = t_joiner.start().await.unwrap();

        let seed_addr = t_seed.local_addr().unwrap();
        let joiner_addr = t_joiner.local_addr().unwrap();

        let config = GossipConfig {
            bind_addr: seed_addr,
            ..Default::default()
        };

        let mut swim_seed = SwimController::new(seed_id, seed_addr, config.clone());
        let mut swim_joiner = SwimController::new(joiner_id, joiner_addr, GossipConfig {
            bind_addr: joiner_addr,
            seed_nodes: vec![seed_addr],
            ..Default::default()
        });

        let mut membership_seed = MembershipList::new(seed_id, config);
        let mut membership_joiner = MembershipList::new(joiner_id, GossipConfig {
            bind_addr: joiner_addr,
            ..Default::default()
        });

        // Joiner bootstraps — sends Join to seed
        swim_joiner.bootstrap(&[seed_addr], &t_joiner).await;

        // Seed receives Join
        let join_msg = tokio::time::timeout(Duration::from_secs(2), rx_seed.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(join_msg.message, GossipMessage::Join { .. }));

        // Seed handles Join → adds joiner, sends MembershipSync
        swim_seed
            .handle_message(&join_msg.message, join_msg.source, &mut membership_seed, &t_seed)
            .await;

        assert_eq!(membership_seed.alive_count(), 1); // seed now knows joiner

        // Joiner receives MembershipSync
        let sync_msg = tokio::time::timeout(Duration::from_secs(2), rx_joiner.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(sync_msg.message, GossipMessage::MembershipSync { .. }));

        // Joiner handles MembershipSync → learns about seed
        swim_joiner
            .handle_message(&sync_msg.message, sync_msg.source, &mut membership_joiner, &t_joiner)
            .await;

        assert_eq!(membership_joiner.alive_count(), 1); // joiner now knows seed
    }
}
