# Sprint 1: Core Implementation
**Date:** 24MAR2026
**Status:** COMPLETED
**Author:** Claude Code

## Context

Sprint 1 implements the critical path from the master roadmap: TOML configuration,
AFL++ subprocess integration, the main fuzzing loop, UDP gossip transport,
coverage/crash dissemination, and CI.

## Scope

### Step 1: TOML Target Configuration
- Created `src/config.rs` with `HivefuzzConfig` struct
- Updated `hivefuzz init` to generate `hivefuzz.toml`
- Updated `hivefuzz run` to load config from TOML
- 6 unit tests covering parsing, validation, and generation

### Step 2: AFL++ Subprocess Integration
- Rewrote `src/fuzzer/afl.rs` with real subprocess execution
- `afl-showmap` integration for coverage-guided execution
- Direct execution fallback when AFL++ is not installed
- ASAN report extraction and crash scoring
- Automatic working directory management with cleanup

### Step 3: Main Fuzzing Loop
- Implemented in `src/node/mod.rs`
- Select input → mutate → execute → process coverage → process crashes
- Fitness tracking and periodic Exp3 strategy evolution
- Graceful shutdown with Ctrl+C handler and final statistics

### Step 4: Mutation Engine
- Created `src/strategy/mutator.rs` with all 21 mutation operators
- Handles edge cases: empty input, single byte, variable-length data
- 4 unit tests covering all mutation types

### Step 5: UDP Gossip Transport
- Rewrote `src/gossip/transport.rs` with real tokio UDP sockets
- Async send/receive with bincode serialization
- Channel-based message dispatch to handlers
- 2 tests: send/receive and ping/pong protocol

### Step 6: Coverage & Crash Dissemination
- Rewrote `src/gossip/dissemination.rs` with real gossip round logic
- Coverage digest broadcasting to fanout targets
- Corpus entry piggybacking (under size limit)
- Crash alert propagation

### Step 7: CI & Integration Tests
- Created `.github/workflows/ci.yml` (build, test, clippy, fmt)
- Created `tests/integration_test.rs` with 6 end-to-end tests
- Tests cover: config roundtrip, mutation-coverage pipeline,
  crash dedup, bloom filter novelty, strategy evolution, gossip multicast

## Results

- **35 tests passing** (29 unit + 6 integration)
- **0 compiler errors, 3 pre-existing warnings** (dead code in stubbed fields)
- All modules compile and link correctly

## Decisions

1. AFL++ is optional — backend gracefully degrades to direct execution without coverage
2. Used port 0 for network tests to avoid CI port conflicts
3. Deferred TCP bulk transfer to Phase 2
4. Deferred message signing to Phase 2 (transport works unsigned for now)

## Open Questions

1. Should we add `cargo clippy` enforcement in CI with `-D warnings`?
2. What's the minimum viable test binary for AFL++ integration testing?
3. Should the fuzz loop be synchronous or fully async with tokio::spawn?
