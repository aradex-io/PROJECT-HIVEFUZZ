# HIVEFUZZ

**Autonomous Distributed Fuzzing Swarm — Leaderless Vulnerability Discovery**

HIVEFUZZ is a fully decentralized fuzzing swarm where every node is equal — no coordinator, no master, no single point of failure. Nodes discover each other, share coverage maps via gossip protocol, evolve mutation strategies based on collective intelligence, and automatically triage/deduplicate crashes.

## Architecture

```
         ┌──────────┐
         │  Node A   │
         │  Fuzzer   │
         │  Gossip   │
         │  Strategy │
    ┌────┴──────────┴────┐
    │                    │
┌───▼──────┐      ┌─────▼────┐
│  Node B  │◄────►│  Node C  │
│  Fuzzer  │gossip│  Fuzzer  │
│  Gossip  │      │  Gossip  │
│  Strategy│      │  Strategy│
└───┬──────┘      └─────┬────┘
    │    ┌──────────┐   │
    └───►│  Node D  │◄──┘
         │   ...    │
         └──────────┘
```

Every node runs identical software. No node is special. Communication is peer-to-peer gossip.

## Core Design Principles

- **No coordinator** — Every node makes autonomous decisions
- **Eventual consistency** — Coverage maps converge via gossip
- **Emergent specialization** — Nodes evolve unique mutation strategies
- **Crash-resilient** — Node death is normal, not an error
- **Cloud-native** — Designed for spot instances
- **Target-agnostic** — Swarm orchestration is independent of the fuzz target

## Project Structure

```
src/
├── main.rs              # CLI entry point
├── lib.rs               # Library root
├── fuzzer/              # Fuzzer engine abstraction layer
│   ├── mod.rs           # FuzzerBackend trait & types
│   ├── afl.rs           # AFL++ backend
│   ├── coverage.rs      # Coverage bitmap management
│   └── corpus.rs        # Corpus management & minimization
├── gossip/              # SWIM-based gossip protocol
│   ├── mod.rs           # Gossip protocol core
│   ├── membership.rs    # Peer discovery & failure detection
│   ├── transport.rs     # UDP/TCP transport layer
│   └── dissemination.rs # Coverage/corpus/crash dissemination
├── strategy/            # Evolutionary mutation strategy engine
│   ├── mod.rs           # Strategy types & evolution
│   └── fitness.rs       # Fitness tracking & bandit algorithms
├── crash/               # Crash triage & deduplication
│   ├── mod.rs           # Crash management
│   ├── dedup.rs         # Deduplication logic
│   └── scoring.rs       # Exploitability scoring
├── node/                # Node identity & lifecycle
│   ├── mod.rs           # Node struct & orchestration
│   └── identity.rs      # Ed25519 keypair identity
├── proto/               # Protocol buffer definitions
│   └── mod.rs
└── utils/               # Shared utilities
    └── mod.rs
```

## Phases

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | Single-Node Fuzzer Core | **In Progress** |
| Phase 1 | Gossip Protocol (SWIM-based) | Planned |
| Phase 2 | Evolutionary Strategy Engine | Planned |
| Phase 3 | Crash Triage & Deduplication | Planned |
| Phase 4 | Deployment & Operations | Planned |

## Tech Stack

- **Rust** — Node binary (performance-critical, memory-safe)
- **AFL++/libFuzzer/Honggfuzz** — Fuzzer backends (via FFI/subprocess)
- **Custom SWIM protocol** — Gossip layer
- **Protocol Buffers** — Message serialization
- **SQLite** — Per-node crash database
- **Prometheus + Grafana** — Observability
- **Docker + Terraform** — Deployment

## Quick Start

```bash
# Build
cargo build --release

# Initialize a target
hivefuzz init --target ./binary --corpus ./seeds/

# Start a node
hivefuzz run --target ./target.toml --seeds seed1.example.com:7878

# Start a local swarm (development)
hivefuzz dev --nodes 5 --target ./target.toml
```

## Performance Targets

| Metric | Target |
|--------|--------|
| Per-node gossip overhead | <5% CPU, <100KB/s network |
| Coverage convergence (10 nodes) | <60s |
| Crash propagation | <10s |
| Node join time | <5s |
| Scaling limit | 100+ nodes |

## License

See [LICENSE](LICENSE) for details.
