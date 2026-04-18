# Roadmap Refresh & Forward Objectives
**Date:** 18APR2026
**Status:** APPROVED
**Author:** Claude Code (branch `claude/project-planning-4L5Yv`)

## Context

HIVEFUZZ has reached the end of its bootstrap period. Sprint 1 (24MAR2026)
delivered the single-node core (TOML config, AFL++ subprocess, fuzz loop,
mutator, UDP transport, dissemination skeleton, CI). Sprint 2 (25MAR2026, status
APPROVED) scoped the SWIM control loop, join procedure, seed corpus loading,
and gossip/fuzz-loop integration.

This document is a roadmap-level refresh. It restates objectives against the
current state of the tree, defines a Minimum Viable Swarm (MVS) target, and
sequences the next three sprints. It is **not** a line-by-line re-plan —
`docs/PLAN.md` remains the canonical phase breakdown; this document updates
priorities, success criteria, and the Sprint 3+ cadence.

## Current State Snapshot

Derived from `docs/completed/COMPLETED.md` and the `src/` tree.

| Area | State |
|------|-------|
| Fuzzer backend abstraction | Trait + AFL++ subprocess landed; libFuzzer stub absent |
| Coverage bitmap | 64KB AFL bitmap, bloom digest, hit-count buckets landed; no RLE, no FP benchmarks |
| Corpus | Content-hash dedup, provenance, priority queue landed; no minimization, no gossip serialization |
| Crash | Multi-level dedup, exploitability/CWE scoring, in-memory DB landed; no SQLite, no minimization, no structured ASAN parse |
| Node identity | Ed25519 keypair, signing primitives landed |
| CLI/config | `init`, `run`, `dev`, `status` + TOML load/validate/generate landed |
| Strategy | Mutator (21 ops), fitness window, Exp3 landed; **not wired to a real evolution tick** |
| Gossip transport | UDP + bincode + async dispatch landed; TCP pool and signing absent |
| SWIM | Membership list + state machine landed; **no ping loop, no join, no leave** |
| Dissemination | Digest/corpus/crash send skeletons landed; no compare/request loop |
| Deployment | Dockerfile + CI landed; no compose, metrics, dashboards, or Terraform |

## Objectives (Forward-Looking)

### Primary Objective — Minimum Viable Swarm (MVS)
A 3-node local swarm discovers each other, gossips coverage, and converges on a
shared corpus while independently fuzzing the same target.

The MVS is the first externally demonstrable milestone. Everything prior has
been infrastructure.

**MVS Acceptance Tests (all must pass):**
1. `hivefuzz dev --nodes 3 --target <toml>` starts three nodes that join each other within 5s.
2. Within 60s, every node's coverage bitmap is within 1% of the union.
3. Killing one node leaves the other two operational; its absence is detected within 3×gossip_interval.
4. A crash discovered by one node appears in every node's crash DB within 10s.
5. `hivefuzz status` reports consistent peer count and coverage totals across nodes.

### Secondary Objectives
- **Observability MVP** — Prometheus `/metrics` endpoint exposing exec/s, edges, crashes, peer count, gossip rounds. Needed to debug MVS, not optional.
- **Durability** — SQLite-backed crash DB so a restart doesn't lose findings.
- **Strategy-in-the-loop** — Exp3 weights actually update from live fitness and influence mutation selection (currently dead code path).

### Explicit Non-Goals (this cycle)
- Honggfuzz backend (deferred post-MVP per ADR review).
- libFuzzer backend (unblocked but not on MVS critical path; schedule after observability).
- Terraform / cloud deployment (Phase 4 — defer until MVS runs clean in compose).
- DBSCAN crash clustering / ML-grade triage.
- Protobuf migration (stay on bincode per ADR-002).

## Target Metrics

The performance targets in `README.md` remain binding. Treated as Sprint 3+
exit criteria:

| Metric | Target | How measured |
|--------|--------|--------------|
| Per-node gossip overhead | <5% CPU, <100KB/s | Prometheus + `top`/`iftop` during MVS test |
| Coverage convergence (3 nodes) | <30s | Integration test asserts digest equality |
| Coverage convergence (10 nodes) | <60s | Compose-based load test |
| Crash propagation | <10s | Integration test: inject crash, time to peer DB |
| Node join time | <5s | SWIM join integration test |
| Fuzz-loop throughput regression | <10% vs single-node baseline | Before/after exec/s on same target |

## Sprint Sequence

### Sprint 3 — SWIM Completion & Gossip Convergence  *(≈1–2 weeks)*
**Theme:** Finish what Sprint 2 scoped; make nodes *actually* agree.

