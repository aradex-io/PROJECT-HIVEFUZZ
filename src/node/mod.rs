pub mod identity;

use anyhow::Result;
use uuid::Uuid;

use crate::crash::CrashDatabase;
use crate::fuzzer::corpus::CorpusManager;
use crate::fuzzer::coverage::CoverageBitmap;
use crate::fuzzer::{FuzzerBackend, TargetConfig};
use crate::gossip::membership::MembershipList;
use crate::gossip::GossipConfig;
use crate::strategy::fitness::{Exp3Updater, FitnessTracker};
use crate::strategy::mutator::apply_mutation;
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

    /// Fitness tracker for strategy evolution.
    fitness_tracker: FitnessTracker,

    /// Exp3 updater for strategy weight evolution.
    exp3: Exp3Updater,

    /// Swarm membership list.
    membership: MembershipList,

    /// Node state.
    state: NodeState,

    /// Evolution interval (strategy update frequency in executions).
    evolution_interval: u64,
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
            fitness_tracker: FitnessTracker::new(100_000),
            exp3: Exp3Updater::new(0.1, 0.01),
            membership: MembershipList::new(node_id, gossip_config),
            state: NodeState::Initializing,
            evolution_interval: 100_000,
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

        // Set up shutdown signal handler
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        // Spawn a task to listen for Ctrl+C
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Received shutdown signal");
            let _ = shutdown_tx.send(());
        });

        let mut executions: u64 = 0;

        loop {
            // Check for shutdown
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            // 1. Select input from corpus (or use a seed)
            let input = self.select_input();

            // 2. Select and apply mutation
            let mutation_type = self.strategy.select_mutation();
            let mutated = apply_mutation(&input, mutation_type);

            // 3. Execute the mutated input
            let result = self.fuzzer.run_input(&mutated)?;

            // 4. Process coverage
            let new_edges = self.global_coverage.merge(&result.coverage);

            // 5. If novel coverage, add to corpus
            if new_edges > 0 {
                let mutation_name = format!("{:?}", mutation_type);
                self.corpus.add(
                    mutated.clone(),
                    new_edges,
                    Some(mutation_name),
                    None,
                );
                tracing::debug!(
                    "New edges: {} (total: {})",
                    new_edges,
                    self.global_coverage.count_edges()
                );
            }

            // 6. Process crashes
            if let Some(ref crash) = result.crash {
                let is_novel = self.crashes.record(crash.clone(), self.identity.id);
                if is_novel {
                    tracing::warn!(
                        "New crash found! severity={:?} signal={} hash={:#x}",
                        crash.severity,
                        crash.signal,
                        crash.stack_hash
                    );
                }
            }

            // 7. Update fitness tracker
            self.fitness_tracker.record(
                mutation_type,
                new_edges,
                result.crash.is_some(),
            );

            // 8. Periodic strategy evolution
            executions += 1;
            if executions % self.evolution_interval == 0 {
                self.evolve_strategy();
            }

            // 9. Periodic status logging
            if executions % 10_000 == 0 {
                let stats = self.fuzzer.stats();
                tracing::info!(
                    "exec={} speed={:.0}/s edges={} crashes={} corpus={}",
                    stats.total_executions,
                    stats.executions_per_sec,
                    stats.total_edges,
                    stats.total_crashes,
                    self.corpus.len()
                );
            }

            // TODO: Periodic gossip rounds
        }

        self.shutdown().await
    }

    /// Select an input from the corpus for mutation.
    fn select_input(&self) -> Vec<u8> {
        use rand::prelude::SliceRandom;

        // Collect corpus entries
        let entries: Vec<_> = self.corpus.entries().collect();

        if entries.is_empty() {
            // No corpus entries yet — generate a random seed
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let len = rng.r#gen_range(1..=64);
            (0..len).map(|_| rng.r#gen()).collect()
        } else {
            // Random selection from corpus (weighted selection is a future improvement)
            let mut rng = rand::thread_rng();
            let entry = entries.choose(&mut rng).unwrap();
            entry.data.clone()
        }
    }

    /// Evolve the mutation strategy using Exp3 bandit algorithm.
    fn evolve_strategy(&mut self) {
        let fitness = self.fitness_tracker.all_fitness();
        self.exp3.update_weights(&mut self.strategy.weights, &fitness);
        self.strategy.generation += 1;
        tracing::debug!(
            "Strategy evolved to generation {}",
            self.strategy.generation
        );
    }

    /// Graceful shutdown: announce departure to swarm.
    pub async fn shutdown(&mut self) -> Result<()> {
        self.state = NodeState::ShuttingDown;
        tracing::info!("Node {} shutting down", self.identity.id);

        // Log final statistics
        let stats = self.fuzzer.stats();
        let crash_summary = self.crashes.summary();
        tracing::info!(
            "Final stats: exec={} edges={} crashes={} (crit={} high={} med={} low={})",
            stats.total_executions,
            stats.total_edges,
            crash_summary.total,
            crash_summary.critical,
            crash_summary.high,
            crash_summary.medium,
            crash_summary.low,
        );

        // TODO: Broadcast Leave message to swarm
        // TODO: Flush crash database to disk

        self.state = NodeState::Stopped;
        Ok(())
    }

    /// Get the node's UUID.
    pub fn id(&self) -> Uuid {
        self.identity.id
    }

    /// Get the current node state.
    pub fn state(&self) -> NodeState {
        self.state
    }

    /// Get the membership list.
    pub fn membership(&self) -> &MembershipList {
        &self.membership
    }

    /// Get the crash database.
    pub fn crashes(&self) -> &CrashDatabase {
        &self.crashes
    }
}
