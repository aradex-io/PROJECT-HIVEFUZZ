use uuid::Uuid;

use super::membership::MembershipList;
use super::transport::Transport;
use super::GossipMessage;
use crate::crash::CrashDatabase;
use crate::fuzzer::corpus::CorpusManager;
use crate::fuzzer::coverage::{BloomDigest, CoverageBitmap};

/// Manages the dissemination of coverage, corpus entries, and crashes
/// across the swarm via the gossip protocol.
pub struct Disseminator {
    /// This node's ID.
    node_id: Uuid,
    /// Sequence counter for gossip rounds.
    round: u64,
    /// Maximum size of corpus entries to piggyback on gossip messages.
    max_piggyback_size: usize,
}

impl Disseminator {
    pub fn new(node_id: Uuid, max_piggyback_size: usize) -> Self {
        Self {
            node_id,
            round: 0,
            max_piggyback_size,
        }
    }

    /// Execute one gossip round:
    /// 1. Select random peers (fanout)
    /// 2. Send coverage digest to each
    /// 3. Piggyback pending corpus entries
    /// 4. Propagate any crash alerts
    pub async fn gossip_round(
        &mut self,
        membership: &MembershipList,
        local_coverage: &CoverageBitmap,
        corpus: &CorpusManager,
        crashes: &CrashDatabase,
        transport: &Transport,
    ) {
        self.round += 1;
        tracing::debug!(
            "Gossip round {} starting (node {})",
            self.round,
            self.node_id
        );

        let targets = membership.select_gossip_targets();
        if targets.is_empty() {
            tracing::trace!("No gossip targets available");
            return;
        }

        let digest = local_coverage.to_bloom_digest();
        let total_edges = local_coverage.count_edges();

        // Send coverage digest to each target
        for target in &targets {
            let msg = GossipMessage::CoverageDigest {
                sender: self.node_id,
                digest: digest.as_bytes().to_vec(),
                total_edges,
            };

            if let Err(e) = transport.send_udp(&msg, target.addr).await {
                tracing::warn!(
                    "Failed to send CoverageDigest to {}: {}",
                    target.id,
                    e
                );
            }
        }

        // Piggyback pending corpus entries (under size limit)
        let pending = corpus.pending_dissemination(5);
        for entry in &pending {
            if entry.data.len() <= self.max_piggyback_size {
                let serialized = match bincode::serialize(entry) {
                    Ok(data) => data,
                    Err(e) => {
                        tracing::warn!("Failed to serialize corpus entry: {}", e);
                        continue;
                    }
                };

                for target in &targets {
                    let msg = GossipMessage::CoverageUpdate {
                        sender: self.node_id,
                        novel_coverage: vec![], // simplified — full bitmap transfer deferred
                        corpus_entries: vec![serialized.clone()],
                    };

                    if let Err(e) = transport.send_udp(&msg, target.addr).await {
                        tracing::warn!(
                            "Failed to send corpus entry to {}: {}",
                            target.id,
                            e
                        );
                    }
                }
            }
        }

        // Propagate crash alerts (high priority)
        let pending_crashes = crashes.pending_dissemination();
        for crash_record in &pending_crashes {
            let crash_data = match bincode::serialize(&crash_record.crash_info) {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!("Failed to serialize crash: {}", e);
                    continue;
                }
            };

            for target in &targets {
                let msg = GossipMessage::CrashAlert {
                    sender: self.node_id,
                    stack_hash: crash_record.stack_hash,
                    crash_data: crash_data.clone(),
                };

                if let Err(e) = transport.send_udp(&msg, target.addr).await {
                    tracing::warn!(
                        "Failed to send CrashAlert to {}: {}",
                        target.id,
                        e
                    );
                }
            }
        }
    }

    /// Handle a received CoverageDigest — compare with local coverage
    /// and determine if we have novel edges the sender lacks.
    pub fn should_send_update(
        &self,
        _sender: Uuid,
        remote_digest: &BloomDigest,
        local_digest: &BloomDigest,
    ) -> bool {
        // If our local digest has bits not in the remote digest,
        // we likely have coverage the remote node doesn't.
        local_digest.likely_has_novel(remote_digest)
    }

    /// Current gossip round number.
    pub fn round(&self) -> u64 {
        self.round
    }
}
