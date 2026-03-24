pub mod identity;

use anyhow::Result;
use uuid::Uuid;

use crate::crash::CrashDatabase;
use crate::fuzzer::corpus::CorpusManager;
use crate::fuzzer::coverage::CoverageBitmap;
use crate::fuzzer::{FuzzerBackend, TargetConfig};
use crate::gossip::membership::MembershipList;
use crate::gossip::GossipConfig;
use crate::strategy::MutationStrategy;

/// A HIVEFUZZ node — the core unit of the swarm.
///
/// Each node runs independently, making autonomous decisions about
/// what to fuzz and how. Nodes communicate via gossip protocol.
pub struct Node {
    /// Unique node identity.
    pub identity: identity::NodeIdentity,

    /// The fuzzer engine.
    fuzzer: Box<dyn FuzzerBackend>,

    /// Global coverage (union of local + received from swarm).
    global_coverage: CoverageBitmap,

    /// Corpus manager.
    corpus: CorpusManager,

    /// Crash database.
    crashes: CrashDatabase,

    /// Current mutation strategy.
    strategy: MutationStrategy,

    /// Swarm membership list.
    membership: MembershipList,

    /// Node state.
    state: NodeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeState {
    /// Node is initializing (loading target, connecting to swarm).
    Initializing,
    /// Node is actively fuzzing.
    Running,
    /// Node is shutting down gracefully.
    ShuttingDown,
    /// Node is stopped.
    Stopped,
}

impl Node {
    /// Create a new HIVEFUZZ node.
    pub fn new(fuzzer: Box<dyn FuzzerBackend>, gossip_config: GossipConfig) -> Self {
        let identity = identity::NodeIdentity::generate();
        let node_id = identity.id;

        Self {
            identity,
            fuzzer,
            global_coverage: CoverageBitmap::new(),
            corpus: CorpusManager::new(node_id, 10_000),
            crashes: CrashDatabase::new(),
            strategy: MutationStrategy::default_uniform(),
            membership: MembershipList::new(node_id, gossip_config),
            state: NodeState::Initializing,
        }
    }

    /// Initialize the node: set up fuzzer, join swarm.
    pub async fn init(&mut self, target: &TargetConfig) -> Result<()> {
        tracing::info!("Node {} initializing", self.identity.id);

        // Initialize fuzzer backend
        self.fuzzer.init(target)?;

        // TODO: Start gossip protocol
        // TODO: Bootstrap peer discovery from seed nodes
        // TODO: Announce ourselves to the swarm

        self.state = NodeState::Running;
        tracing::info!("Node {} ready", self.identity.id);
        Ok(())
    }

    /// Main fuzzing loop — runs until shutdown signal.
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Node {} starting fuzz loop", self.identity.id);

        // TODO: Implement main loop:
        // 1. Select input from corpus
        // 2. Apply mutation (selected by strategy)
        // 3. Execute input
        // 4. Process result (coverage, crashes)
        // 5. Periodically: gossip round, strategy evolution

        Ok(())
    }

    /// Graceful shutdown: announce departure to swarm.
    pub async fn shutdown(&mut self) -> Result<()> {
        self.state = NodeState::ShuttingDown;
        tracing::info!("Node {} shutting down", self.identity.id);

        // TODO: Broadcast Leave message to swarm
        // TODO: Flush crash database to disk

        self.state = NodeState::Stopped;
        Ok(())
    }

    pub fn id(&self) -> Uuid {
        self.identity.id
    }

    pub fn state(&self) -> NodeState {
        self.state
    }
}
