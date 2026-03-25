use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use uuid::Uuid;

use super::{GossipConfig, PeerInfo, PeerState};

/// Manages the membership list of the swarm.
///
/// Implements SWIM-style failure detection:
/// - Periodic pings to random peers
/// - Indirect pings via third-party peers
/// - Suspicion mechanism before declaring a peer dead
pub struct MembershipList {
    /// Our node ID.
    node_id: Uuid,
    /// Known peers.
    peers: HashMap<Uuid, PeerEntry>,
    /// Gossip configuration.
    config: GossipConfig,
}

struct PeerEntry {
    info: PeerInfo,
    last_ping_sent: Option<Instant>,
    last_pong_received: Option<Instant>,
    suspected_at: Option<Instant>,
}

impl MembershipList {
    pub fn new(node_id: Uuid, config: GossipConfig) -> Self {
        Self {
            node_id,
            peers: HashMap::new(),
            config,
        }
    }

    /// Register a new peer (from join announcement or membership sync).
    pub fn add_peer(&mut self, id: Uuid, addr: SocketAddr) {
        if id == self.node_id {
            return; // don't add ourselves
        }

        self.peers.entry(id).or_insert_with(|| PeerEntry {
            info: PeerInfo {
                id,
                addr,
                state: PeerState::Alive,
                last_seen: chrono::Utc::now().timestamp() as u64,
            },
            last_ping_sent: None,
            last_pong_received: None,
            suspected_at: None,
        });
    }

    /// Mark a peer as having responded (alive).
    pub fn mark_alive(&mut self, id: &Uuid) {
        if let Some(peer) = self.peers.get_mut(id) {
            peer.info.state = PeerState::Alive;
            peer.info.last_seen = chrono::Utc::now().timestamp() as u64;
            peer.last_pong_received = Some(Instant::now());
            peer.suspected_at = None;
        }
    }

    /// Mark a peer as suspected (missed ping response).
    pub fn mark_suspected(&mut self, id: &Uuid) {
        if let Some(peer) = self.peers.get_mut(id) {
            if peer.info.state == PeerState::Alive {
                peer.info.state = PeerState::Suspected;
                peer.suspected_at = Some(Instant::now());
                tracing::warn!("Peer {} suspected failed", id);
            }
        }
    }

    /// Confirm a suspected peer as dead (suspicion timeout elapsed).
    pub fn confirm_dead(&mut self, id: &Uuid) {
        if let Some(peer) = self.peers.get_mut(id) {
            if peer.info.state == PeerState::Suspected {
                peer.info.state = PeerState::Dead;
                tracing::warn!("Peer {} confirmed dead", id);
            }
        }
    }

    /// Remove a peer (voluntary leave or confirmed dead).
    pub fn remove_peer(&mut self, id: &Uuid) {
        self.peers.remove(id);
    }

    /// Select random alive peers for gossip (fanout selection).
    pub fn select_gossip_targets(&self) -> Vec<PeerInfo> {
        use rand::prelude::SliceRandom;

        let alive: Vec<_> = self
            .peers
            .values()
            .filter(|p| p.info.state == PeerState::Alive)
            .map(|p| p.info.clone())
            .collect();

        let mut rng = rand::thread_rng();
        let mut selected = alive;
        selected.shuffle(&mut rng);
        selected.truncate(self.config.fanout);
        selected
    }

    /// Get peers that need failure detection checks.
    pub fn get_suspected_timeout_peers(&self) -> Vec<Uuid> {
        let now = Instant::now();
        self.peers
            .values()
            .filter(|p| {
                p.info.state == PeerState::Suspected
                    && p.suspected_at
                        .is_some_and(|t| now.duration_since(t) > self.config.suspicion_timeout)
            })
            .map(|p| p.info.id)
            .collect()
    }

    /// Get a snapshot of all known alive peers for membership sync.
    pub fn alive_peers(&self) -> Vec<PeerInfo> {
        self.peers
            .values()
            .filter(|p| p.info.state == PeerState::Alive)
            .map(|p| p.info.clone())
            .collect()
    }

    /// Total number of known peers (all states).
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Select alive peers for indirect ping (excluding the target).
    /// Used for PingReq: ask other peers to ping a suspected peer on our behalf.
    pub fn select_ping_req_targets(&self, exclude: &Uuid) -> Vec<PeerInfo> {
        use rand::prelude::SliceRandom;

        let alive: Vec<_> = self
            .peers
            .values()
            .filter(|p| p.info.state == PeerState::Alive && p.info.id != *exclude)
            .map(|p| p.info.clone())
            .collect();

        let mut rng = rand::thread_rng();
        let mut selected = alive;
        selected.shuffle(&mut rng);
        // Use fewer delegates than fanout — typically 1-2
        selected.truncate(self.config.fanout.min(2));
        selected
    }

    /// Get the address of a specific peer.
    pub fn peer_addr(&self, id: &Uuid) -> Option<SocketAddr> {
        self.peers.get(id).map(|p| p.info.addr)
    }

    /// Get the gossip config.
    pub fn config(&self) -> &GossipConfig {
        &self.config
    }

    /// Number of alive peers.
    pub fn alive_count(&self) -> usize {
        self.peers
            .values()
            .filter(|p| p.info.state == PeerState::Alive)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_select_peers() {
        let node_id = Uuid::new_v4();
        let config = GossipConfig {
            fanout: 2,
            ..Default::default()
        };
        let mut membership = MembershipList::new(node_id, config);

        let peer1 = Uuid::new_v4();
        let peer2 = Uuid::new_v4();
        let peer3 = Uuid::new_v4();

        membership.add_peer(peer1, "127.0.0.1:7001".parse().unwrap());
        membership.add_peer(peer2, "127.0.0.1:7002".parse().unwrap());
        membership.add_peer(peer3, "127.0.0.1:7003".parse().unwrap());

        assert_eq!(membership.len(), 3);
        assert_eq!(membership.alive_count(), 3);

        let targets = membership.select_gossip_targets();
        assert_eq!(targets.len(), 2); // fanout = 2
    }

    #[test]
    fn test_failure_detection_lifecycle() {
        let node_id = Uuid::new_v4();
        let config = GossipConfig::default();
        let mut membership = MembershipList::new(node_id, config);

        let peer = Uuid::new_v4();
        membership.add_peer(peer, "127.0.0.1:7001".parse().unwrap());

        assert_eq!(membership.alive_count(), 1);

        membership.mark_suspected(&peer);
        assert_eq!(membership.alive_count(), 0);

        // Peer recovers
        membership.mark_alive(&peer);
        assert_eq!(membership.alive_count(), 1);
    }

    #[test]
    fn test_dont_add_self() {
        let node_id = Uuid::new_v4();
        let config = GossipConfig::default();
        let mut membership = MembershipList::new(node_id, config);

        membership.add_peer(node_id, "127.0.0.1:7001".parse().unwrap());
        assert_eq!(membership.len(), 0);
    }
}
