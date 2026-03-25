pub mod dissemination;
pub mod membership;
pub mod swim;
pub mod transport;

use std::net::SocketAddr;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for the gossip protocol layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipConfig {
    /// Address to bind the gossip listener.
    pub bind_addr: SocketAddr,
    /// Seed nodes for initial swarm discovery.
    pub seed_nodes: Vec<SocketAddr>,
    /// Interval between gossip rounds.
    pub gossip_interval: Duration,
    /// Number of peers to gossip with each round.
    pub fanout: usize,
    /// Timeout before marking a peer as suspected failed.
    pub failure_timeout: Duration,
    /// Timeout before confirming a suspected peer as dead.
    pub suspicion_timeout: Duration,
    /// Maximum corpus entry size to piggyback on gossip messages.
    pub max_piggyback_size: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:7878".parse().unwrap(),
            seed_nodes: Vec::new(),
            gossip_interval: Duration::from_secs(5),
            fanout: 3,
            failure_timeout: Duration::from_secs(15),
            suspicion_timeout: Duration::from_secs(30),
            max_piggyback_size: 4096,
        }
    }
}

/// Messages exchanged between nodes via the gossip protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GossipMessage {
    /// Ping for failure detection (SWIM protocol).
    Ping {
        sender: Uuid,
        seq: u64,
    },

    /// Acknowledgment of a ping.
    PingAck {
        sender: Uuid,
        seq: u64,
    },

    /// Indirect ping request (ask another node to ping on our behalf).
    PingReq {
        sender: Uuid,
        target: Uuid,
        target_addr: SocketAddr,
        seq: u64,
    },

    /// Coverage digest for comparison.
    CoverageDigest {
        sender: Uuid,
        /// Bloom filter of known edge coverage.
        digest: Vec<u8>,
        /// Total edges known.
        total_edges: u32,
    },

    /// Coverage update with novel edges and the inputs that found them.
    CoverageUpdate {
        sender: Uuid,
        /// Compressed coverage bitmap of novel edges.
        novel_coverage: Vec<u8>,
        /// Corpus entries that found these edges.
        corpus_entries: Vec<Vec<u8>>,
    },

    /// Announce a new crash (high-priority dissemination).
    CrashAlert {
        sender: Uuid,
        /// Crash fingerprint for deduplication.
        stack_hash: u64,
        /// Serialized crash info.
        crash_data: Vec<u8>,
    },

    /// Share mutation strategy fitness scores.
    StrategyUpdate {
        sender: Uuid,
        /// Serialized strategy with fitness data.
        strategy_data: Vec<u8>,
        /// Overall fitness score.
        fitness_score: f64,
    },

    /// New node announcing itself to the swarm.
    Join {
        node_id: Uuid,
        addr: SocketAddr,
    },

    /// Node voluntarily leaving the swarm.
    Leave {
        node_id: Uuid,
    },

    /// Membership list exchange.
    MembershipSync {
        sender: Uuid,
        /// Partial list of known peers.
        peers: Vec<PeerInfo>,
    },
}

/// Information about a known peer in the swarm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: Uuid,
    pub addr: SocketAddr,
    pub state: PeerState,
    pub last_seen: u64, // unix timestamp
}

/// State of a peer as observed by this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerState {
    Alive,
    Suspected,
    Dead,
}
