use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A single entry in the fuzzing corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusEntry {
    /// Unique identifier for this corpus entry.
    pub id: Uuid,
    /// The actual input data.
    pub data: Vec<u8>,
    /// SHA-256 hash of the data for deduplication.
    pub content_hash: u64,
    /// Number of new edges this input discovered when first found.
    pub new_edges: u32,
    /// Which node originally discovered this input.
    pub discovered_by: Uuid,
    /// Which mutation operation produced this input.
    pub mutation_source: Option<String>,
    /// Parent corpus entry ID (if this was derived from another entry).
    pub parent_id: Option<Uuid>,
    /// When this entry was discovered.
    pub discovered_at: DateTime<Utc>,
    /// Whether this entry has been shared via gossip.
    pub disseminated: bool,
}

/// Manages the local corpus for a fuzzer node.
pub struct CorpusManager {
    /// All corpus entries, keyed by content hash for dedup.
    entries: HashMap<u64, CorpusEntry>,
    /// Maximum corpus size before minimization triggers.
    max_size: usize,
    /// This node's ID (for provenance tracking).
    node_id: Uuid,
}

impl CorpusManager {
    pub fn new(node_id: Uuid, max_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_size,
            node_id,
        }
    }

    /// Add a new corpus entry. Returns true if it was novel (not a duplicate).
    pub fn add(&mut self, data: Vec<u8>, new_edges: u32, mutation_source: Option<String>, parent_id: Option<Uuid>) -> bool {
        let content_hash = Self::hash_content(&data);

        if self.entries.contains_key(&content_hash) {
            return false;
        }

        let entry = CorpusEntry {
            id: Uuid::new_v4(),
            content_hash,
            new_edges,
            discovered_by: self.node_id,
            mutation_source,
            parent_id,
            discovered_at: Utc::now(),
            disseminated: false,
            data,
        };

        self.entries.insert(content_hash, entry);
        true
    }

    /// Import a corpus entry received from a peer via gossip.
    pub fn import(&mut self, entry: CorpusEntry) -> bool {
        if self.entries.contains_key(&entry.content_hash) {
            return false;
        }
        let hash = entry.content_hash;
        self.entries.insert(hash, entry);
        true
    }

    /// Get entries that haven't been disseminated yet, prioritized by new_edges.
    pub fn pending_dissemination(&self, limit: usize) -> Vec<&CorpusEntry> {
        let mut pending: Vec<_> = self.entries.values().filter(|e| !e.disseminated).collect();
        pending.sort_by(|a, b| b.new_edges.cmp(&a.new_edges));
        pending.truncate(limit);
        pending
    }

    /// Mark entries as disseminated.
    pub fn mark_disseminated(&mut self, ids: &[Uuid]) {
        for entry in self.entries.values_mut() {
            if ids.contains(&entry.id) {
                entry.disseminated = true;
            }
        }
    }

    /// Get all entries.
    pub fn entries(&self) -> impl Iterator<Item = &CorpusEntry> {
        self.entries.values()
    }

    /// Number of entries in the corpus.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Check if corpus needs minimization.
    pub fn needs_minimization(&self) -> bool {
        self.entries.len() > self.max_size
    }

    fn hash_content(data: &[u8]) -> u64 {
        xxhash_rust::xxh3::xxh3_64(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_dedup() {
        let node_id = Uuid::new_v4();
        let mut corpus = CorpusManager::new(node_id, 1000);

        assert!(corpus.add(b"hello".to_vec(), 5, None, None));
        assert!(!corpus.add(b"hello".to_vec(), 5, None, None)); // duplicate
        assert!(corpus.add(b"world".to_vec(), 3, None, None));

        assert_eq!(corpus.len(), 2);
    }

    #[test]
    fn test_pending_dissemination() {
        let node_id = Uuid::new_v4();
        let mut corpus = CorpusManager::new(node_id, 1000);

        corpus.add(b"low".to_vec(), 1, None, None);
        corpus.add(b"high".to_vec(), 10, None, None);
        corpus.add(b"mid".to_vec(), 5, None, None);

        let pending = corpus.pending_dissemination(2);
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].new_edges, 10); // highest priority first
    }
}
