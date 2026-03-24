pub mod afl;
pub mod corpus;
pub mod coverage;

use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Configuration for a fuzz target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetConfig {
    /// Path to the instrumented target binary.
    pub binary_path: String,
    /// Command-line arguments (use @@ for input file placeholder).
    pub arguments: Vec<String>,
    /// Timeout per execution.
    pub timeout: Duration,
    /// Memory limit in MB.
    pub memory_limit_mb: u64,
    /// Whether the target reads from stdin or a file.
    pub input_mode: InputMode,
    /// Optional dictionary path.
    pub dictionary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputMode {
    Stdin,
    File,
}

/// Core fuzzer trait — backend-agnostic.
///
/// Any fuzzing engine (AFL++, libFuzzer, Honggfuzz) implements this trait
/// to participate in the HIVEFUZZ swarm.
pub trait FuzzerBackend: Send {
    /// Initialize the fuzzer with a target configuration.
    fn init(&mut self, target: &TargetConfig) -> Result<()>;

    /// Execute a single input and return the result.
    fn run_input(&mut self, input: &[u8]) -> Result<FuzzResult>;

    /// Get the current cumulative coverage bitmap.
    fn get_coverage(&self) -> &coverage::CoverageBitmap;

    /// Get all corpus entries.
    fn get_corpus(&self) -> Vec<corpus::CorpusEntry>;

    /// Add a new entry to the corpus.
    fn add_to_corpus(&mut self, entry: corpus::CorpusEntry) -> Result<()>;

    /// Get execution statistics.
    fn stats(&self) -> FuzzerStats;
}

/// Result of executing a single fuzz input.
#[derive(Debug, Clone)]
pub struct FuzzResult {
    /// Coverage bitmap from this execution.
    pub coverage: coverage::CoverageBitmap,
    /// Number of new edges discovered by this input.
    pub new_edges: u32,
    /// Crash information, if the input triggered a crash.
    pub crash: Option<CrashInfo>,
    /// Wall-clock execution time.
    pub exec_time: Duration,
}

/// Information about a crash triggered by a fuzz input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashInfo {
    /// The input that triggered the crash.
    pub input: Vec<u8>,
    /// Signal that killed the process (e.g., 11 for SIGSEGV).
    pub signal: i32,
    /// Hash of the top N stack frames for deduplication.
    pub stack_hash: u64,
    /// Raw stack trace.
    pub stack_trace: Option<String>,
    /// ASAN/MSAN report if available.
    pub asan_report: Option<String>,
    /// Estimated severity/exploitability.
    pub severity: Severity,
}

/// Crash severity classification based on bug class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Aggregate fuzzer statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FuzzerStats {
    pub total_executions: u64,
    pub executions_per_sec: f64,
    pub total_edges: u32,
    pub total_crashes: u32,
    pub corpus_size: usize,
    pub uptime_secs: u64,
}
