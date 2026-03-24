# HIVEFUZZ — Implementation Plan

## Critical Review & Refined Execution Strategy

This plan has been reviewed for feasibility, risk, and sequencing. Key refinements
from the initial spec are noted inline.

---

## Phase 0: Single-Node Fuzzer Core

**Goal:** A working single-node fuzzer with the abstraction layer needed for distributed operation.

### 0.1 — Fuzzer Backend Abstraction (HIGH PRIORITY)
- [x] Define `FuzzerBackend` trait
- [x] Define `FuzzResult`, `CrashInfo`, `Severity` types
- [x] Define `TargetConfig` for target specification
- [ ] Implement AFL++ backend via fork-server (subprocess + shared memory)
- [ ] Implement libFuzzer backend (in-process, via shared library)
- [ ] Add integration tests with a known-vulnerable test binary

**Critical review note:** The initial plan lists Honggfuzz as a third backend.
This is premature — AFL++ and libFuzzer cover >90% of use cases. Defer Honggfuzz
to post-MVP. Starting with AFL++ subprocess wrapping is the fastest path; we
don't need FFI initially — `afl-fuzz` can be orchestrated via its `-S`/`-M` mode
or we can use `afl-showmap` for single-input execution.

### 0.2 — Coverage Bitmap Management (HIGH PRIORITY)
- [x] Implement 64KB AFL-style bitmap with merge/diff operations
- [x] Implement bloom filter digest for bandwidth-efficient gossip
- [x] Add hit count classification (AFL buckets)
- [ ] Add bitmap compression for serialization (run-length encoding)
- [ ] Benchmark bloom filter false positive rates and tune parameters

**Critical review note:** The 1KB bloom filter is an aggressive size. We should
benchmark false positive rates at realistic coverage levels (5K-20K edges) and
be prepared to increase to 2-4KB if needed. False negatives (missing novel
coverage) are more costly than the extra bandwidth.

### 0.3 — Corpus Management (HIGH PRIORITY)
- [x] Implement corpus storage with content-hash deduplication
- [x] Track provenance (which node, which mutation)
- [x] Priority queue for dissemination (novel edges first)
- [ ] Implement corpus minimization (afl-cmin equivalent)
- [ ] Add seed loading from directory
- [ ] Implement corpus serialization for gossip transfer

**Critical review note:** Corpus minimization is expensive. For Phase 0, simple
dedup-by-coverage is sufficient. Full minimization can run as a background task
in Phase 1.

### 0.4 — Crash Management (MEDIUM PRIORITY)
- [x] Implement multi-level crash deduplication (stack hash + ASAN class)
- [x] Implement exploitability scoring from ASAN reports
- [x] Implement CWE suggestion
- [x] In-memory crash database with summary statistics
- [ ] Add SQLite persistence layer
- [ ] Implement crash minimization (delta debugging)
- [ ] Parse ASAN stack traces for structured data

**Critical review note:** Crash minimization via delta debugging is a nice-to-have
for Phase 0. The critical path is: detect crash → deduplicate → store → report.
Minimization can run lazily.

### 0.5 — Node Identity (DONE)
- [x] UUID-based node identification
- [x] Ed25519 keypair generation
- [x] Message signing and verification
- [x] Public identity serialization for peer exchange

### 0.6 — CLI & Configuration
- [x] CLI with init/run/dev/status subcommands
- [ ] TOML-based target configuration file
- [ ] Validate target binary (instrumentation check)
- [ ] Seed corpus loading and validation

---

## Phase 1: Gossip Protocol

**Goal:** Nodes can discover each other, share coverage, and propagate crashes
without any central coordinator.

### 1.1 — Transport Layer (HIGH PRIORITY)
- [x] Transport abstraction (UDP + TCP)
- [ ] UDP socket implementation for gossip messages
- [ ] TCP connection pool for bulk transfers
- [ ] Message framing and serialization (bincode initially, protobuf later)
- [ ] Message signing (Ed25519) for all outgoing messages
- [ ] Signature verification for incoming messages

**Critical review note:** Start with bincode over UDP/TCP. Protobuf adds build
complexity (prost-build, .proto files). Migrate to protobuf in Phase 2 when the
wire format stabilizes.

