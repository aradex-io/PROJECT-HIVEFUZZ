use serde::{Deserialize, Serialize};

/// AFL-style edge coverage bitmap.
///
/// Each byte represents a hit count bucket for a particular edge (branch pair).
/// The bitmap is a fixed-size array indexed by `(prev_location XOR cur_location)`.
pub const BITMAP_SIZE: usize = 65536; // 64KB, standard AFL bitmap size

#[derive(Clone, Serialize, Deserialize)]
pub struct CoverageBitmap {
    /// Raw bitmap — each byte is a hit count for one edge.
    map: Vec<u8>,
}

impl std::fmt::Debug for CoverageBitmap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoverageBitmap")
            .field("size", &self.map.len())
            .field("edges_hit", &self.count_edges())
            .finish()
    }
}

impl CoverageBitmap {
    /// Create a new zeroed bitmap.
    pub fn new() -> Self {
        Self {
            map: vec![0u8; BITMAP_SIZE],
        }
    }

    /// Create a bitmap from raw bytes.
    pub fn from_raw(data: Vec<u8>) -> Self {
        assert_eq!(data.len(), BITMAP_SIZE);
        Self { map: data }
    }

    /// Get raw bitmap bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.map
    }

    /// Count the number of edges (non-zero entries) in the bitmap.
    pub fn count_edges(&self) -> u32 {
        self.map.iter().filter(|&&b| b != 0).count() as u32
    }

    /// Merge another bitmap into this one (union).
    /// Returns the number of new edges added.
    pub fn merge(&mut self, other: &CoverageBitmap) -> u32 {
        let mut new_edges = 0u32;
        for (a, &b) in self.map.iter_mut().zip(other.map.iter()) {
            if *a == 0 && b != 0 {
                new_edges += 1;
            }
            *a |= b;
        }
        new_edges
    }

    /// Compute the diff: edges present in `other` but not in `self`.
    pub fn diff(&self, other: &CoverageBitmap) -> CoverageBitmap {
        let mut result = CoverageBitmap::new();
        for i in 0..BITMAP_SIZE {
            if self.map[i] == 0 && other.map[i] != 0 {
                result.map[i] = other.map[i];
            }
        }
        result
    }

    /// Check if this bitmap has any edges not present in `other`.
    pub fn has_novel_edges(&self, other: &CoverageBitmap) -> bool {
        for i in 0..BITMAP_SIZE {
            if self.map[i] != 0 && other.map[i] == 0 {
                return true;
            }
        }
        false
    }

    /// Generate a compact bloom filter digest for bandwidth-efficient gossip.
    /// Uses a smaller representation (~1KB) for coverage comparison.
    pub fn to_bloom_digest(&self) -> BloomDigest {
        let mut digest = BloomDigest::new();
        for (i, &byte) in self.map.iter().enumerate() {
            if byte != 0 {
                digest.set(i);
            }
        }
        digest
    }

    /// Classify hit counts into AFL-style buckets.
    pub fn classify_counts(&mut self) {
        for byte in self.map.iter_mut() {
            *byte = match *byte {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 4,
                4..=7 => 8,
                8..=15 => 16,
                16..=31 => 32,
                32..=127 => 64,
                128..=255 => 128,
            };
        }
    }
}

impl Default for CoverageBitmap {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact bloom filter for bandwidth-efficient coverage comparison during gossip.
///
/// ~1KB representation vs 64KB for the full bitmap. Used to quickly determine
/// if a peer has coverage we don't (with some false positives).
pub const BLOOM_SIZE: usize = 1024; // 1KB
const BLOOM_HASH_COUNT: u32 = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomDigest {
    bits: Vec<u8>,
}

impl BloomDigest {
    pub fn new() -> Self {
        Self {
            bits: vec![0u8; BLOOM_SIZE],
        }
    }

    fn set(&mut self, index: usize) {
        for k in 0..BLOOM_HASH_COUNT {
            let hash = Self::hash(index, k);
            let byte_idx = hash / 8;
            let bit_idx = hash % 8;
            self.bits[byte_idx] |= 1 << bit_idx;
        }
    }

    /// Check if this digest likely contains coverage not in `other`.
    /// False positives possible, false negatives not.
    pub fn likely_has_novel(&self, other: &BloomDigest) -> bool {
        for (a, b) in self.bits.iter().zip(other.bits.iter()) {
            if a & !b != 0 {
                return true;
            }
        }
        false
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bits
    }

    fn hash(index: usize, k: u32) -> usize {
        // Simple double-hashing scheme
        let h1 = index;
        let h2 = index.wrapping_mul(2654435761); // Knuth's multiplicative hash
        (h1.wrapping_add((k as usize).wrapping_mul(h2))) % (BLOOM_SIZE * 8)
    }
}

impl Default for BloomDigest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_bitmap() {
        let bm = CoverageBitmap::new();
        assert_eq!(bm.count_edges(), 0);
    }

    #[test]
    fn test_merge() {
        let mut a = CoverageBitmap::new();
        let mut b = CoverageBitmap::new();

        a.map[0] = 1;
        a.map[1] = 1;
        b.map[1] = 1;
        b.map[2] = 1;

        let new = a.merge(&b);
        assert_eq!(new, 1); // only edge at index 2 is new
        assert_eq!(a.count_edges(), 3);
    }

    #[test]
    fn test_diff() {
        let mut a = CoverageBitmap::new();
        let mut b = CoverageBitmap::new();

        a.map[0] = 1;
        b.map[0] = 1;
        b.map[1] = 1;

        let diff = a.diff(&b);
        assert_eq!(diff.map[0], 0); // both have it
        assert_eq!(diff.map[1], 1); // only b has it
    }

    #[test]
    fn test_bloom_digest() {
        let mut bm = CoverageBitmap::new();
        bm.map[100] = 1;
        bm.map[200] = 1;

        let digest = bm.to_bloom_digest();
        let empty_digest = CoverageBitmap::new().to_bloom_digest();

        assert!(digest.likely_has_novel(&empty_digest));
    }
}
