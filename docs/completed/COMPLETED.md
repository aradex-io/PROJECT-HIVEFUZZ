# HIVEFUZZ — Completed Roadmap Items

Tracks completed items from the master roadmap (`docs/PLAN.md`).

---

## Phase 0: Single-Node Fuzzer Core

### 0.1 — Fuzzer Backend Abstraction
- [x] Define `FuzzerBackend` trait
- [x] Define `FuzzResult`, `CrashInfo`, `Severity` types
- [x] Define `TargetConfig` for target specification
- [x] Implement AFL++ backend (subprocess + direct execution modes)
- [ ] Implement libFuzzer backend (in-process, via shared library)
- [ ] Add integration tests with a known-vulnerable test binary

### 0.2 — Coverage Bitmap Management
- [x] Implement 64KB AFL-style bitmap with merge/diff operations
- [x] Implement bloom filter digest for bandwidth-efficient gossip
- [x] Add hit count classification (AFL buckets)
- [ ] Add bitmap compression for serialization (run-length encoding)
- [ ] Benchmark bloom filter false positive rates and tune parameters

### 0.3 — Corpus Management
- [x] Implement corpus storage with content-hash deduplication
- [x] Track provenance (which node, which mutation)
- [x] Priority queue for dissemination (novel edges first)
- [ ] Implement corpus minimization (afl-cmin equivalent)
- [ ] Add seed loading from directory
- [ ] Implement corpus serialization for gossip transfer

### 0.4 — Crash Management
- [x] Implement multi-level crash deduplication (stack hash + ASAN class)
- [x] Implement exploitability scoring from ASAN reports
- [x] Implement CWE suggestion
- [x] In-memory crash database with summary statistics
- [ ] Add SQLite persistence layer
- [ ] Implement crash minimization (delta debugging)

### 0.5 — Node Identity
- [x] UUID-based node identification
- [x] Ed25519 keypair generation
- [x] Message signing and verification
- [x] Public identity serialization for peer exchange

### 0.6 — CLI & Configuration
- [x] CLI with init/run/dev/status subcommands
- [x] TOML-based target configuration file (`hivefuzz.toml`)
- [x] Config validation (binary existence, parameter sanity)
- [x] Config generation via `hivefuzz init`
- [ ] Validate target binary instrumentation
- [ ] Seed corpus loading and validation

### 0.7 — Mutation Engine
- [x] 21 mutation operators implemented
- [x] Weighted probability distribution for mutation selection
- [x] Fitness tracking with rolling window
- [x] Exp3 bandit algorithm for strategy evolution
- [x] Strategy blending for gossip-based learning

### 0.8 — Main Fuzz Loop
- [x] Input selection from corpus
- [x] Mutation application via strategy engine
- [x] Coverage processing and novelty detection
- [x] Crash processing and deduplication
- [x] Fitness recording per execution
- [x] Periodic strategy evolution
- [x] Graceful shutdown with statistics

---

## Phase 1: Gossip Protocol

### 1.1 — Transport Layer
- [x] UDP transport implementation (tokio UdpSocket)
- [x] Bincode serialization for GossipMessage
- [x] Async message send/receive with channel dispatch
- [ ] TCP connection pool for bulk transfers
- [ ] Message signing (Ed25519) for all outgoing messages
- [ ] Signature verification for incoming messages

### 1.2 — SWIM Membership Protocol
- [x] Membership list with peer state tracking
- [x] Peer selection for gossip (random fanout)
- [x] Failure detection state machine (Alive → Suspected → Dead)
- [ ] Implement ping/ping-ack protocol loop
- [ ] Implement indirect ping (ping-req) for robustness
- [ ] Implement join procedure
- [ ] Implement voluntary leave

### 1.3 — Coverage Dissemination
- [x] Disseminator with gossip round structure
- [x] Coverage digest sending to fanout targets
- [x] Corpus entry piggybacking
- [x] Crash alert propagation
- [ ] Coverage comparison and update request logic
- [ ] Track which edges have been shared

### 1.4 — Crash Dissemination
- [x] CrashAlert message propagation
- [ ] Crash fingerprint pre-check before full transfer
- [ ] Full crash data transfer via TCP

---

## Phase 2: Evolutionary Strategy Engine
- [x] Per-mutation fitness tracking
- [x] Exp3 weight updates
- [ ] Wire to actual fuzzing loop execution tracking
- [ ] Strategy gossip exchange
- [ ] Strategy adoption policy

---

## Phase 3: Crash Triage & Deduplication
- [x] Multi-level dedup (stack hash + ASAN class + signal)
- [x] Exploitability scoring
- [x] CWE suggestion
- [ ] Coverage-bitmap-based crash clustering
- [ ] Delta debugging minimization
- [ ] Report generation

---

## Phase 4: Deployment & Operations
- [x] Dockerfile (multi-stage build)
- [x] GitHub Actions CI (build, test, clippy, fmt)
- [ ] Docker Compose for local multi-node testing
- [ ] Prometheus metrics endpoint
- [ ] Grafana dashboard
- [ ] Terraform templates
