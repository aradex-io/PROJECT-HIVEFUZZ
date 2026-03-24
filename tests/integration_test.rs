/// Integration tests for HIVEFUZZ components working together.
use hivefuzz::config::HivefuzzConfig;
use hivefuzz::crash::CrashDatabase;
use hivefuzz::fuzzer::corpus::CorpusManager;
use hivefuzz::fuzzer::coverage::CoverageBitmap;
use hivefuzz::gossip::transport::{Transport, TransportConfig};
use hivefuzz::gossip::GossipMessage;
use hivefuzz::strategy::fitness::FitnessTracker;
use hivefuzz::strategy::mutator::apply_mutation;
use hivefuzz::strategy::{MutationStrategy, MutationType};

use std::io::Write;
use uuid::Uuid;

#[test]
fn test_config_to_target_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("hivefuzz.toml");
    let mut file = std::fs::File::create(&config_path).unwrap();
    write!(file, "{}", HivefuzzConfig::generate_default("/bin/true")).unwrap();

    let config = HivefuzzConfig::load(&config_path).unwrap();
    let target = config.to_target_config();

    assert_eq!(target.binary_path, "/bin/true");
    assert_eq!(target.timeout, std::time::Duration::from_millis(5000));
    assert_eq!(target.memory_limit_mb, 256);
}

#[test]
fn test_mutation_coverage_corpus_pipeline() {
    let node_id = Uuid::new_v4();
    let mut corpus = CorpusManager::new(node_id, 1000);
    let mut global_coverage = CoverageBitmap::new();
    let strategy = MutationStrategy::default_uniform();
    let mut fitness = FitnessTracker::new(100_000);

    // Simulate a fuzzing iteration
    let seed = b"test input data for fuzzing";
    corpus.add(seed.to_vec(), 0, None, None);

    // Mutate the seed
    let mutation = strategy.select_mutation();
    let mutated = apply_mutation(seed, mutation);
    assert!(!mutated.is_empty());

    // Simulate coverage discovery
    let mut exec_coverage = CoverageBitmap::new();
    exec_coverage.as_bytes_mut()[42] = 1;
    exec_coverage.as_bytes_mut()[100] = 3;

    let new_edges = global_coverage.merge(&exec_coverage);
    assert_eq!(new_edges, 2);

    // Add to corpus if novel
    if new_edges > 0 {
        let added = corpus.add(mutated, new_edges, Some(format!("{:?}", mutation)), None);
        assert!(added);
    }

    // Record fitness
    fitness.record(mutation, new_edges, false);
    assert!(fitness.fitness(&mutation) > 0.0);

    assert_eq!(corpus.len(), 2);
    assert_eq!(global_coverage.count_edges(), 2);
}

#[test]
fn test_crash_dedup_across_nodes() {
    let node1 = Uuid::new_v4();
    let node2 = Uuid::new_v4();
    let mut db = CrashDatabase::new();

    let crash = hivefuzz::fuzzer::CrashInfo {
        input: vec![1, 2, 3],
        signal: 11,
        stack_hash: 0xDEADBEEF,
        stack_trace: None,
        asan_report: Some(
            "ERROR: AddressSanitizer: heap-buffer-overflow on WRITE".to_string(),
        ),
        severity: hivefuzz::fuzzer::Severity::Critical,
    };

    // First node reports crash — novel
    assert!(db.record(crash.clone(), node1));
    assert_eq!(db.len(), 1);

    // Second node reports same crash — duplicate
    assert!(!db.record(crash.clone(), node2));
    assert_eq!(db.len(), 1);

    // Different crash — novel
    let crash2 = hivefuzz::fuzzer::CrashInfo {
        stack_hash: 0xCAFEBABE,
        ..crash
    };
    assert!(db.record(crash2, node1));
    assert_eq!(db.len(), 2);

    let summary = db.summary();
    assert_eq!(summary.total, 2);
    assert_eq!(summary.critical, 2);
}

#[test]
fn test_coverage_bloom_digest_novelty() {
    let mut coverage_a = CoverageBitmap::new();
    let mut coverage_b = CoverageBitmap::new();

    // Node A has some edges
    coverage_a.as_bytes_mut()[10] = 1;
    coverage_a.as_bytes_mut()[20] = 1;

    // Node B has different edges
    coverage_b.as_bytes_mut()[30] = 1;
    coverage_b.as_bytes_mut()[40] = 1;

    let digest_a = coverage_a.to_bloom_digest();
    let digest_b = coverage_b.to_bloom_digest();

    // Both should detect novelty in the other
    assert!(digest_a.likely_has_novel(&digest_b));
    assert!(digest_b.likely_has_novel(&digest_a));

    // Merged coverage should have all edges
    let new_edges = coverage_a.merge(&coverage_b);
    assert_eq!(new_edges, 2);
    assert_eq!(coverage_a.count_edges(), 4);
}

#[test]
fn test_strategy_evolution_improves_weights() {
    let mut strategy = MutationStrategy::default_uniform();
    let mut fitness = FitnessTracker::new(100_000);
    let exp3 = hivefuzz::strategy::fitness::Exp3Updater::new(0.1, 0.01);

    // Simulate: BitFlip1 finds edges, RandomByte doesn't
    for _ in 0..1000 {
        fitness.record(MutationType::BitFlip1, 1, false);
        fitness.record(MutationType::RandomByte, 0, false);
    }

    let initial_bf1 = strategy.weights[&MutationType::BitFlip1];
    let initial_rb = strategy.weights[&MutationType::RandomByte];

    // They start equal
    assert!((initial_bf1 - initial_rb).abs() < 0.01);

    // Evolve
    let all_fitness = fitness.all_fitness();
    exp3.update_weights(&mut strategy.weights, &all_fitness);

    // BitFlip1 should now have higher weight
    assert!(strategy.weights[&MutationType::BitFlip1] > strategy.weights[&MutationType::RandomByte]);
}

#[tokio::test]
async fn test_gossip_transport_multicast() {
    // Simulate 3-node gossip: node1 sends to node2 and node3
    let mut t1 = Transport::new(TransportConfig {
        udp_addr: "127.0.0.1:0".parse().unwrap(),
        max_udp_size: 65507,
    });
    let mut t2 = Transport::new(TransportConfig {
        udp_addr: "127.0.0.1:0".parse().unwrap(),
        max_udp_size: 65507,
    });
    let mut t3 = Transport::new(TransportConfig {
        udp_addr: "127.0.0.1:0".parse().unwrap(),
        max_udp_size: 65507,
    });

    let _rx1 = t1.start().await.unwrap();
    let mut rx2 = t2.start().await.unwrap();
    let mut rx3 = t3.start().await.unwrap();

    let addr2 = t2.local_addr().unwrap();
    let addr3 = t3.local_addr().unwrap();

    let sender = Uuid::new_v4();

    // Node1 sends Join to both node2 and node3
    let join_msg = GossipMessage::Join {
        node_id: sender,
        addr: t1.local_addr().unwrap(),
    };

    t1.send_udp(&join_msg, addr2).await.unwrap();
    t1.send_udp(&join_msg, addr3).await.unwrap();

    // Both should receive it
    let msg2 = tokio::time::timeout(std::time::Duration::from_secs(2), rx2.recv())
        .await
        .unwrap()
        .unwrap();

    let msg3 = tokio::time::timeout(std::time::Duration::from_secs(2), rx3.recv())
        .await
        .unwrap()
        .unwrap();

    assert!(matches!(msg2.message, GossipMessage::Join { .. }));
    assert!(matches!(msg3.message, GossipMessage::Join { .. }));
}
