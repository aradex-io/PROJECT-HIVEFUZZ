pub mod identity;

use std::path::Path;

use anyhow::Result;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::crash::CrashDatabase;
use crate::fuzzer::corpus::CorpusManager;
use crate::fuzzer::coverage::CoverageBitmap;
use crate::fuzzer::{CrashInfo, FuzzerBackend, TargetConfig};
use crate::gossip::dissemination::Disseminator;
use crate::gossip::membership::MembershipList;
use crate::gossip::swim::SwimController;
use crate::gossip::transport::{IncomingMessage, Transport, TransportConfig};
use crate::gossip::{GossipConfig, GossipMessage};
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

    /// Gossip transport layer.
    transport: Transport,

    /// SWIM protocol controller.
    swim: SwimController,

    /// Coverage/crash disseminator.
    disseminator: Disseminator,

    /// Gossip configuration.
    gossip_config: GossipConfig,

    /// Incoming message receiver from transport (set during init).
    msg_rx: Option<mpsc::Receiver<IncomingMessage>>,

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

        let transport = Transport::new(TransportConfig {
            udp_addr: gossip_config.bind_addr,
            max_udp_size: 65507,
        });

        let swim = SwimController::new(
            node_id,
            gossip_config.bind_addr,
            gossip_config.clone(),
        );

        let disseminator = Disseminator::new(
            node_id,
            gossip_config.max_piggyback_size,
        );

        Self {
            identity,
            fuzzer,
            global_coverage: CoverageBitmap::new(),
            corpus: CorpusManager::new(node_id, 10_000),
            crashes: CrashDatabase::new(),
            strategy: MutationStrategy::default_uniform(),
            fitness_tracker: FitnessTracker::new(100_000),
            exp3: Exp3Updater::new(0.1, 0.01),
            membership: MembershipList::new(node_id, gossip_config.clone()),
            transport,
            swim,
            disseminator,
            gossip_config,
            msg_rx: None,
            state: NodeState::Initializing,
            evolution_interval: 100_000,
        }
    }

    /// Load seed corpus files from a directory.
    pub fn load_seeds(&mut self, seeds_dir: &Path) -> Result<usize> {
        const MAX_SEED_SIZE: u64 = 1_048_576; // 1MB
        self.corpus.load_seeds(seeds_dir, MAX_SEED_SIZE)
    }

    /// Initialize the node: set up fuzzer, start transport, join swarm.
    pub async fn init(&mut self, target: &TargetConfig) -> Result<()> {
        tracing::info!("Node {} initializing", self.identity.id);

        // Initialize fuzzer backend
        self.fuzzer.init(target)?;

        // Start gossip transport
        let mut msg_rx = self.transport.start().await?;

        // Update SWIM controller with the actual bound address
        if let Some(actual_addr) = self.transport.local_addr() {
            self.swim = SwimController::new(
                self.identity.id,
                actual_addr,
                self.gossip_config.clone(),
            );
            tracing::info!("Gossip transport bound to {}", actual_addr);
        }

        // Bootstrap: join the swarm via seed nodes
        self.swim
            .bootstrap(&self.gossip_config.seed_nodes, &self.transport)
            .await;

        // Process any MembershipSync responses received during bootstrap
        self.drain_messages(&mut msg_rx, std::time::Duration::from_millis(500))
            .await;

        // Save the receiver for the run loop
        self.msg_rx = Some(msg_rx);

        self.state = NodeState::Running;
        tracing::info!(
            "Node {} ready (peers: {})",
            self.identity.id,
            self.membership.alive_count()
        );
        Ok(())
    }

    /// Drain incoming messages for a given duration (used during bootstrap).
    async fn drain_messages(
        &mut self,
        rx: &mut mpsc::Receiver<IncomingMessage>,
        duration: std::time::Duration,
    ) {
        let deadline = tokio::time::Instant::now() + duration;
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(incoming)) => {
                    self.handle_incoming_message(incoming).await;
                }
                _ => break,
            }
        }
    }

    /// Handle a single incoming gossip message.
    async fn handle_incoming_message(&mut self, incoming: IncomingMessage) {
        let source = incoming.source;

        // Let SWIM handle protocol messages first
        let handled = self
            .swim
            .handle_message(
                &incoming.message,
                source,
                &mut self.membership,
                &self.transport,
            )
            .await;

        if handled {
            return;
        }

        // Handle data messages
        match incoming.message {
            GossipMessage::CoverageDigest { sender, digest, total_edges } => {
                tracing::debug!(
                    "Received CoverageDigest from {} ({} edges)",
                    sender,
                    total_edges
                );
                // TODO: Compare and respond with CoverageUpdate if we have novel edges
                let _ = digest;
            }

            GossipMessage::CoverageUpdate { sender, corpus_entries, .. } => {
                tracing::debug!(
                    "Received CoverageUpdate from {} ({} corpus entries)",
                    sender,
                    corpus_entries.len()
                );
                // Import corpus entries
                for entry_data in &corpus_entries {
                    match bincode::deserialize::<crate::fuzzer::corpus::CorpusEntry>(entry_data) {
                        Ok(entry) => {
                            self.corpus.import(entry);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to deserialize corpus entry: {}", e);
                        }
                    }
                }
            }

            GossipMessage::CrashAlert { sender, stack_hash, crash_data } => {
                tracing::info!(
                    "Received CrashAlert from {} (hash={:#x})",
                    sender,
                    stack_hash
                );
                match bincode::deserialize::<CrashInfo>(&crash_data) {
                    Ok(crash) => {
                        let is_novel = self.crashes.record(crash, sender);
                        if is_novel {
                            tracing::warn!("Novel crash received from swarm: hash={:#x}", stack_hash);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to deserialize crash data: {}", e);
                    }
                }
            }

            GossipMessage::StrategyUpdate { sender, strategy_data, fitness_score } => {
                tracing::debug!(
                    "Received StrategyUpdate from {} (fitness={:.4})",
                    sender,
                    fitness_score
                );
                // TODO: Blend strategy if fitness is significantly higher than ours
                let _ = strategy_data;
            }

            // SWIM messages already handled above
            _ => {}
        }
    }

    /// Main fuzzing loop — runs until shutdown signal.
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Node {} starting fuzz loop", self.identity.id);

        let mut msg_rx = self.msg_rx.take()
            .expect("Node::init() must be called before run()");

        // Set up shutdown signal handler
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("Received shutdown signal");
            let _ = shutdown_tx.send(());
        });

        let mut executions: u64 = 0;
        let gossip_interval = self.gossip_config.gossip_interval;
        let mut gossip_ticker = tokio::time::interval(gossip_interval);
        gossip_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Shutdown signal
                _ = &mut shutdown_rx => {
                    break;
                }

                // Incoming gossip messages (non-blocking drain)
                Some(incoming) = msg_rx.recv() => {
                    self.handle_incoming_message(incoming).await;
                }

                // Periodic gossip tick
                _ = gossip_ticker.tick() => {
                    // SWIM protocol tick (ping/failure detection)
                    self.swim.tick(&mut self.membership, &self.transport).await;

                    // Coverage/crash dissemination
                    self.disseminator.gossip_round(
                        &self.membership,
                        &self.global_coverage,
                        &self.corpus,
                        &self.crashes,
                        &self.transport,
                    ).await;
                }

                // Default: do a fuzz iteration
                else => {
                    break;
                }
            }

            // Do one fuzz iteration between async events
            self.fuzz_one_iteration(&mut executions)?;
        }

        // If we got here via shutdown signal, do some final fuzz iterations
        // to drain, then shut down
        self.shutdown().await
    }

    /// Execute a single fuzzing iteration.
    fn fuzz_one_iteration(&mut self, executions: &mut u64) -> Result<()> {
        // 1. Select input from corpus
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
            self.corpus.add(mutated, new_edges, Some(mutation_name), None);
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
        self.fitness_tracker.record(mutation_type, new_edges, result.crash.is_some());

        // 8. Periodic strategy evolution
        *executions += 1;
        if *executions % self.evolution_interval == 0 {
            self.evolve_strategy();
        }

        // 9. Periodic status logging
        if *executions % 10_000 == 0 {
            let stats = self.fuzzer.stats();
            tracing::info!(
                "exec={} speed={:.0}/s edges={} crashes={} corpus={} peers={}",
                stats.total_executions,
                stats.executions_per_sec,
                stats.total_edges,
                stats.total_crashes,
                self.corpus.len(),
                self.membership.alive_count()
            );
        }

        Ok(())
    }

    /// Select an input from the corpus for mutation.
    fn select_input(&self) -> Vec<u8> {
        use rand::prelude::SliceRandom;

        let entries: Vec<_> = self.corpus.entries().collect();

        if entries.is_empty() {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let len = rng.r#gen_range(1..=64);
            (0..len).map(|_| rng.r#gen()).collect()
        } else {
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

        // Broadcast Leave to all peers
        self.swim.leave(&self.membership, &self.transport).await;

        // Log final statistics
        let stats = self.fuzzer.stats();
        let crash_summary = self.crashes.summary();
        tracing::info!(
            "Final stats: exec={} edges={} crashes={} (crit={} high={} med={} low={}) peers={}",
            stats.total_executions,
            stats.total_edges,
            crash_summary.total,
            crash_summary.critical,
            crash_summary.high,
            crash_summary.medium,
            crash_summary.low,
            self.membership.alive_count(),
        );

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
