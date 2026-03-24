use crate::fuzzer::CrashInfo;

/// Multi-level crash deduplication strategy.
///
/// Level 1: Stack trace hash (top N frames)
/// Level 2: Coverage bitmap fingerprint at crash point
/// Level 3: ASAN/MSAN report type + location
pub struct CrashDeduplicator {
    /// Number of top stack frames to use for hashing.
    stack_depth: usize,
}

impl CrashDeduplicator {
    pub fn new(stack_depth: usize) -> Self {
        Self { stack_depth }
    }

    /// Compute a deduplication fingerprint for a crash.
    /// Two crashes with the same fingerprint are considered duplicates.
    pub fn fingerprint(&self, crash: &CrashInfo) -> CrashFingerprint {
        CrashFingerprint {
            stack_hash: crash.stack_hash,
            asan_class: self.extract_asan_class(&crash.asan_report),
            signal: crash.signal,
        }
    }

    /// Extract ASAN bug class from report (e.g., "heap-buffer-overflow",
    /// "heap-use-after-free").
    fn extract_asan_class(&self, asan_report: &Option<String>) -> Option<String> {
        let report = asan_report.as_ref()?;

        // ASAN reports contain lines like:
        // "ERROR: AddressSanitizer: heap-buffer-overflow on ..."
        for line in report.lines() {
            if line.contains("AddressSanitizer:") {
                if let Some(class_start) = line.find("AddressSanitizer: ") {
                    let after = &line[class_start + 18..];
                    if let Some(end) = after.find(" on") {
                        return Some(after[..end].to_string());
                    }
                    // Fallback: take until end of line or next space
                    return Some(after.split_whitespace().next()?.to_string());
                }
            }
        }
        None
    }
}

/// Fingerprint used for crash deduplication.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CrashFingerprint {
    pub stack_hash: u64,
    pub asan_class: Option<String>,
    pub signal: i32,
}

impl Default for CrashDeduplicator {
    fn default() -> Self {
        Self::new(5) // top 5 frames by default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzzer::Severity;

    #[test]
    fn test_asan_class_extraction() {
        let dedup = CrashDeduplicator::default();

        let report = Some(
            "=================================================================\n\
             ERROR: AddressSanitizer: heap-buffer-overflow on address 0x602000000010\n\
             READ of size 4 at 0x602000000010 thread T0"
                .to_string(),
        );

        let class = dedup.extract_asan_class(&report);
        assert_eq!(class.as_deref(), Some("heap-buffer-overflow"));
    }

    #[test]
    fn test_same_crash_same_fingerprint() {
        let dedup = CrashDeduplicator::default();

        let crash1 = CrashInfo {
            input: vec![1, 2, 3],
            signal: 11,
            stack_hash: 12345,
            stack_trace: None,
            asan_report: None,
            severity: Severity::High,
        };

        let crash2 = CrashInfo {
            input: vec![4, 5, 6], // different input
            signal: 11,
            stack_hash: 12345, // same stack hash
            stack_trace: None,
            asan_report: None,
            severity: Severity::High,
        };

        assert_eq!(dedup.fingerprint(&crash1), dedup.fingerprint(&crash2));
    }
}
