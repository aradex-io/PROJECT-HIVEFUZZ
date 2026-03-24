/// TOML-based configuration for HIVEFUZZ targets.
///
/// A `hivefuzz.toml` file describes the fuzz target, corpus location,
/// and tuning parameters for a fuzzing campaign.
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::fuzzer::{InputMode, TargetConfig};

/// Top-level configuration loaded from `hivefuzz.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HivefuzzConfig {
    /// Target binary configuration.
    pub target: TargetSection,

    /// Corpus configuration.
    #[serde(default)]
    pub corpus: CorpusSection,

    /// Fuzzer tuning parameters.
    #[serde(default)]
    pub fuzzer: FuzzerSection,

    /// Gossip / swarm configuration.
    #[serde(default)]
    pub swarm: SwarmSection,
}

/// `[target]` section — describes the binary to fuzz.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetSection {
    /// Path to the instrumented target binary.
    pub binary: String,

    /// Command-line arguments. Use `@@` as placeholder for the input file.
    #[serde(default)]
    pub arguments: Vec<String>,

    /// How the target receives fuzz input.
    #[serde(default = "default_input_mode")]
    pub input_mode: InputModeConfig,

    /// Per-execution timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Memory limit in megabytes.
    #[serde(default = "default_memory_limit")]
    pub memory_limit_mb: u64,

    /// Optional dictionary file for dictionary-based mutations.
    pub dictionary: Option<String>,
}

/// `[corpus]` section — seed corpus and output directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusSection {
    /// Path to seed corpus directory.
    #[serde(default = "default_seeds_dir")]
    pub seeds: String,

    /// Path to output directory (crashes, queue, etc.).
    #[serde(default = "default_output_dir")]
    pub output: String,

    /// Maximum corpus size before minimization triggers.
    #[serde(default = "default_max_corpus_size")]
    pub max_size: usize,
}

/// `[fuzzer]` section — tuning parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzerSection {
    /// Havoc stage mutation depth.
    #[serde(default = "default_havoc_depth")]
    pub havoc_depth: u32,

    /// Strategy evolution interval (in executions).
    #[serde(default = "default_evolution_interval")]
    pub evolution_interval: u64,
}

/// `[swarm]` section — gossip and networking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSection {
    /// Gossip interval in seconds.
    #[serde(default = "default_gossip_interval_secs")]
    pub gossip_interval_secs: u64,

    /// Number of peers to gossip with each round.
    #[serde(default = "default_fanout")]
    pub fanout: usize,
}

/// How the target binary receives fuzz input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputModeConfig {
    #[serde(rename = "stdin")]
    Stdin,
    #[serde(rename = "file")]
    File,
}

// --- Defaults ---

fn default_input_mode() -> InputModeConfig {
    InputModeConfig::Stdin
}

fn default_timeout_ms() -> u64 {
    5000
}

fn default_memory_limit() -> u64 {
    256
}

fn default_seeds_dir() -> String {
    "seeds".to_string()
}

fn default_output_dir() -> String {
    "output".to_string()
}

fn default_max_corpus_size() -> usize {
    10_000
}

fn default_havoc_depth() -> u32 {
    4
}

fn default_evolution_interval() -> u64 {
    100_000
}

fn default_gossip_interval_secs() -> u64 {
    5
}

fn default_fanout() -> usize {
    3
}

impl Default for CorpusSection {
    fn default() -> Self {
        Self {
            seeds: default_seeds_dir(),
            output: default_output_dir(),
            max_size: default_max_corpus_size(),
        }
    }
}

impl Default for FuzzerSection {
    fn default() -> Self {
        Self {
            havoc_depth: default_havoc_depth(),
            evolution_interval: default_evolution_interval(),
        }
    }
}

impl Default for SwarmSection {
    fn default() -> Self {
        Self {
            gossip_interval_secs: default_gossip_interval_secs(),
            fanout: default_fanout(),
        }
    }
}

impl HivefuzzConfig {
    /// Load configuration from a TOML file.
    pub fn load(path: &Path) -> Result<Self> {
        let content =
            std::fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;
        let config: Self =
            toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
        config.validate(path)?;
        Ok(config)
    }

    /// Validate the configuration for correctness.
    fn validate(&self, config_path: &Path) -> Result<()> {
        let base_dir = config_path.parent().unwrap_or(Path::new("."));
        let binary_path = base_dir.join(&self.target.binary);

        if !binary_path.exists() {
            bail!(
                "Target binary not found: {} (resolved to {})",
                self.target.binary,
                binary_path.display()
            );
        }

        if self.target.timeout_ms == 0 {
            bail!("Timeout must be greater than 0");
        }

        if self.target.memory_limit_mb == 0 {
            bail!("Memory limit must be greater than 0");
        }

        Ok(())
    }