### 1.2 — SWIM Membership Protocol (HIGH PRIORITY)
- [x] Membership list with peer state tracking
- [x] Peer selection for gossip (random fanout)
- [x] Failure detection state machine (Alive → Suspected → Dead)
- [ ] Implement ping/ping-ack protocol
- [ ] Implement indirect ping (ping-req) for robustness
- [ ] Implement join procedure (contact seed → receive peers → announce)
- [ ] Implement voluntary leave (broadcast departure)
- [ ] Implement suspicion timeout and death confirmation
- [ ] Integration test: 3-5 nodes discover each other

**Critical review note:** SWIM's indirect ping is critical for avoiding false
positives in failure detection (especially in cloud environments with variable
latency). Don't skip this — it's the difference between a stable and unstable
swarm.

### 1.3 — Coverage Dissemination (HIGH PRIORITY)
- [x] Disseminator skeleton with gossip round structure
- [ ] Send bloom filter digest each gossip round
- [ ] Compare received digests against local coverage
- [ ] Request full coverage bitmap when bloom filter indicates novelty
- [ ] Send novel corpus entries with coverage updates
- [ ] Track which edges have been shared (avoid re-sending)
- [ ] Benchmark: convergence time for N nodes

**Critical review note:** The spec says "5-10 second gossip interval." Start
with 5s. At fanout=3, a 10-node swarm converges in ~3 rounds (15s). This is
acceptable. If convergence is too slow, reduce interval or increase fanout.
Watch bandwidth — at 100 nodes, 1KB/digest × 3 peers × every 5s = 600 B/s
per node, well within budget.

