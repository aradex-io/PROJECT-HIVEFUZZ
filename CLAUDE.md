# HIVEFUZZ — Claude Code Rules

## Development Process

For every non-trivial action, follow this process:

1. **Plan** — Define what needs to be done, break it into steps. Write the plan to `docs/planning/DDMMMYYYY-<slug>.md`
2. **Critical Review** — Review the plan for risks, edge cases, and flaws. Write findings to `docs/review/DDMMMYYYY-<slug>.md`
3. **Finalize the Plan** — Lock in the approach after review. Update the planning doc with `Status: APPROVED`
4. **Execute the Plan** — Implement the changes. Use the TodoWrite tool to track progress
5. **Verify Against Benchmarks** — `cargo test` must pass. Confirm the implementation meets the defined success criteria
6. **Test Cases** — Create unit tests for all new logic. Create integration tests for cross-module behavior
7. **Update Completed** — Mark completed items in `docs/completed/COMPLETED.md`

## Project Context

- **Language:** Rust (2024 edition)
- **Purpose:** Decentralized fuzzing swarm — no coordinator, gossip-based peer-to-peer
- **Key modules:**
  - `config/` — TOML-based target configuration
  - `fuzzer/` — backend abstraction (AFL++, coverage, corpus)
  - `gossip/` — SWIM protocol (membership, transport, dissemination)
  - `strategy/` — evolutionary mutation (fitness tracking, Exp3, mutator)
  - `crash/` — triage, dedup & scoring
  - `node/` — identity & lifecycle (main fuzz loop)
  - `commands/` — CLI subcommands (init, run, dev, status)

## Code Standards

- All public types and traits must have doc comments
- Every module must have unit tests for core logic
- Use `anyhow::Result` for fallible operations in application code (commands, node)
- Use `thiserror` for library-level error types (fuzzer, gossip, crash, strategy)
- Prefer `tracing` over `println!` for all logging
- Keep the `FuzzerBackend` trait backend-agnostic — no AFL-specific types in the interface
- Use `serde` derive for all types that cross module boundaries or hit the wire
- Minimize `unsafe` — use only when required by FFI (e.g., shared memory)

## Architecture Principles

- No coordinator node — every node is equal
- Eventual consistency via gossip — nodes don't wait for sync
- Crash-resilient — node death is normal, not an error
- Target-agnostic — swarm orchestration is independent of fuzz target
- Start simple (bincode/UDP), evolve later (protobuf/QUIC)
- Hot path awareness — coverage bitmap operations are performance-critical; avoid allocations in the fuzz loop

## Testing

- `cargo test` must pass before any commit
- Unit tests go in `#[cfg(test)] mod tests` within each source file
- Integration tests go in `tests/`
- Use `tempfile` for tests that need filesystem
- Use port 0 (`127.0.0.1:0`) for network tests to avoid port conflicts
- Benchmark coverage bitmap operations for performance regression

## Dependency Management

- Pin major versions in `Cargo.toml` (e.g., `tokio = { version = "1", ... }`)
- Justify new dependencies — prefer stdlib or existing deps when possible
- Use `features` to minimize compile time (only enable what's needed)
- Dev-only deps go in `[dev-dependencies]`

## Commit & Branch Conventions

- Commit messages: imperative mood, concise subject line
- Format: `<type>: <subject>` where type is one of: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`
- Branch names: `<type>/<short-description>` (e.g., `feat/gossip-transport`, `fix/crash-dedup`)

## Documentation Artifacts

All research, planning, and review documents follow a standardized structure.

### Directory Layout

```
docs/
├── PLAN.md              # Master roadmap (living document)
├── planning/            # Sprint plans, implementation designs
│   └── DDMMMYYYY-<slug>.md
├── review/              # Critical reviews, post-mortems
│   └── DDMMMYYYY-<slug>.md
├── reference/           # Stable reference docs (architecture, protocols)
│   └── <topic>.md
├── completed/           # Roadmap completion tracking
│   └── COMPLETED.md
```

### File Naming

- **Date format:** `DDMMMYYYY` (e.g., `24MAR2026`)
- **Slug:** lowercase, hyphen-separated (e.g., `sprint-1-plan`, `phase-0-review`)
- **Examples:**
  - `docs/planning/24MAR2026-sprint-1-plan.md`
  - `docs/review/24MAR2026-phase-0-review.md`
  - `docs/reference/gossip-protocol.md`

### Document Structure

Every planning/review document should include:

```markdown
# Title
**Date:** DDMMMYYYY
**Status:** DRAFT | IN REVIEW | APPROVED | COMPLETED
**Author:** <who created it>

## Context
<Why this document exists>

## Content
<The actual content>

## Decisions
<Key decisions made, with rationale>

## Open Questions
<Unresolved items>
```
