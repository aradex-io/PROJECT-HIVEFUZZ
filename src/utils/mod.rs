/// Shared utilities for HIVEFUZZ.

/// Compute xxh3 hash of arbitrary data.
pub fn hash_bytes(data: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(data)
}