### 1.4 — Crash Dissemination (HIGH PRIORITY)
- [ ] Immediate crash propagation (don't wait for next gossip round)
- [ ] Crash fingerprint gossip (send fingerprint before full crash data)
- [ ] Full crash data transfer via TCP (crash inputs can be large)
- [ ] Swarm-wide crash deduplication via fingerprint comparison

### 1.5 — Bandwidth Management (MEDIUM PRIORITY)
- [ ] Measure actual gossip overhead per node
- [ ] Implement corpus entry size limits for piggyback
- [ ] Lazy corpus transfer (only send when peer requests)
- [ ] Compression for TCP bulk transfers (zstd)

---

## Phase 2: Evolutionary Strategy Engine

**Goal:** Nodes independently evolve mutation strategies, share fitness data,
and the swarm collectively explores the strategy space efficiently.

### 2.1 — Fitness Tracking (HIGH PRIORITY)
- [x] Per-mutation-type statistics tracking
- [x] Rolling window with decay
- [x] Fitness score calculation
- [ ] Wire up to actual fuzzing loop (record every execution)
- [ ] Add time-based fitness (edges/second, not just edges/execution)

**Critical review note:** The spec's rolling window of 100K executions is
reasonable for AFL++-class speed (~1K exec/s on complex targets). For simpler
targets running at 10K+ exec/s, the window should be configurable.

### 2.2 — Strategy Evolution (HIGH PRIORITY)
- [x] Exp3 bandit algorithm implementation
- [x] Strategy blending from peer data
- [x] Noise injection for diversity
- [ ] Connect Exp3 to actual strategy weight updates
- [ ] Implement evolution interval (every N executions)
- [ ] Add minimum weight floor (never fully abandon any mutation type)
- [ ] Track strategy generation counter for gossip

### 2.3 — Swarm Strategy Exchange (MEDIUM PRIORITY)
- [ ] Add strategy fitness data to gossip messages
- [ ] Strategy adoption policy (blend if peer fitness >2x local)
- [ ] Strategy diversity monitoring across swarm
- [ ] Speciation enforcement (add noise if swarm converges too much)

**Critical review note:** The 2x fitness threshold for adoption is a
hyperparameter that needs tuning. Too low = premature convergence. Too high =
too slow to adopt good strategies. Start with 2x and instrument to find the
right value empirically.

### 2.4 — Validation (MEDIUM PRIORITY)
- [ ] Benchmark: evolved vs static strategies on known targets
- [ ] Visualize strategy evolution over time
- [ ] Measure strategy diversity across swarm

---

## Phase 3: Crash Triage & Deduplication

**Goal:** Automated classification, deduplication, and scoring of crashes
across the entire swarm.

### 3.1 — Advanced Deduplication
- [ ] Coverage-bitmap-based crash clustering (not just stack hash)
- [ ] ASAN report structural comparison
- [ ] DBSCAN clustering of crash feature vectors
- [ ] "Likely N unique bugs" estimation

### 3.2 — Distributed Crash Minimization
- [ ] Delta debugging implementation
- [ ] Distributed: any node can minimize any crash
- [ ] Gossip "minimized" flag to prevent redundant work
- [ ] Minimized input replaces original in crash DB

### 3.3 — Report Generation
- [ ] Per-bug report: reproducer, stack trace, severity, CWE
- [ ] Swarm summary: coverage timeline, per-node contribution
- [ ] JSON + HTML report formats

---

## Phase 4: Deployment & Operations

**Goal:** Production-ready deployment and observability.

### 4.1 — Container Packaging
- [x] Dockerfile (multi-stage build)
- [ ] Docker Compose for local multi-node testing
- [ ] Target configuration via environment variables
- [ ] Health check endpoint

### 4.2 — Observability
- [ ] Prometheus metrics endpoint per node
- [ ] Key metrics: exec/s, coverage, crashes, gossip rounds, peer count
- [ ] Grafana dashboard template
- [ ] Strategy heatmap visualization

### 4.3 — Cloud Deployment
- [ ] Terraform templates for spot instance fleets (AWS first)
- [ ] Auto-scaling based on edge discovery rate
- [ ] Spot interruption handling (graceful leave)
- [ ] Persistent corpus sync to S3

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| AFL++ integration harder than expected | Medium | High | Start with subprocess wrapping, not FFI. Use afl-showmap for single-input execution. |
| Gossip protocol instability at scale | Medium | High | Extensive testing at 10, 50, 100 nodes. SWIM is well-understood — stick to the paper. |
| Strategy evolution doesn't help | Medium | Medium | Run controlled experiments early. Uniform strategy is the fallback. |
| Bloom filter false positives waste bandwidth | Low | Medium | Benchmark early, size up if needed. Full bitmap fallback is always available. |
| Crash dedup misses duplicates | Medium | Low | Multiple dedup layers (stack hash + coverage + ASAN class). Over-reporting is better than missing bugs. |

---

## Immediate Next Steps (Sprint 1)

1. **AFL++ subprocess integration** — Run afl-showmap on a test binary, parse coverage output
2. **TOML target config** — Load target configuration from file
3. **UDP gossip transport** — Two nodes exchanging ping/pong
4. **Integration test harness** — Compile a known-vulnerable test binary with AFL instrumentation
5. **CI setup** — GitHub Actions: build + test on every push

---

## Architecture Decision Records

### ADR-001: Rust over Go
Go's `memberlist` library provides a battle-tested SWIM implementation. However,
Rust offers: zero-cost abstractions for bitmap operations (critical hot path),
memory safety without GC pauses (important for consistent exec/s), and direct
compatibility with AFL++'s C shared memory interface. The gossip protocol is
simple enough to implement from scratch.

### ADR-002: Bincode before Protobuf
Starting with bincode (serde-native) for serialization. It's faster, requires no
code generation, and the wire format doesn't need to be stable yet. Migrate to
protobuf when: (a) we need cross-language compatibility, or (b) we need schema
evolution for backward-compatible protocol changes.

### ADR-003: Bloom filter for coverage digest
A bloom filter trades accuracy for bandwidth. At 1KB with 3 hash functions, we
get ~1% false positive rate for 1000 edges. This is acceptable — a false positive
only triggers an unnecessary coverage bitmap exchange, not data loss. If false
positive rate becomes problematic at high coverage counts, we can increase the
bloom filter size.

### ADR-004: Exp3 for strategy evolution
Exp3 (Exponential-weight algorithm for Exploration and Exploitation) is chosen
over UCB1 because: (a) it handles adversarial reward distributions (coverage
frontier is non-stationary), (b) it naturally balances exploration/exploitation,
(c) it's simple to implement. If Exp3 proves too noisy, UCB1 with sliding window
is the fallback.
