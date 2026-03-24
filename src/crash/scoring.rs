use crate::fuzzer::{CrashInfo, Severity};

/// Score a crash's exploitability based on ASAN report and signal type.
pub fn score_exploitability(crash: &CrashInfo) -> Severity {
    // If we have an ASAN report, use it for classification
    if let Some(ref report) = crash.asan_report {
        return score_from_asan(report);
    }

    // Fall back to signal-based heuristic
    match crash.signal {
        11 => Severity::Medium, // SIGSEGV — could be anything
        6 => Severity::Low,     // SIGABRT — usually assertion failure
        4 => Severity::High,    // SIGILL — could indicate code exec
        8 => Severity::Low,     // SIGFPE — division by zero
        _ => Severity::Low,
    }
}

/// Score exploitability from ASAN report content.
fn score_from_asan(report: &str) -> Severity {
    let report_lower = report.to_lowercase();

    // Critical: write primitives
    if report_lower.contains("heap-buffer-overflow") && report_lower.contains("write") {
        return Severity::Critical;
    }

    // High: use-after-free, stack buffer overflow, double-free
    if report_lower.contains("heap-use-after-free")
        || report_lower.contains("stack-buffer-overflow")
        || report_lower.contains("double-free")
    {
        return Severity::High;
    }

    // Medium: read overflows, uninitialized memory
    if (report_lower.contains("heap-buffer-overflow") && report_lower.contains("read"))
        || report_lower.contains("use-of-uninitialized-value")
    {
        return Severity::Medium;
    }

    // Low: null deref, stack overflow, assertion failures
    if report_lower.contains("null")
        || report_lower.contains("stack-overflow")
        || report_lower.contains("assertion")
    {
        return Severity::Low;
    }

    // Default to medium for unknown ASAN types
    Severity::Medium
}

/// Suggest a CWE classification based on crash characteristics.
pub fn suggest_cwe(crash: &CrashInfo) -> Option<String> {
    let report = crash.asan_report.as_ref()?;
    let report_lower = report.to_lowercase();

    if report_lower.contains("heap-buffer-overflow") {
        Some("CWE-122: Heap-based Buffer Overflow".to_string())
    } else if report_lower.contains("stack-buffer-overflow") {
        Some("CWE-121: Stack-based Buffer Overflow".to_string())
    } else if report_lower.contains("heap-use-after-free") {
        Some("CWE-416: Use After Free".to_string())
    } else if report_lower.contains("double-free") {
        Some("CWE-415: Double Free".to_string())
    } else if report_lower.contains("null") {
        Some("CWE-476: NULL Pointer Dereference".to_string())
    } else if report_lower.contains("use-of-uninitialized-value") {
        Some("CWE-457: Use of Uninitialized Variable".to_string())
    } else if report_lower.contains("integer-overflow") {
        Some("CWE-190: Integer Overflow".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asan_scoring() {
        assert_eq!(
            score_from_asan("ERROR: AddressSanitizer: heap-buffer-overflow on WRITE"),
            Severity::Critical
        );
        assert_eq!(
            score_from_asan("ERROR: AddressSanitizer: heap-use-after-free"),
            Severity::High
        );
        assert_eq!(
            score_from_asan("ERROR: AddressSanitizer: heap-buffer-overflow on READ"),
            Severity::Medium
        );
    }

    #[test]
    fn test_cwe_suggestion() {
        let crash = CrashInfo {
            input: vec![],
            signal: 11,
            stack_hash: 0,
            stack_trace: None,
            asan_report: Some("ERROR: AddressSanitizer: heap-use-after-free".to_string()),
            severity: Severity::High,
        };

        let cwe = suggest_cwe(&crash);
        assert_eq!(cwe.as_deref(), Some("CWE-416: Use After Free"));
    }
}
