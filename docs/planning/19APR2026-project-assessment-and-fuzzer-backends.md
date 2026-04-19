# Project Assessment & Multi-Backend / Hybrid-Swarm Plan
**Date:** 19APR2026
**Status:** APPROVED
**Author:** Claude Code (branch `claude/project-assessment-fuzzer-options-nnush`)

## Context

Two asks rolled into one document:
1. Take stock of HIVEFUZZ right now and produce a concrete action-items list for
   the pending roadmap work.
2. Evaluate incorporating additional fuzzers (libFuzzer, Honggfuzz, LLM-assisted)
   so operators can (a) pick a backend per node and (b) deploy **heterogeneous /
   hybrid swarms** where different nodes run different engines for different
   purposes.

This refines — does not replace — `docs/PLAN.md` and the 18APR2026 roadmap
refresh. The backend-plurality work is a net addition, not a re-scoping of the
MVS.

---

## Part A — Current Status Assessment (as of 19APR2026)

### Ground Truth from the Tree

`cargo test` → 31 unit + 8 integration tests, all green. Warnings only
(unused imports in `commands/run.rs`, dead fields in `crash/dedup.rs` and
`gossip/membership.rs`).

| Area | Landed | Missing (pending) |
|------|--------|-------------------|
| Fuzzer backend abstraction | `FuzzerBackend` trait; `TargetConfig`, `FuzzResult`, `CrashInfo`, `Severity` | libFuzzer backend, Honggfuzz backend, instrumentation sniff, vulnerable-target integration test |
| Coverage bitmap | 64KB AFL bitmap, merge/diff, bloom digest, hit-count buckets | RLE compression for wire, FP benchmarks & tuning |
| Corpus | Content-hash dedup, provenance, priority queue, seed loading | Minimization (afl-cmin equivalent), serialization-for-gossip wire format |
| Crash | Multi-level dedup, exploitability scoring, CWE suggestion, in-memory DB | **SQLite persistence**, delta-debug minimization, structured ASAN parsing, coverage-based crash clustering, report generator |
| Node identity | Ed25519 keypair, signing primitives | **Gossip frames are not yet signed/verified** |
| Config / CLI | TOML load+validate+generate, `init`/`run`/`dev`/`status` | Target instrumentation sniff in `init`, `--backend` selector |
| Strategy | Mutator (21 ops), fitness window, Exp3, blend/noise | **Exp3 is not wired to live fitness;** evolution tick not driven by real executions in run loop; strategy gossip adoption policy |
| Gossip transport | UDP + bincode + async dispatch | TCP pool for bulk transfers, message signing, verify-on-ingest |
| SWIM | Membership, state machine, ping/ack, bootstrap/join (Sprint 2 landed) | Indirect ping (ping-req), voluntary leave broadcast hardening, suspicion-timeout tuning |
| Dissemination | Digest / corpus / crash send skeletons | Compare-and-request loop (diff local bloom against incoming digest, pull novel data) |
| Deployment | Dockerfile + CI | Compose, `/metrics` endpoint, Grafana, Terraform |

### Sprint 2 Status — Resolved

The 18APR2026 review flagged ambiguity about whether Sprint 2 had merged.
Git log confirms commit `b2b2849 "feat: implement SWIM gossip protocol, seed
loading, and concurrent fuzz loop"` landed. Integration tests
`test_swim_two_node_discovery` and `test_seed_corpus_loading` pass. So Sprint 2
**is merged**; Sprint 3 should focus on the dissemination compare-and-request
loop, indirect ping, signing, and TCP pool — **not** replan Sprint 2.

### Reality-Check on the Roadmap Refresh

The 18APR2026 sprint plan (Sprints 3–5) remains valid. The fuzzer-plurality work
proposed in Part B does **not** derail the MVS milestone; it's a Phase-5+
addition that reuses the existing `FuzzerBackend` trait.

---

## Part B — Multi-Backend & Hybrid-Swarm Strategy

### B.1 — Why Plural Backends Matter

AFL++ is an excellent default but is not universally the right tool:

