# Critical Review — Project Assessment & Multi-Backend Plan
**Date:** 19APR2026
**Status:** COMPLETED
**Author:** Claude Code
**Reviews:** `docs/planning/19APR2026-project-assessment-and-fuzzer-backends.md`

## Context

CLAUDE.md mandates a critical review for every non-trivial planning action.
This document stress-tests the 19APR2026 plan: is the status assessment
accurate, is the backend-plurality design sound, and what is likely to break
in execution?

## Summary Verdict

**Approve with caveats.** The assessment matches the tree; the sprint ordering
(MVS first, backends after) is correct. Four caveats need acknowledging before
Sprint 6 starts (see Risks §1, §3, §4, §6). None invalidate the plan.

## What the Plan Gets Right

- **Refuses to merge engine coverage bitmaps.** Many "multi-engine fuzzer" designs try to unify coverage and produce meaningless aggregates. Restricting bitmap gossip to within-cohort peers, and sharing only corpus + crash fingerprints across cohorts, is the honest design.
- **Ranks LLM-as-mutator above LLM-as-backend.** A mutator plugin is smaller, cheaper, and easier to safely disable. The full backend can come later once we know the mutator earns its keep.
- **Defers Honggfuzz behind libFuzzer.** libFuzzer's in-process speed is a differentiator AFL++ subprocess can't touch; Honggfuzz's differentiator (Intel PT, kernel) is much narrower.
- **Preserves defaults.** An existing `hivefuzz.toml` keeps working without a `backend` field. No forced migration.
- **Security boundary for LLM calls is explicit.** "No binaries, no crash dumps — only inputs + operator-supplied grammar hint" is the right rule.

## Risks & Flaws

### 1. Corpus cross-pollination may be less valuable than assumed  *(medium impact)*
The claim that a libFuzzer-found input helps an AFL++ node, and vice versa, is
plausible but not free. If both engines instrument the same target with
different sanitizers, inputs that crash under libFuzzer's ASAN might not crash
under AFL++'s build — the corpus is shared, the *findings* are not symmetric.
Worse: the two binaries may diverge in code paths due to different sanitizer
runtimes, making "coverage gained from imported corpus" a weak signal.

**Mitigation:** Sprint 6 integration test must measure *novel edges gained
from cross-cohort corpus imports* and fail if it's zero. If the number is
small, surface it and re-scope before Sprint 8.

### 2. LLM rate-limiting interacts badly with Exp3  *(low impact)*
If the LLM mutator is rate-limited to, say, one call per 10s, its sample count
in the Exp3 window will be tiny compared to thousands of AFL mutations. Exp3
will either (a) ignore it (never selected) or (b) be misled by a single lucky
outcome.

**Mitigation:** Sprint 7 must track LLM-mutation stats in a **separate**
fitness bucket, not the main Exp3 pool. Selection becomes "budget-based"
(every N seconds, run one LLM mutation regardless of Exp3 weight) rather
than weight-based. Plan should make this explicit.

### 3. ~~"No binaries, no crash dumps" is easy to violate accidentally~~ — **REVOKED 19APR2026**
Operator clarified HIVEFUZZ runs on dedicated rigs / isolated VMs; third-party-API
exposure is not in the threat model. LLM prompts may freely include stack traces,
memory addresses, target source, and crash dumps — they meaningfully improve
mutation quality. The redaction test is not required.

### 4. Heterogeneous SWIM membership complicates failure detection  *(low impact)*
If an LLM node takes 30s to respond (waiting on API), SWIM's ping/ack will
false-positive-suspect it. Current suspicion timeouts are tuned assuming
sub-second ping turnaround.

**Mitigation:** SWIM ping/ack must remain on a fast-path that does **not**
block on fuzz work. Verify the SWIM loop runs on a separate tokio task from
the fuzz loop (it should — `src/node/mod.rs` already uses `tokio::select!`).
Add an integration test: spawn a node with an artificially slow mutator and
confirm it stays Alive in the SWIM view.

