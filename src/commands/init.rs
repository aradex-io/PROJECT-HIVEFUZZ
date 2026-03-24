use std::path::Path;

use anyhow::{bail, Result};

use crate::config::HivefuzzConfig;

/// Initialize a new fuzzing target.
///
/// Validates the target binary, sets up the corpus directory, and generates
/// a `hivefuzz.toml` configuration file.
pub async fn run(target_path: &str, corpus_path: Option<&str>) -> Result<()> {
    let target = Path::new(target_path);
    if !target.exists() {
        bail!("Target binary not found: {}", target_path);
    }

    tracing::info!("Validating target binary: {}", target_path);

    // TODO: Check if binary is instrumented (has AFL/coverage feedback)
    // TODO: Test-run the binary to verify it works
    // TODO: Check for ASAN instrumentation

    if let Some(corpus) = corpus_path {
        let corpus_dir = Path::new(corpus);
        if !corpus_dir.exists() {
            bail!("Corpus directory not found: {}", corpus);
        }
        let count = std::fs::read_dir(corpus_dir)?.count();
        tracing::info!("Found {} seed files in corpus", count);
    }

    // Generate hivefuzz.toml
    let config_path = Path::new("hivefuzz.toml");
    if config_path.exists() {
        bail!(
            "Configuration file already exists: {}. Delete it first to re-initialize.",
            config_path.display()
        );
    }

    let config_content = HivefuzzConfig::generate_default(target_path);
    std::fs::write(config_path, &config_content)?;
    tracing::info!("Generated configuration: {}", config_path.display());

    // Create output directories
    let output_dir = Path::new("output");
    if !output_dir.exists() {
        std::fs::create_dir_all(output_dir)?;
        tracing::info!("Created output directory: {}", output_dir.display());
    }

    // Create seeds directory if corpus wasn't specified
    if corpus_path.is_none() {
        let seeds_dir = Path::new("seeds");
        if !seeds_dir.exists() {
            std::fs::create_dir_all(seeds_dir)?;
            tracing::info!(
                "Created seeds directory: {} — add seed inputs here",
                seeds_dir.display()
            );
        }
    }

    tracing::info!("Target initialized successfully");
    tracing::info!(
        "Next: run `hivefuzz run --config hivefuzz.toml` to start fuzzing"
    );

    Ok(())
}