- **libFuzzer** — in-process, persistent mode, best for small fast targets (parsers, crypto).
- **Honggfuzz** — hardware-counter feedback (Intel PT), good for kernel / closed-source targets.
- **LLM-assisted mutators** — semantic-aware mutation for structured inputs (JSON, SQL, source code, protocol messages). References: OSS-Fuzz-Gen, Fuzz4All, ChatFuzz, TitanFuzz, PromptFuzz. Much slower per execution but dramatically higher novel-edge yield on grammar-shaped targets.
- **Grammar / structure-aware** engines (Nautilus, Grammarinator, Gramatron) — a natural fit alongside LLMs for the "schema-aware" class.

A swarm that **mixes** these engines on the same target amortizes each one's
blind spots: AFL++ bruteforces the bit layer, libFuzzer exploits the in-process
speed advantage, LLM nodes explore semantically valid but adversarial inputs
that the bitflippers can't reach.

### B.2 — What HIVEFUZZ Already Has Going For It

- `FuzzerBackend` trait at `src/fuzzer/mod.rs` is already the clean seam for new engines — it exposes `init`, `run_input`, `get_coverage`, `get_corpus`, `add_to_corpus`, `stats`.
- Config is TOML-driven. Adding a `[fuzzer] backend = "afl++" | "libfuzzer" | "honggfuzz" | "llm"` field is straightforward.
- Gossip messages carry raw `Vec<u8>` corpus entries + a `CoverageBitmap` — neither depends on which engine produced them, provided the coverage model is reconcilable (see B.4).

### B.3 — What Needs to Change

1. **Backend selection in config + CLI.**
   - Extend `FuzzerSection` with `backend: BackendKind` (default `AflPlusPlus`).
   - Add `--backend <kind>` CLI override to `hivefuzz run` / `dev`.
   - Factory in `commands/run.rs`: construct the right `Box<dyn FuzzerBackend>`.

2. **LibFuzzerBackend.**
   - Two modes: (a) subprocess mode — spawn a libFuzzer binary with `-runs=1 -artifact_prefix=...` per input (simple, slow); (b) in-process mode — load target as a shared library, call `LLVMFuzzerTestOneInput` via FFI, harvest coverage from `__sancov_*` callbacks. Start with (a); (b) is a follow-up.
   - `unsafe` contained to the FFI shim module; document the ABI assumptions.

3. **HonggfuzzBackend.**
   - Subprocess wrapper around `honggfuzz --persistent` with its socket/pipe protocol for input injection and feedback. Lower priority than libFuzzer because AFL++ subprocess already covers similar ground.

4. **LLM-assisted engine — two viable shapes.**
   - **Shape 1 (recommended first):** LLM as a **mutator plugin**, not a full backend. Add a new `MutationType::LlmRewrite` that calls an LLM (Claude via Anthropic SDK) to produce a grammar-aware variant of a corpus entry. The existing AFL++/libFuzzer backend still executes it. Use Exp3 to down-weight it automatically if it's not producing coverage gains per token of budget.
   - **Shape 2 (later):** **LlmBackend** — a full backend that swaps the random mutator for an LLM-driven one and runs a thin harness loop. Useful for "LLM-only specialist" nodes in a hybrid swarm.
   - **Budget:** LLM calls are rate-limited (e.g., ≤1 call per N seconds, configurable) and bounded by cost caps. Cache by input hash to avoid duplicate spend. (Previous draft included a no-binaries / no-crash-dumps security boundary; this is **revoked** — operator runs on dedicated rigs, so stack traces, crash dumps, and target source may be used as LLM context. See `19APR2026-top-10-fuzzer-survey.md` Decision 1.)

5. **Coverage reconciliation across engines.**
   This is the real design problem. AFL++ uses a 64KB edge-id-indexed bitmap
   with hit-count buckets. libFuzzer uses 8-bit counters per PC. Honggfuzz uses
   branch counters or Intel PT. Direct bitmap merging across different
   instrumentation schemas is **not meaningful** — the same byte index means
   different things in each engine.
   - **Decision:** keep per-engine coverage bitmaps *local*; share only **corpus entries** and **crash fingerprints** across heterogeneous nodes. Coverage convergence (the MVS criterion) is meaningful only within an engine cohort.
   - **Corpus cross-pollination is the primary value-add of a hybrid swarm**: a libFuzzer node discovers an input, gossips it, an AFL++ node runs it and may find that input exercises edges the libFuzzer PC counters never highlighted (and vice versa).
   - Gossip protocol change: tag every `CoverageDigest` / `CoverageUpdate` with a `backend_kind` field. Peers of the same kind use it as today; peers of a different kind accept the **corpus entries** but ignore the bitmap.

