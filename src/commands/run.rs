use std::path::Path;

use anyhow::Result;

use crate::config::HivefuzzConfig;
use crate::fuzzer::afl::AflBackend;
use crate::gossip::GossipConfig;
use crate::node::Node;

/// Start a HIVEFUZZ node.
pub async fn run(
    config_path: &str,
    seed_nodes: &[String],
    bind_addr: &str,
    port: u16,
) -> Result<()> {
    // Load target configuration from TOML
    let config = HivefuzzConfig::load(Path::new(config_path))?;
    tracing::info!(
        "Loaded config from {}: target={}",
        config_path,
        config.target.binary
    );

    let target_config = config.to_target_config();

    let gossip_config = GossipConfig {
        bind_addr: format!("{}:{}", bind_addr, port).parse()?,
        seed_nodes: seed_nodes
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<_>, _>>()?,
        gossip_interval: std::time::Duration::from_secs(config.swarm.gossip_interval_secs),
        fanout: config.swarm.fanout,
        ..Default::default()
    };

    let backend = AflBackend::new(uuid::Uuid::new_v4());
    let mut node = Node::new(Box::new(backend), gossip_config);

    node.init(&target_config).await?;
    node.run().await?;

    Ok(())
}
