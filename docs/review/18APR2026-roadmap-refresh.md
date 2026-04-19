# Critical Review — Roadmap Refresh & Forward Objectives
**Date:** 18APR2026
**Status:** COMPLETED
**Author:** Claude Code
**Reviews:** `docs/planning/18APR2026-roadmap-refresh.md`

## Context

CLAUDE.md mandates a critical review for every non-trivial planning action.
This review stress-tests the roadmap refresh: do the objectives match reality,
are the sprint boundaries defensible, and what is likely to go wrong?

## Summary Verdict

**Approve with caveats.** The MVS framing is sound and the sprint sequencing
is defensible. Three caveats need acknowledgement before execution (see
Risks §1, §2, §4). None of them invalidate the plan.

## What the Plan Gets Right

- **Anchoring on a Minimum Viable Swarm.** The prior docs list capabilities by phase; the refresh instead anchors on a demonstrable 3-node outcome with measurable acceptance tests. This forces prioritization.
- **Promoting observability into Sprint 4.** Debugging convergence without `/metrics` is painful. Pulling this forward from Phase 4 is correct and low-cost.
- **Explicit non-goals.** Calling out Honggfuzz / libFuzzer / Terraform / DBSCAN as out-of-scope prevents scope creep.
- **Message signing pulled forward.** Retrofitting signatures into a live protocol is painful. Doing it during Sprint 3 while the gossip loop is being completed is the right moment.
- **Strategy-in-the-loop called out as gap.** The observation that Exp3 exists but isn't wired is a real defect; surfacing it prevents the swarm from claiming evolutionary behavior it doesn't have.

## Risks & Flaws

### 1. Sprint 2 status ambiguity may double-count work  *(medium impact)*
`docs/planning/25MAR2026-sprint-2-multi-node.md` is APPROVED but `COMPLETED.md`
shows SWIM ping loop, join, and seed loading as unchecked. Sprint 3 assumes
Sprint 2 is **not** merged. If Sprint 2 partially landed since `COMPLETED.md`
was last updated, Sprint 3 will re-plan completed work.

**Mitigation:** First action of Sprint 3 is a git-log audit of
`src/gossip/swim.rs`, `src/gossip/membership.rs`, and `src/node/mod.rs` to
establish ground truth before writing the sprint plan. Update `COMPLETED.md`
*before* scoping Sprint 3.

### 2. Convergence targets may be optimistic under real AFL++ exec rates  *(medium impact)*
The 30s (3 nodes) / 60s (10 nodes) convergence targets assume gossip_interval≈5s
and fanout=3. That's roughly correct for *digest* propagation, but
convergence-in-coverage also requires TCP corpus transfer to complete. On a
target producing novel edges at tens per second, corpus transfer is the
bottleneck, not digest dissemination.

**Mitigation:** Define "converged" precisely in the integration test: either
(a) bitmap equality (strict, likely misses target), or (b) <1% bitmap delta
for a full gossip_interval (weaker, testable). The plan already says "<1%";
keep it and document the rationale in the Sprint 3 test.

### 3. MVS test #3 (kill a node, detect within 3×interval) is SWIM-timing-sensitive  *(low impact)*
At gossip_interval=5s, 3× is 15s. SWIM's standard failure detection takes
suspicion_timeout *after* the initial miss, often multiples of the ping
period. The literal 3× bound may be unreachable without tuning.

**Mitigation:** Either raise the bound to `ping_interval + suspicion_timeout`
(explicit) or tune timeouts in the test. Don't let a cosmetic bound fail an
otherwise working protocol.

### 4. "10-node compose" test in Sprint 5 is CI-hostile  *(medium impact)*
Docker-in-CI with 10 long-running containers is flaky (port conflicts,
timing, resource limits on free runners). Putting this in the required PR
gate will produce false negatives.

**Mitigation:** Run the 10-node test as a nightly/scheduled workflow, or as
an opt-in `workflow_dispatch`, not on every PR. Plan already notes this as
an Open Question; promote to a decision before Sprint 5.

### 5. Strategy-in-loop wiring may expose noisy fitness signal  *(low impact)*
Once Exp3 actually drives mutation selection, it will amplify whatever the
fitness metric rewards. If "edges per execution" is computed over too short a
window, early lucky mutations will dominate. The plan mentions a minimum
weight floor but not window sizing.

**Mitigation:** Sprint 4 plan should specify the fitness window size
explicitly (start with the existing 100K-execution window) and add a
regression-style unit test that asserts no mutation type drops below the
floor.

### 6. SQLite persistence can introduce fsync on the hot path  *(low impact)*
A write-through crash DB is correct, but naively calling `INSERT` with
default journaling per crash will stall the fuzz loop on slow disks. Crashes
are rare, but a crash *storm* (e.g., first run against a broken target) can
produce thousands per second.

**Mitigation:** Use WAL mode, batched inserts, or push persistence to a
background task with a bounded channel. Call this out in the Sprint 4 plan.

### 7. Bloom filter benchmarking has no fail threshold  *(low impact)*
"Bump size if >3% FP" is a reasonable rule of thumb but not a test. Without
a concrete assertion, the benchmark becomes a report nobody reads.

**Mitigation:** Sprint 4 should include a unit test that builds a bloom with
realistic edge counts, computes observed FP rate, and fails if above
threshold. Tune the threshold empirically but commit it.

## Edge Cases the Plan Does Not Cover

- **Clock skew between nodes.** Gossip signatures and failure timeouts both rely implicitly on rough clock agreement. On spot instances with drifting clocks this can cause signature replay window issues or false suspicion. Add a note to Sprint 3: no wall-clock dependencies in protocol logic; use monotonic clocks.
- **Partial network partitions.** SWIM handles asymmetric failure via indirect ping; the plan mentions this but doesn't include a test. Add a two-partition integration test in Sprint 5.
- **Identity key rotation / compromise.** Ed25519 keypair is generated per node and never rotated. Acceptable for MVS; flag as a known limitation.
- **Seed file pathological inputs.** Sprint 4 seed loading skips empty and >1MB files. Should also skip binary files that crash AFL parsing (e.g., invalid magic bytes for a parser target). Consider running each seed through the backend once with a timeout before admission.

## Decisions

1. **Approve the plan as-is.** The risks above are operational, not structural; they refine execution rather than require replanning.
2. **Require a Sprint 2 status audit before Sprint 3 scoping** (addresses Risk §1).
3. **Convergence definition must be written into the integration test, not left implicit** (addresses Risk §2).
4. **10-node compose test moves off the PR gate** before Sprint 5 starts (addresses Risk §4).

## Open Questions

1. Do we need a CHANGELOG in addition to `COMPLETED.md`, or is the latter sufficient for release tracking? (Leaning: `COMPLETED.md` is enough until there's an external user.)
2. Should `docs/reference/` be seeded now (gossip protocol, bitmap format, crash fingerprint format) or lazily when an external integrator appears? (Leaning: lazily — premature documentation rots fastest.)