### 5. `--topology` flag design can proliferate  *(low impact)*
Today: `afl:2,libfuzzer:2,llm:1`. Tomorrow operators will want
"libfuzzer with ASAN + libfuzzer with UBSan + llm with JSON grammar." The
flag will become a mini-DSL.

**Mitigation:** Sprint 8 should accept the flag for the simple case and
*also* accept a topology YAML/TOML file for anything more complex. Avoid
fighting a flag-format war.

### 6. FFI for in-process libFuzzer crosses an `unsafe` boundary the project has avoided  *(medium impact)*
CLAUDE.md says "Minimize `unsafe` — use only when required by FFI (e.g., shared
memory)." In-process libFuzzer requires a nontrivial unsafe shim:
`LLVMFuzzerTestOneInput` signature, `__sancov_pcs_init`, signal handlers for
in-process crash catching, and a `longjmp`-based recovery path. Getting this
wrong is a source of hard-to-reproduce node crashes that look like target
crashes.

**Mitigation:** Sprint 9 must deliver the FFI shim behind a strict module
boundary (`src/fuzzer/libfuzzer_ffi/`), with every `unsafe` block commented
for invariants, and a shadow stack trace test that executes a known-crashing
target 1000 times and confirms the node survives. Consider a feature flag so
CI builds can opt out until the FFI is hardened.

### 7. Cost control for LLM is not free to implement  *(low impact)*
"Rate-limit by time" is easy; "rate-limit by cost with hard cap that pauses
spend and notifies the operator" is a small feature on its own. Hand-waving
it in Sprint 7 risks shipping something that accidentally spends a lot.

**Mitigation:** Sprint 7 plan must include a hard-stop on cumulative-cost
overrun (e.g., `$MAX_LLM_SPEND_USD` env var, default tight) and a Prometheus
counter `hivefuzz_llm_spend_usd_total`. Treat this as acceptance criteria.

## Edge Cases the Plan Does Not Cover

- **Backend hot-swap.** Operators may want to switch a node's backend without restart (e.g., LLM node hits cost cap, degrade to AFL). Not a v1 feature; flag explicitly as **not supported** so nobody assumes it.
- **Same binary, different instrumentation.** Two AFL++ nodes could run different AFL build variants (one with ASAN, one without). The coverage bitmap shape matches but edge IDs are from different compilations. Within-cohort bitmap gossip will *appear* to work but produce meaningless merges. Cohort identity should include a binary hash, not just backend kind.
- **LLM providers rate-limit globally.** If a 10-node swarm all calls Anthropic simultaneously, we hit org-level rate limits, not per-node. Need a distributed token-bucket or a single-node pattern ("LLM scout" node) before horizontal scale.
- **Corpus poisoning.** A malicious LLM or a buggy prompt could inject inputs that are deliberately unhelpful (huge, malformed, etc.). Existing corpus size limits protect disk, but not Exp3 signal — a flood of low-yield LLM mutations will still be measured. Per Risk §2, the separate fitness bucket mitigates this.

## Decisions

1. **Approve the plan as-is.** Risks above are operational, not structural.
2. **Cohort identity = (backend_kind, target_binary_hash)** (addresses Edge Case 2). Bake into `GossipMessage` tagging from Sprint 6 day one.
3. **LLM fitness is a separate bucket, not part of Exp3** (addresses Risk §2). Codify in Sprint 7 plan.
4. ~~LLM prompt redaction is a required test~~ — **REVOKED 19APR2026.** Redaction is not a requirement; see Risk §3 annotation.
5. **In-process libFuzzer requires a `libfuzzer-in-process` feature flag** (addresses Risk §6). Default build stays subprocess-only.

## Open Questions

1. **Measurement.** Do we agree that "novel edges from cross-cohort corpus imports" is the right success metric for Sprint 6? (Leaning yes; if consensus, write it into the Sprint 6 plan as the pass/fail gate.)
2. **LLM cost ceiling default.** Should the out-of-box default be `$0` (explicit opt-in) or a small nonzero? Leaning `$0` — nothing spent unless an operator sets the cap.
3. **Operator grammar hints.** Freeform string in `hivefuzz.toml` vs. a structured grammar file format. Leaning freeform for v1; reconsider once we see what operators actually write.
