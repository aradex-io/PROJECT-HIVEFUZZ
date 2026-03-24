# HIVEFUZZ — Claude Code Rules

## Development Process

For every action, follow this process:

1. **Plan** — Define what needs to be done, break it into steps
2. **Critical Review** — Review the plan for risks, edge cases, and flaws. Refine before proceeding
3. **Finalize the Plan** — Lock in the approach after review
4. **Execute the Plan** — Implement the changes
5. **Verify Against Benchmarks** — Confirm the implementation meets the defined success criteria
6. **Test Cases** — If applicable, create test cases for all implemented pieces

## Project Context

- **Language:** Rust (2024 edition)
- **Purpose:** Decentralized fuzzing swarm — no coordinator, gossip-based peer-to-peer
- **Key modules:** `fuzzer/` (backend abstraction), `gossip/` (SWIM protocol), `strategy/` (evolutionary mutation), `crash/` (triage & dedup), `node/` (identity & lifecycle)

## Code Standards

- All public types and traits must have doc comments
- Every module should have unit tests for core logic
- Use `anyhow::Result` for fallible operations in application code
- Use `thiserror` for library-level error types
- Prefer `tracing` over `println!` for all logging
- Keep the `FuzzerBackend` trait backend-agnostic — no AFL-specific types in the interface

## Architecture Principles

- No coordinator node — every node is equal
- Eventual consistency via gossip — nodes don't wait for sync
- Crash-resilient — node death is normal, not an error
- Target-agnostic — swarm orchestration is independent of fuzz target
- Start simple (bincode/UDP), evolve later (protobuf/QUIC)

## Testing

- `cargo test` must pass before any commit
- Integration tests go in `tests/`
- Use `tempfile` for tests that need filesystem
- Benchmark coverage bitmap operations for performance regression