6. **Hybrid swarm orchestration.**
   - `hivefuzz dev --nodes 5 --target X.toml --topology "afl:2,libfuzzer:2,llm:1"` launches a mixed swarm for local testing.
   - Each node advertises its backend kind in the SWIM `Join` / `MembershipSync` payload. Membership API exposes `peers_by_backend(kind)` for strategy logic.
   - Strategy adoption policy (Phase 2 work) becomes backend-aware: don't blend mutation weights from an LLM node into an AFL++ node — their weight spaces don't align. Share them with same-kind peers only.

### B.4 — Data-Flow Design for Heterogeneous Swarms

```
          ┌──────────────────┐        ┌────────────────────┐
          │  AFL++ cohort    │        │  libFuzzer cohort  │
          │  ───────────     │        │  ───────────       │
          │  gossip edge     │        │  gossip PC cover   │
          │  bitmaps +       │        │  counters +        │
          │  corpus          │        │  corpus            │
          └──────┬───────────┘        └───────────┬────────┘
                 │ corpus entries only            │
                 └──────────┬─────────────────────┘
                            │
                      ┌─────▼──────┐
                      │  LLM-node  │
                      │  consumes  │
                      │  corpus,   │
                      │  emits     │
                      │  semantic  │
                      │  variants  │
                      └────────────┘
```