    /// Convert to the internal `TargetConfig` used by the fuzzer backend.
    pub fn to_target_config(&self) -> TargetConfig {
        TargetConfig {
            binary_path: self.target.binary.clone(),
            arguments: self.target.arguments.clone(),
            timeout: Duration::from_millis(self.target.timeout_ms),
            memory_limit_mb: self.target.memory_limit_mb,
            input_mode: match self.target.input_mode {
                InputModeConfig::Stdin => InputMode::Stdin,
                InputModeConfig::File => InputMode::File,
            },
            dictionary: self.target.dictionary.clone(),
        }
    }

    /// Generate a default `hivefuzz.toml` for a target binary.
    pub fn generate_default(target_binary: &str) -> String {
        format!(
            r#"# HIVEFUZZ target configuration

[target]
binary = "{target_binary}"
arguments = []
input_mode = "stdin"
timeout_ms = 5000
memory_limit_mb = 256
# dictionary = "dict.txt"

[corpus]
seeds = "seeds"
output = "output"
max_size = 10000

[fuzzer]
havoc_depth = 4
evolution_interval = 100000

[swarm]
gossip_interval_secs = 5
fanout = 3
"#
        )
    }

    /// Get the output directory path, resolved relative to config file location.
    pub fn output_dir(&self, config_path: &Path) -> PathBuf {
        let base_dir = config_path.parent().unwrap_or(Path::new("."));
        base_dir.join(&self.corpus.output)
    }

    /// Get the seeds directory path, resolved relative to config file location.
    pub fn seeds_dir(&self, config_path: &Path) -> PathBuf {
        let base_dir = config_path.parent().unwrap_or(Path::new("."));
        base_dir.join(&self.corpus.seeds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
[target]
binary = "/bin/true"
"#;
        let config: HivefuzzConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target.binary, "/bin/true");
        assert_eq!(config.target.timeout_ms, 5000);
        assert_eq!(config.corpus.seeds, "seeds");
        assert_eq!(config.swarm.fanout, 3);
    }

    #[test]
    fn test_parse_full_config() {
        let toml_str = r#"
[target]
binary = "/bin/true"
arguments = ["--flag", "@@"]
input_mode = "file"
timeout_ms = 1000
memory_limit_mb = 512
dictionary = "dict.txt"

[corpus]
seeds = "my_seeds"
output = "my_output"
max_size = 5000

[fuzzer]
havoc_depth = 8
evolution_interval = 50000

[swarm]
gossip_interval_secs = 10
fanout = 5
"#;
        let config: HivefuzzConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.target.arguments, vec!["--flag", "@@"]);
        assert_eq!(config.target.timeout_ms, 1000);
        assert_eq!(config.target.memory_limit_mb, 512);
        assert_eq!(config.corpus.seeds, "my_seeds");
        assert_eq!(config.corpus.max_size, 5000);
        assert_eq!(config.fuzzer.havoc_depth, 8);
        assert_eq!(config.swarm.gossip_interval_secs, 10);
    }

    #[test]
    fn test_generate_default_config() {
        let config_str = HivefuzzConfig::generate_default("./target_binary");
        assert!(config_str.contains("binary = \"./target_binary\""));
        assert!(config_str.contains("[corpus]"));
        assert!(config_str.contains("[swarm]"));

        // Ensure the generated config can be parsed back
        let _config: HivefuzzConfig = toml::from_str(&config_str).unwrap();
    }

    #[test]
    fn test_to_target_config() {
        let toml_str = r#"
[target]
binary = "/bin/true"
arguments = ["--test"]
input_mode = "file"
timeout_ms = 2000
memory_limit_mb = 128
"#;
        let config: HivefuzzConfig = toml::from_str(toml_str).unwrap();
        let target = config.to_target_config();

        assert_eq!(target.binary_path, "/bin/true");
        assert_eq!(target.arguments, vec!["--test"]);
        assert_eq!(target.timeout, Duration::from_millis(2000));
        assert_eq!(target.memory_limit_mb, 128);
        assert!(matches!(target.input_mode, InputMode::File));
    }

    #[test]
    fn test_load_and_validate() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("hivefuzz.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        write!(
            file,
            "{}",
            HivefuzzConfig::generate_default("/bin/true")
        )
        .unwrap();

        let config = HivefuzzConfig::load(&config_path).unwrap();
        assert_eq!(config.target.binary, "/bin/true");
    }

    #[test]
    fn test_validate_missing_binary() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("hivefuzz.toml");
        let mut file = std::fs::File::create(&config_path).unwrap();
        write!(
            file,
            "{}",
            HivefuzzConfig::generate_default("./nonexistent_binary")
        )
        .unwrap();

        let result = HivefuzzConfig::load(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
