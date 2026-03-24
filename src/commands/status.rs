use anyhow::Result;

/// Query a node for its status and swarm information.
pub async fn run(node_addr: &str) -> Result<()> {
    tracing::info!("Querying node at {}", node_addr);

    // TODO: Connect to node's status endpoint
    // TODO: Display: node ID, uptime, executions/sec, coverage, crashes
    // TODO: Display: known peers, swarm size, gossip round

    tracing::warn!("Status command not yet implemented");
    Ok(())
}
