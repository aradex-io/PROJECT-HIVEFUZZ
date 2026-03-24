pub mod dedup;
pub mod scoring;

use serde::{Deserialize, Serialize};

use crate::fuzzer::{CrashInfo, Severity};

/// A fully-triaged crash record in the crash database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRecord {
    /// Unique crash ID.
    pub id: u64,
    /// Stack trace hash (primary dedup key).
    pub stack_hash: u64,
    /// Coverage bitmap fingerprint at crash point.
    pub coverage_fingerprint: u64,
    /// The crash details.
    pub crash_info: CrashInfo,
    /// Has this crash been minimized?
    pub minimized: bool,
    /// Minimized input (if available).
    pub minimized_input: Option<Vec<u8>>,
    /// Which node(s) discovered this crash.
    pub discovered_by: Vec<uuid::Uuid>,
    /// Timestamp of first discovery.
    pub first_seen: i64,
    /// Timestamp of last occurrence.
    pub last_seen: i64,
    /// Number of times this crash has been hit across the swarm.
    pub hit_count: u64,
    /// Has this crash been shared via gossip?
    pub disseminated: bool,
    /// Suggested CWE classification.
    pub suggested_cwe: Option<String>,
}

/// Manages the per-node crash database.
pub struct CrashDatabase {
    /// In-memory cache of crash records, keyed by stack_hash.
    records: std::collections::HashMap<u64, CrashRecord>,
    // TODO: SQLite persistence layer
}

impl CrashDatabase {
    pub fn new() -> Self {
        Self {
            records: std::collections::HashMap::new(),
        }
    }

    /// Record a new crash. Returns true if this is a novel crash.
    pub fn record(&mut self, crash: CrashInfo, node_id: uuid::Uuid) -> bool {
        let now = chrono::Utc::now().timestamp();

        if let Some(existing) = self.records.get_mut(&crash.stack_hash) {
            existing.hit_count += 1;
            existing.last_seen = now;
            if !existing.discovered_by.contains(&node_id) {
                existing.discovered_by.push(node_id);
            }
            return false;
        }

        let record = CrashRecord {
            id: crash.stack_hash, // simplified; use proper ID generation
            stack_hash: crash.stack_hash,
            coverage_fingerprint: 0, // TODO: compute from coverage bitmap
            minimized: false,
            minimized_input: None,
            discovered_by: vec![node_id],
            first_seen: now,
            last_seen: now,
            hit_count: 1,
            disseminated: false,
            suggested_cwe: scoring::suggest_cwe(&crash),
            crash_info: crash,
        };

        self.records.insert(record.stack_hash, record);
        true
    }

    /// Get crashes that haven't been disseminated yet.
    pub fn pending_dissemination(&self) -> Vec<&CrashRecord> {
        self.records
            .values()
            .filter(|r| !r.disseminated)
            .collect()
    }

    /// Mark a crash as disseminated.
    pub fn mark_disseminated(&mut self, stack_hash: u64) {
        if let Some(record) = self.records.get_mut(&stack_hash) {
            record.disseminated = true;
        }
    }

    /// Get all crash records sorted by severity (highest first).
    pub fn all_by_severity(&self) -> Vec<&CrashRecord> {
        let mut records: Vec<_> = self.records.values().collect();
        records.sort_by(|a, b| b.crash_info.severity.cmp(&a.crash_info.severity));
        records
    }

    /// Number of unique crashes.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Summary statistics.
    pub fn summary(&self) -> CrashSummary {
        let mut summary = CrashSummary::default();
        for record in self.records.values() {
            summary.total += 1;
            match record.crash_info.severity {
                Severity::Critical => summary.critical += 1,
                Severity::High => summary.high += 1,
                Severity::Medium => summary.medium += 1,
                Severity::Low => summary.low += 1,
            }
        }
        summary
    }
}

impl Default for CrashDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct CrashSummary {
    pub total: u32,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
}
