use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use uuid::Uuid;

use super::corpus::{CorpusEntry, CorpusManager};
use super::coverage::{CoverageBitmap, BITMAP_SIZE};
use super::{CrashInfo, FuzzResult, FuzzerBackend, FuzzerStats, InputMode, Severity, TargetConfig};

/// AFL++ backend implementation.
///
/// Wraps AFL++ tools (`afl-showmap`, `afl-fuzz`) via subprocess execution,
/// managing coverage feedback and processing results.
pub struct AflBackend {
    config: Option<TargetConfig>,
    coverage: CoverageBitmap,
    corpus: CorpusManager,
    stats: FuzzerStats,
    start_time: Option<Instant>,
    node_id: Uuid,
    /// Working directory for temporary files.
    work_dir: Option<PathBuf>,
    /// Path to afl-showmap binary.
    afl_showmap_path: Option<PathBuf>,
}

impl AflBackend {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            config: None,
            coverage: CoverageBitmap::new(),
            corpus: CorpusManager::new(node_id, 10_000),
            stats: FuzzerStats::default(),
            start_time: None,
            node_id,
            work_dir: None,
            afl_showmap_path: None,
        }
    }

    /// Locate the `afl-showmap` binary on the system.
    fn find_afl_showmap() -> Option<PathBuf> {
        // Check common locations
        let candidates = [
            "afl-showmap",
            "/usr/local/bin/afl-showmap",
            "/usr/bin/afl-showmap",
        ];

        for candidate in &candidates {
            let output = Command::new("which").arg(candidate).output().ok()?;
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Some(PathBuf::from(path));
            }
        }

        None
    }

    /// Run a single input through `afl-showmap` to collect coverage.
    fn run_with_showmap(&self, input: &[u8]) -> Result<(CoverageBitmap, Option<CrashInfo>, std::time::Duration)> {
        let config = self.config.as_ref().expect("not initialized");
        let work_dir = self.work_dir.as_ref().expect("no work dir");
        let showmap = self.afl_showmap_path.as_ref().expect("no afl-showmap");

        let exec_start = Instant::now();

        // Write input to a temporary file
        let input_path = work_dir.join("cur_input");
        std::fs::write(&input_path, input)
            .context("Failed to write input file")?;

        // Build afl-showmap command
        let coverage_path = work_dir.join("cur_coverage");
        let mut cmd = Command::new(showmap);
        cmd.arg("-o").arg(&coverage_path)
            .arg("-t").arg(config.timeout.as_millis().to_string())
            .arg("-m").arg(config.memory_limit_mb.to_string())
            .arg("-q"); // quiet mode

        match config.input_mode {
            InputMode::Stdin => {
                cmd.arg("--").arg(&config.binary_path);
                for arg in &config.arguments {
                    cmd.arg(arg);
                }
                cmd.stdin(std::process::Stdio::piped());
            }
            InputMode::File => {
                cmd.arg("--").arg(&config.binary_path);
                for arg in &config.arguments {
                    if arg == "@@" {
                        cmd.arg(&input_path);
                    } else {
                        cmd.arg(arg);
                    }
                }
            }
        }

        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn afl-showmap")?;

        // Feed input via stdin if needed
        if matches!(config.input_mode, InputMode::Stdin) {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(input);
            }
        }

        let output = child.wait_with_output().context("Failed to wait for afl-showmap")?;
        let exec_time = exec_start.elapsed();

        // Check for crash (non-zero exit status from the target)
        let crash_info = if !output.status.success() {
            let signal = output.status.code().unwrap_or(-1);
            let stderr_str = String::from_utf8_lossy(&output.stderr);

            // Extract ASAN report if present
            let asan_report = if stderr_str.contains("AddressSanitizer") || stderr_str.contains("ERROR: ") {
                Some(stderr_str.to_string())
            } else {
                None
            };

            let severity = crate::crash::scoring::score_exploitability(&CrashInfo {
                input: input.to_vec(),
                signal,
                stack_hash: 0,
                stack_trace: None,
                asan_report: asan_report.clone(),
                severity: Severity::Low,
            });

            // Compute stack hash from stderr
            let stack_hash = crate::utils::hash_bytes(stderr_str.as_bytes());

            // Signal 2 (SIGINT) and some exit codes are not crashes
            if signal != 0 && signal != 1 && signal != 2 {
                Some(CrashInfo {
                    input: input.to_vec(),
                    signal,
                    stack_hash,
                    stack_trace: if stderr_str.is_empty() {
                        None
                    } else {
                        Some(stderr_str.to_string())
                    },
                    asan_report,
                    severity,
                })
            } else {
                None
            }
        } else {
            None
        };

        // Parse coverage from afl-showmap output
        let coverage = if coverage_path.exists() {
            self.parse_showmap_output(&coverage_path)?
        } else {
            CoverageBitmap::new()
        };

        Ok((coverage, crash_info, exec_time))
    }

    /// Parse afl-showmap text output into a coverage bitmap.
    ///
    /// Format: one "edge_id:hit_count" pair per line.
    fn parse_showmap_output(&self, path: &Path) -> Result<CoverageBitmap> {
        let content = std::fs::read_to_string(path)
            .context("Failed to read showmap output")?;

        let mut bitmap = CoverageBitmap::new();
        let map = bitmap.as_bytes_mut();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((edge_str, count_str)) = line.split_once(':') {
                if let (Ok(edge), Ok(count)) = (edge_str.parse::<usize>(), count_str.parse::<u8>()) {
                    if edge < BITMAP_SIZE {
                        map[edge] = count;
                    }
                }
            }
        }

        bitmap.classify_counts();
        Ok(bitmap)
    }

    /// Execute a target binary directly (without AFL++ tooling).
    /// Used as a fallback when afl-showmap is not available.
    fn run_direct(&self, input: &[u8]) -> Result<(Option<CrashInfo>, std::time::Duration)> {
        let config = self.config.as_ref().expect("not initialized");
        let work_dir = self.work_dir.as_ref().expect("no work dir");
        let exec_start = Instant::now();

        let input_path = work_dir.join("cur_input");
        std::fs::write(&input_path, input).context("Failed to write input file")?;

        let mut cmd = Command::new(&config.binary_path);
        for arg in &config.arguments {
            if arg == "@@" {
                cmd.arg(&input_path);
            } else {
                cmd.arg(arg);
            }
        }

        match config.input_mode {
            InputMode::Stdin => {
                cmd.stdin(std::process::Stdio::piped());
            }
            InputMode::File => {
                cmd.stdin(std::process::Stdio::null());
            }
        }

        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().context("Failed to spawn target binary")?;

        if matches!(config.input_mode, InputMode::Stdin) {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(input);
            }
        }

        let output = child.wait_with_output().context("Failed to wait for target")?;
        let exec_time = exec_start.elapsed();

        let crash_info = if !output.status.success() {
            let signal = output.status.code().unwrap_or(-1);
            // Skip normal exit codes
            if signal > 1 {
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let asan_report = if stderr_str.contains("AddressSanitizer") {
                    Some(stderr_str.to_string())
                } else {
                    None
                };

                let severity = crate::crash::scoring::score_exploitability(&CrashInfo {
                    input: input.to_vec(),
                    signal,
                    stack_hash: 0,
                    stack_trace: None,
                    asan_report: asan_report.clone(),
                    severity: Severity::Low,
                });

                let stack_hash = crate::utils::hash_bytes(stderr_str.as_bytes());

                Some(CrashInfo {
                    input: input.to_vec(),
                    signal,
                    stack_hash,
                    stack_trace: if stderr_str.is_empty() {
                        None
                    } else {
                        Some(stderr_str.to_string())
                    },
                    asan_report,
                    severity,
                })
            } else {
                None
            }
        } else {
            None
        };

        Ok((crash_info, exec_time))
    }
}