1. Ping/PingAck loop + indirect ping (`src/gossip/swim.rs`).
2. Join procedure end-to-end against a seed list.
3. Voluntary leave + graceful shutdown broadcast.
4. Dissemination compare-and-request: on digest receipt, diff against local bloom, pull novel coverage/corpus via TCP.
5. TCP connection pool for bulk transfers (coverage bitmap, corpus entry, crash data).
6. Ed25519 message signing on every outgoing gossip frame; verify on ingest; drop unsigned in strict mode.
7. Integration test: 3 nodes on loopback converge; kill one and confirm failure detection.

**Exit criteria:** MVS acceptance tests 1–3 pass.

### Sprint 4 — Durability, Metrics & Strategy-in-Loop  *(≈1 week)*
**Theme:** Make the swarm observable and make strategy actually drive mutation.

1. SQLite persistence for `CrashDb`; load on startup, flush on shutdown, write-through on new crash.
2. Seed corpus loading from `--seeds` directory (skip empty / >1MB) + validation.
3. Wire Exp3: per-execution fitness recording, evolution tick every N execs, minimum weight floor, generation counter.
4. Prometheus `/metrics` endpoint in `src/node/mod.rs`: exec/s, edges, crashes (by severity), peer count, gossip rounds, strategy generation.
5. Target binary instrumentation sniff in `hivefuzz init` (AFL marker detection).
6. Bloom filter false-positive benchmark at 1K / 5K / 10K / 20K edges; bump size if >3% FP.

**Exit criteria:** MVS acceptance tests 4–5 pass. Metrics scraped. Restart preserves crash DB.

### Sprint 5 — Scale & Triage  *(≈1–2 weeks)*
**Theme:** Move from "3 nodes on loopback" to "10 nodes in compose" and start real crash triage.

1. Docker Compose topology for N nodes sharing a seed; health-check endpoint.
2. Corpus minimization pass as a background task (simple dedup-by-coverage; full afl-cmin deferred).
3. Structured ASAN stack parsing for better dedup inputs.
4. Delta-debug crash minimization (opportunistic, runs on idle worker).
5. Grafana dashboard template (exec/s, coverage timeline, peer map, crash feed).
6. Bandwidth instrumentation to verify targets.

**Exit criteria:** 10-node compose swarm sustained for 30 minutes; convergence target met; dashboard renders.

## Cross-Cutting Engineering Discipline

- **Testing** — every sprint adds an integration test tied to a success criterion. A sprint with no new integration test is suspect.
- **CI** — keep `cargo test && cargo clippy -D warnings && cargo fmt --check` green. Add `-D warnings` to clippy enforcement in Sprint 3.
- **Hot path discipline** — before any change in `fuzzer/coverage.rs` or the fuzz loop, note expected allocation/exec-cycle impact. Benchmark if non-obvious.
- **Docs cadence** — every sprint gets a `DDMMMYYYY-sprint-N-*.md` plan + review. `COMPLETED.md` updated at sprint close.

## Decisions

1. **MVS is the organizing milestone.** Everything not on the MVS critical path is deferred or explicitly labelled optional in its sprint.
2. **Observability is promoted from Phase 4 to Sprint 4.** Debugging MVS convergence without metrics is a guessing game; cost of adding the endpoint now is low.
3. **libFuzzer and Honggfuzz stay deferred.** The AFL++ subprocess backend is sufficient for all MVS acceptance tests.
4. **Message signing moves into Sprint 3** (was Phase 2). Unsigned gossip is a correctness risk for failure detection once multiple nodes are live; cheaper to bake in now than retrofit.
5. **SQLite persistence moves into Sprint 4** (was Phase 0 tail). A crash DB that evaporates on restart is not acceptable for a swarm of preemptible nodes.
6. **Strategy-in-the-loop is explicitly flagged.** Exp3 exists but does not currently influence fuzzing. Sprint 4 closes this gap; without it, Phase 2 is theater.

## Open Questions

1. **Sprint 2 execution status.** The plan is APPROVED but `COMPLETED.md` still shows SWIM loop / join items unchecked. Sprint 3 assumes Sprint 2 is *not yet* merged. If it partially landed, fold the residual into Sprint 3 scope rather than duplicating.
2. **Test-binary provenance.** MVS tests need a known-vulnerable AFL-instrumented target reproducibly. Options: (a) commit a tiny C program + build script, (b) pull from a public fuzzer-benchmark suite. Decide before Sprint 3 integration tests are written.
3. **Compose vs. loopback for 10-node test.** Docker-in-CI is fiddly; consider running the 10-node test locally/nightly rather than on every PR.
4. **Failure-detection tuning under loopback.** Loopback has ~0ms latency, which may make suspicion timeouts trigger too aggressively or too leniently compared to WAN. Pick timeouts that are sane under both; document the assumption.
