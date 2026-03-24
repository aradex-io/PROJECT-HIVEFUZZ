use uuid::Uuid;

use super::membership::MembershipList;
use crate::fuzzer::coverage::BloomDigest;

/// Manages the dissemination of coverage, corpus entries, and crashes
/// across the swarm via the gossip protocol.
pub struct Disseminator {
    /// This node's ID.
    node_id: Uuid,
    /// Sequence counter for gossip rounds.
    round: u64,
}

impl Disseminator {
    pub fn new(node_id: Uuid) -> Self {
        Self { node_id, round: 0 }
    }

    /// Execute one gossip round:
    /// 1. Select random peers (fanout)
    /// 2. Send coverage digest to each
    /// 3. Piggyback pending corpus entries
    /// 4. Propagate any crash alerts
    pub async fn gossip_round(
        &mut self,
        _membership: &MembershipList,
        _local_digest: &BloomDigest,
    ) {
        self.round += 1;
        tracing::debug!(
            "Gossip round {} starting (node {})",
            self.round,
            self.node_id
        );

        // TODO: Implement gossip round:
        // 1. membership.select_gossip_targets()
        // 2. For each target:
        //    a. Send CoverageDigest message
        //    b. Attach pending corpus entries (under size limit)
        //    c. If we have unshared crashes, send CrashAlert
        //    d. Periodically include StrategyUpdate
    }

    /// Handle a received CoverageDigest — compare with local coverage
    /// and respond with CoverageUpdate if we have novel edges.
    pub async fn handle_coverage_digest(
        &self,
        _sender: Uuid,
        _remote_digest: &BloomDigest,
        _local_digest: &BloomDigest,
    ) {
        // TODO: Compare digests, send CoverageUpdate if we have novel edges
    }

    /// Current gossip round number.
    pub fn round(&self) -> u64 {
        self.round
    }
}
