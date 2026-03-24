use anyhow::Result;

use crate::fuzzer::afl::AflBackend;
use crate::fuzzer::TargetConfig;
use crate::gossip::GossipConfig;
use crate::node::Node;

/// Start a HIVEFUZZ node.
pub async fn run(
    target_config_path: &str,
    seed_nodes: &[String],
    bind_addr: &str,
    port: u16,
) -> Result<()> {
    // TODO: Load target config from TOML file
    let target_config = TargetConfig {
        binary_path: target_config_path.to_string(),
        arguments: vec![],
        timeout: std::time::Duration::from_secs(5),
        memory_limit_mb: 256,
        input_mode: crate::fuzzer::InputMode::Stdin,
        dictionary: None,
    };

    let gossip_config = GossipConfig {
        bind_addr: format!("{}:{}", bind_addr, port).parse()?,
        seed_nodes: seed_nodes
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<_>, _>>()?,
        ..Default::default()
    };

    let backend = AflBackend::new(uuid::Uuid::new_v4());
    let mut node = Node::new(Box::new(backend), gossip_config);

    node.init(&target_config).await?;
    node.run().await?;

    Ok(())
}