Key invariants:
- **Corpus** — shared across *all* backends; inputs are bytes, engine-agnostic.
- **Crash fingerprints** — shared across all backends; stack-hash + ASAN class is engine-agnostic.
- **Coverage bitmaps** — shared only within a backend cohort.
- **Strategy weights** — shared only within a backend cohort.
- **Membership / liveness** — shared globally (SWIM doesn't care about backend).

### B.5 — Sprint Sequencing for the Backend Work

This slots **after** MVS (Sprints 3–4) and builds on Sprint 5's scale work.

- **Sprint 6 — Backend Plurality (core).** Backend enum in config, factory in
  CLI, libFuzzer subprocess backend, `backend_kind` tag in gossip frames,
  per-cohort coverage routing. Integration test: 2 AFL + 2 libFuzzer nodes
  converge on corpus cross-pollination.
- **Sprint 7 — LLM-assisted mutator (Shape 1).** `MutationType::LlmRewrite`,
  Anthropic SDK integration with prompt caching, rate/cost caps, Exp3
  weight-down on poor yield, opt-in flag so the default build doesn't require
  an API key.
- **Sprint 8 — Hybrid topology + Honggfuzz.** `--topology` flag in `dev`,
  `peers_by_backend` API, Honggfuzz subprocess backend, backend-aware strategy
  adoption.
- **Sprint 9 — In-process libFuzzer + LlmBackend (Shape 2).** FFI shim with
  unsafe boundary, full LLM-only backend for specialist nodes.

### B.6 — Action Items List (Consolidated)

**Immediate — finish MVS (Sprints 3–4 per 18APR2026 plan):**
- [ ] Indirect ping (ping-req) in `src/gossip/swim.rs`
- [ ] Dissemination compare-and-request loop in `src/gossip/dissemination.rs`
- [ ] TCP connection pool for bulk transfer (bitmap, corpus, crash input)
- [ ] Ed25519 message signing on outgoing gossip; verify on ingest
- [ ] SQLite persistence for `CrashDatabase` (WAL mode, batched inserts)
- [ ] Exp3 actually driven by live fitness in the run loop
- [ ] Prometheus `/metrics` endpoint
- [ ] Target instrumentation sniff in `hivefuzz init`
- [ ] Bloom filter FP benchmark with committed assertion
- [ ] Integration test: 3 nodes converge on bitmap (<1% delta) within 30s

**Housekeeping (low-risk, do alongside):**
- [ ] Remove unused imports in `src/commands/run.rs`
- [ ] Wire or remove `stack_depth` in `src/crash/dedup.rs`
- [ ] Wire or remove `last_ping_sent` in `src/gossip/membership.rs`
- [ ] Turn on `cargo clippy -D warnings` in CI

**Sprint 5 per 18APR2026 plan (scale & triage):**
- [ ] Docker Compose topology for 10 nodes
- [ ] Corpus minimization as background task
- [ ] Structured ASAN stack parsing
- [ ] Delta-debug crash minimization
- [ ] Grafana dashboard template

**New — Backend plurality (Sprint 6):**
- [ ] `BackendKind` enum in `src/fuzzer/mod.rs`, serde-serialized
- [ ] `[fuzzer] backend = ...` in `hivefuzz.toml`; CLI `--backend` override
- [ ] Backend factory in `commands/run.rs` / `commands/dev.rs`
- [ ] `LibFuzzerBackend` (subprocess mode first)
- [ ] `GossipMessage` variants carry `backend_kind`; per-cohort routing in dissemination
- [ ] Integration test: mixed 2× AFL + 2× libFuzzer swarm shares corpus

**New — LLM-assisted mutator (Sprint 7):**
- [ ] Add Anthropic SDK dependency (feature-gated)
- [ ] `MutationType::LlmRewrite` plugin calling Claude with prompt caching
- [ ] Per-node rate / cost cap config
- [ ] Input-hash cache to avoid duplicate spend
- [ ] Exp3 safeguard: auto down-weight if yield < threshold over window
- [ ] Opt-in: no-op when API key env var absent

**New — Hybrid topology + Honggfuzz (Sprint 8):**
- [ ] `--topology afl:N,libfuzzer:M,llm:K` in `hivefuzz dev`
- [ ] `backend_kind` advertised in SWIM `Join`/`MembershipSync`
- [ ] `peers_by_backend()` on `MembershipList`
- [ ] Backend-aware strategy adoption (same-cohort only)
- [ ] `HonggfuzzBackend` subprocess wrapper
- [ ] Integration test: heterogeneous 3-cohort swarm runs 10 minutes

**New — Specialist backends (Sprint 9):**
- [ ] libFuzzer in-process mode via FFI
- [ ] Full `LlmBackend` (Shape 2) for LLM-only specialist nodes
- [ ] Grammar-aware backend spike (Nautilus or Gramatron subprocess)

## Decisions

1. **Keep the MVS as the next milestone.** Backend plurality is sequenced *after*
   the 18APR2026 Sprints 3–5, not interleaved. A hybrid swarm with a broken
   gossip loop has no value.
2. **Coverage bitmaps stay cohort-local in a heterogeneous swarm;** only corpus
   and crash fingerprints cross cohort boundaries. This is the only honest
   design — cross-engine bitmap merging is not semantically defined.
3. **LLM-assisted fuzzing enters as a mutator plugin first (Shape 1), a full
   backend second (Shape 2).** Lower blast radius; easier to bound cost.
4. **~~No LLM call ever ships target binaries, full crash dumps, or memory
   contents to a third-party API.~~** **REVOKED** 19APR2026 — operator runs
   HIVEFUZZ on dedicated rigs / isolated VMs, so the third-party-API
   exposure threat model doesn't apply. LLM prompts may include stack
   traces, crash dumps, target source, and any other context that improves
   mutation quality. Cost control (Decision 7 of the review) still applies.
5. **AFL++ remains the default backend.** No breaking config changes — an
   existing `hivefuzz.toml` without a `backend` field keeps working.
6. **Honggfuzz stays deferred** relative to libFuzzer and LLM work. Its unique
   value (Intel PT, kernel) is real but narrower than libFuzzer's in-process
   speed or LLM semantic exploration.

## Open Questions

1. **LLM provider abstraction.** Start with Claude-only (leveraging the
   Anthropic SDK already in the stack) or abstract so operators can bring their
   own model? Leaning Claude-first; add an OpenAI-compatible adapter later if
   asked.
2. **Cost accounting.** Do we expose LLM spend as a Prometheus metric? (Probably
   yes — operators will need it.)
3. **Offline LLM support.** Should we allow llama.cpp / local-model endpoints?
   Low priority for v1; revisit after Shape-1 lands.
4. **libFuzzer coverage export.** libFuzzer's `-print_coverage=1` is text-only
   and slow. For real-time coverage we'd need to tap `__sancov` callbacks via
   FFI — defer to in-process mode in Sprint 9.
5. **Grammar hints.** If a node runs LLM-assisted, where does it get the
   grammar? A new `[target.grammar]` section in `hivefuzz.toml` (freeform
   string the operator writes) seems sufficient; full grammar file support
   can come later.