impl FuzzerBackend for AflBackend {
    fn init(&mut self, target: &TargetConfig) -> Result<()> {
        // Validate target binary exists
        let binary = Path::new(&target.binary_path);
        if !binary.exists() {
            bail!("Target binary not found: {}", target.binary_path);
        }

        // Set up working directory
        let work_dir = std::env::temp_dir().join(format!("hivefuzz-{}", self.node_id));
        std::fs::create_dir_all(&work_dir)
            .context("Failed to create working directory")?;
        self.work_dir = Some(work_dir);

        // Locate AFL++ tools
        self.afl_showmap_path = Self::find_afl_showmap();
        if self.afl_showmap_path.is_some() {
            tracing::info!(
                "AFL++ found: afl-showmap at {}",
                self.afl_showmap_path.as_ref().unwrap().display()
            );
        } else {
            tracing::warn!(
                "afl-showmap not found — running in direct execution mode (no coverage feedback)"
            );
        }

        self.config = Some(target.clone());
        self.start_time = Some(Instant::now());
        tracing::info!(
            "AFL++ backend initialized for target: {}",
            target.binary_path
        );
        Ok(())
    }

    fn run_input(&mut self, input: &[u8]) -> Result<FuzzResult> {
        if self.config.is_none() {
            bail!("AFL++ backend not initialized — call init() first");
        }

        let (coverage, new_edges, crash, exec_time) = if self.afl_showmap_path.is_some() {
            // Use afl-showmap for coverage-guided execution
            let (exec_coverage, crash, exec_time) = self.run_with_showmap(input)?;
            let new_edges = self.coverage.merge(&exec_coverage);
            (exec_coverage, new_edges, crash, exec_time)
        } else {
            // Direct execution (no coverage)
            let (crash, exec_time) = self.run_direct(input)?;
            (CoverageBitmap::new(), 0, crash, exec_time)
        };

        self.stats.total_executions += 1;
        if crash.is_some() {
            self.stats.total_crashes += 1;
        }
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                self.stats.executions_per_sec =
                    self.stats.total_executions as f64 / elapsed;
            }
        }

        Ok(FuzzResult {
            coverage,
            new_edges,
            crash,
            exec_time,
        })
    }

    fn get_coverage(&self) -> &CoverageBitmap {
        &self.coverage
    }

    fn get_corpus(&self) -> Vec<CorpusEntry> {
        self.corpus.entries().cloned().collect()
    }

    fn add_to_corpus(&mut self, entry: CorpusEntry) -> Result<()> {
        self.corpus.import(entry);
        self.stats.corpus_size = self.corpus.len();
        Ok(())
    }

    fn stats(&self) -> FuzzerStats {
        let mut stats = self.stats.clone();
        stats.total_edges = self.coverage.count_edges();
        if let Some(start) = self.start_time {
            stats.uptime_secs = start.elapsed().as_secs();
        }
        stats
    }
}

impl Drop for AflBackend {
    fn drop(&mut self) {
        // Clean up working directory
        if let Some(ref work_dir) = self.work_dir {
            let _ = std::fs::remove_dir_all(work_dir);
        }
    }
}
