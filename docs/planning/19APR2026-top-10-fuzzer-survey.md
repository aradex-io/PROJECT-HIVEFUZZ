# Top-10 Fuzzer Survey & Integration Verdicts
**Date:** 19APR2026
**Status:** APPROVED
**Author:** Claude Code (branch `claude/project-assessment-fuzzer-options-nnush`)

## Context

Operator clarified that HIVEFUZZ runs on dedicated rigs / isolated VMs, so
the LLM-prompt redaction and "no binaries/no crash dumps" constraints from
the 19APR2026 plan are **dropped**. That unlocks shape-2 LLM backends (full
LLM-driven mutator) and removes the cost of a redaction layer, but does not
change the hybrid-swarm design — coverage bitmaps across engines are still
incompatible for purely technical reasons.

This document surveys the top-10 fuzzers worth considering for HIVEFUZZ
integration, with pros, cons, and a keep-or-skip verdict per fuzzer.

---

## Selection Criteria

A fuzzer is "worth adding" if it meets at least one of:

- **Uncovered target class** — language/platform HIVEFUZZ can't currently fuzz (JVM, Python, Go, JS, binaries without source).
- **Differentiated exploration strategy** — exploration mechanism AFL++ fundamentally lacks (concolic solving, grammar-awareness, semantic/LLM mutation).
- **Performance regime AFL++ can't match** — in-process speed, hardware counters.
- **Mature OSS-Fuzz lineage** — if Google fuzzes with it, the tooling is battle-tested.

A fuzzer is "skip" if:

- Pure reimplementation of AFL-style bitflipping with marginal gains (<10% FuzzBench delta).
- Orphaned or unmaintained (last commit >18 months).
- Would require a full rewrite of HIVEFUZZ's architecture to integrate cleanly.

---

## The Top 10

### 1. AFL++  —  **KEEP (default)**
*Already integrated via subprocess.*

- **Pros:** FuzzBench-leader for bug-finding (146 unique causes in one study); huge mutator inventory (MOpt, Redqueen, laf-intel, AFLfast++); active community; binary-mode via QEMU/Unicorn; persistent-mode for speed; fork-server already stable.
- **Cons:** Subprocess wrapper is slower than in-process alternatives; coverage model is AFL-bitmap-only (64KB edges with hit buckets); grammar-blind.
- **Integration cost (already paid):** 0.
- **Verdict:** **Keep as default backend.** No reason to move away.

### 2. libFuzzer  —  **ADD (Sprint 6, high priority)**
*In-process, persistent, part of LLVM.*

- **Pros:** Orders-of-magnitude faster on small targets (no fork-exec per input); native language bindings (cargo-fuzz for Rust, Atheris for Python, Jazzer for JVM, Jazzer.js for Node — all libFuzzer-derived); OSS-Fuzz's native engine; best-in-class for parsers/crypto.
- **Cons:** Requires source + clang rebuild with `-fsanitize=fuzzer`; in-process crashes abort the process (mitigated by persistent mode + fork mode); coverage model is 8-bit per-PC counters, **not** compatible with AFL++ bitmap — corpus is sharable, bitmap is not.
- **Integration cost:** Medium. Subprocess wrapper first (run target with `-runs=1`), in-process FFI later.
- **Verdict:** **Highest-value add.** Biggest practical win per engineering hour; unlocks the entire libFuzzer-descended ecosystem.

### 3. Honggfuzz  —  **DEFER (Sprint 8 or later)**
*Hardware-counter feedback (Intel PT / perf_events).*

- **Pros:** On some FuzzBench studies Honggfuzz finds the most *additional* crashes after AFL++; persistent mode is fast; Intel PT gives coverage on closed-source binaries without instrumentation; maintained by Google.
- **Cons:** Requires bare-metal or VM with perf-counter access (containers often restrict this); install is fiddly; the "when would I reach for this" use-case largely overlaps AFL++ QEMU mode.
- **Integration cost:** Low-medium (subprocess wrapper).
- **Verdict:** **Defer.** Real value but narrow. Add only after libFuzzer + LLM lands and there's demand for Intel-PT-on-closed-binaries.

### 4. LibAFL  —  **PIVOT QUESTION, not a backend**
*Modular Rust fuzzing framework from the AFL++ team.*

- **Pros:** Beats AFL++ on FuzzBench (score 98.61 vs 96.32); already Rust (native fit for HIVEFUZZ); scales linearly across cores via LLMP message passing; modular components (observers, feedbacks, stages, mutators); `no_std` support for embedded; the direction the AFL++ team is clearly investing in.
- **Cons:** It's a **library, not a fuzzer** — you assemble one. Integrating LibAFL would mean replacing most of `src/fuzzer/` and `src/strategy/` with LibAFL types; HIVEFUZZ would become "LibAFL + distributed gossip layer" rather than "AFL++ subprocess + gossip."
- **Integration cost:** High (architectural pivot, not a backend add).
- **Verdict:** **Strategic question, not a Sprint-6 item.** Propose a spike in Sprint 10+: replace `AflBackend` with a `LibAflBackend` that embeds a LibAFL fuzzer in-process. If the spike shows >2× throughput and cleaner coverage handling, it becomes the new default. Track as ADR-005 candidate.

### 5. Jazzer (JVM)  —  **ADD (Sprint 8)**
*Coverage-guided in-process fuzzer for Java/Kotlin/Scala.*

- **Pros:** Only serious option for JVM targets; OSS-Fuzz-adopted; Spring/Hibernate/JUnit integrations; libFuzzer-derived engine.
- **Cons:** JVM-only; must ship a JRE in the node image; coverage is per-Java-edge — not compatible with AFL++ or libFuzzer bitmaps.
- **Integration cost:** Medium (subprocess + JRE dependency).
- **Verdict:** **Add when a JVM target appears in scope.** Defer until then. Scaffold the backend factory to make it a ~1-day add later.

### 6. Atheris (Python)  —  **ADD (Sprint 8)**
*Python in-process fuzzer, libFuzzer-derived.*

- **Pros:** Only serious option for Python targets (parsers, ML code, web frameworks); cheap to run (no native build); good for fuzzing C-extensions via their Python wrappers.
- **Cons:** Python's GIL limits per-process scaling (HIVEFUZZ's multi-node model already handles this); slow compared to C targets.
- **Integration cost:** Low (subprocess).
- **Verdict:** **Add when a Python target appears.** Same rationale as Jazzer.

### 7. cargo-fuzz / `rustc -Z sanitizer=fuzzer`  —  **ADD (falls out of libFuzzer)**
*Rust-native libFuzzer wrapper.*

- **Pros:** Fuzzes HIVEFUZZ's own code; validates the fuzzer abstraction from the inside; libFuzzer ecosystem member so no new backend class.
- **Cons:** Nightly toolchain typically needed; scope limited to Rust targets.
- **Integration cost:** Zero if we already have libFuzzer subprocess support — cargo-fuzz is just a target-side convention.
- **Verdict:** **Free once libFuzzer lands.** Document as a supported target class in the README, add a dogfood integration test.

### 8. Nautilus / Gramatron (grammar-aware)  —  **ADD as a specialist cohort (Sprint 9)**
*Coverage-guided grammar-aware generational fuzzers.*

- **Pros:** On grammar-shaped targets (compilers, interpreters, SQL parsers, JS engines) Gramatron achieves 24%+ more coverage than flat mutators and finds complex bugs Nautilus can't; inputs are 24% smaller (cheaper to gossip); grammar files are operator-supplied, reusable.
- **Cons:** Requires a grammar — unusable on targets that have no structured format; Nautilus integrates with AFL-style loop; Gramatron is standalone; both single-target per grammar.
- **Integration cost:** Medium (subprocess + operator workflow for providing grammar).
- **Verdict:** **Add as an optional specialist cohort.** A node configured with `backend = "gramatron"` + `grammar = "sql.g"` can generate high-quality seeds and gossip them into the swarm. Non-grammar nodes still run AFL++ / libFuzzer. Strong synergy with LLM-assisted (LLM can *author* the grammar).

### 9. Hybrid concolic (SymCC / Fuzzolic + Fuzzy-Sat)  —  **ADD as a scout node (Sprint 9)**
*Concolic execution helping fuzzers past hard path constraints.*

- **Pros:** Solves path constraints (magic bytes, checksums) that no mutation-based fuzzer will brute-force; Fuzzolic + Fuzzy-Sat beats SymCC/SymQEMU/Qsym on coverage; SymCC works on source, Fuzzolic works on binaries via QEMU; published integrations with AFL++ already exist.
- **Cons:** Expensive — one concolic execution takes milliseconds to seconds, so it's a *scout*, not a main engine; SMT solver dependency; SymCC needs recompilation with a wrapper compiler.
- **Integration cost:** Medium-high (subprocess + SMT dependency + glue to feed solved inputs back into corpus).
- **Verdict:** **Add as a single specialist node role.** "Concolic scout" runs Fuzzolic on corpus entries that have plateaued on coverage (curated by the gossip layer), emits novel inputs back into the shared corpus. One node per swarm is usually plenty.

### 10. LLM-driven (OSS-Fuzz-Gen / Fuzz4All / PromptFuzz / TitanFuzz)  —  **ADD in two shapes (Sprint 7)**
*LLMs as (a) fuzz-driver authors and (b) semantic mutators.*

- **Pros:**
  - **OSS-Fuzz-Gen** — generates fuzz drivers (harnesses) for projects that don't have one. Unblocks fuzzing arbitrary libraries. Google is running this in production against OSS-Fuzz since 2024.
  - **PromptFuzz** — coverage-guided prompt mutation achieves 40.12% branch coverage, 1.61× OSS-Fuzz baseline; found 33 real CVE-class bugs.
  - **Fuzz4All** — language-agnostic LLM fuzzing, 98 bugs across GCC/Clang/Z3/CVC5/OpenJDK/Qiskit.
  - **TitanFuzz** — 30–50% more code coverage than SOTA on TensorFlow/PyTorch via CodeGen-based seed mutation.
- **Cons:** Cost (API spend scales with call count); latency (each call adds seconds); semantic mutator is useful mainly on structured inputs; a bad prompt template poisons a whole swarm if gossiped.
- **Integration cost:**
  - **Shape 1** (LLM mutator plugin): low — add `MutationType::LlmRewrite`, wire to Anthropic SDK with prompt caching, Exp3 or budget-based selection.
  - **Shape 2** (LLM-authored fuzz drivers, à la OSS-Fuzz-Gen): medium — a new node role that ingests target source, produces harnesses, feeds them to the libFuzzer/AFL backend on the same node.
  - **Shape 3** (full LlmBackend): medium — node runs Fuzz4All-style loop with no traditional mutator.
- **Verdict:** **Highest-leverage novel capability.** Ship Shape 1 in Sprint 7 (mutator plugin). Ship Shape 2 (driver generation) as an optional tool rather than a backend. Ship Shape 3 as an experimental specialist node after Shape 1 proves itself.

---

## Summary Table

| # | Fuzzer | Target class | Verdict | Sprint |
|---|--------|--------------|---------|--------|
| 1 | AFL++ | C/C++ binary, QEMU | **KEEP default** | — |
| 2 | libFuzzer | C/C++ in-process | **ADD first** | 6 |
| 3 | Honggfuzz | C/C++/Java/Go binary, Intel PT | **DEFER** | 8+ |
| 4 | LibAFL | Rust framework for any | **PIVOT SPIKE** | 10+ |
| 5 | Jazzer | JVM in-process | **ADD on demand** | 8 |
| 6 | Atheris | Python in-process | **ADD on demand** | 8 |
| 7 | cargo-fuzz | Rust | **FREE with libFuzzer** | 6 |
| 8 | Nautilus / Gramatron | grammar-shaped | **ADD specialist cohort** | 9 |
| 9 | SymCC / Fuzzolic | hybrid concolic | **ADD as scout node** | 9 |
| 10 | LLM-driven (PromptFuzz / Fuzz4All) | semantic/driver-gen | **ADD in three shapes** | 7 |

## Decisions

1. **Security-boundary clauses for LLM calls are dropped** (operator runs on dedicated rigs / isolated VMs). The 19APR2026 plan's Decision 4 is revoked. LLM prompts may include stack traces, target source, and crash dumps as useful context.
2. **libFuzzer is Sprint 6, first add.** Highest ROI per engineering hour; unlocks Jazzer/Atheris/cargo-fuzz line of descent.
3. **LLM capability lands in Sprint 7 in three shapes** (mutator / driver-gen / full backend). Shape 1 is required; 2 and 3 follow.
4. **LibAFL is a strategic pivot question, not a Sprint 6 item.** Tracked as ADR-005 candidate for a Sprint-10 spike: "Can HIVEFUZZ embed a LibAFL fuzzer in-process instead of shelling out to AFL++?" — potentially 2× throughput and unified coverage model.
5. **Grammar-aware (Gramatron) and concolic scout (Fuzzolic) nodes are optional specialists** — one or two per swarm, not a general-purpose cohort.
6. **Jazzer, Atheris, Honggfuzz are conditional adds** — scaffold the backend factory in Sprint 6 so they're each ~1-day additions when a target appears.
7. **The no-coordinator invariant holds.** Specialist nodes (concolic scout, LLM driver-gen, grammar) are still equal peers — their output flows into the shared corpus via gossip, they don't control the swarm.

## Open Questions

1. **LibAFL pivot timing.** Do we run the spike in Sprint 10 unconditionally, or gate it on whether the AFL++-subprocess throughput becomes a bottleneck in production? Leaning *gate it* — don't invent work.
2. **Grammar authoring workflow.** If LLM-driven Shape 2 can generate fuzz drivers, can it also generate Gramatron grammars from target source? Worth spiking in Sprint 9.
3. **OSS-Fuzz-Gen alignment.** Should we adopt OSS-Fuzz-Gen's prompts/templates directly (they're OSS, Apache-2.0) or roll our own? Leaning adopt — free quality.
4. **Concolic scout input selection.** Which corpus entries should the concolic node pick? Simple heuristic: those that have been in corpus >24h with zero new-edge credit. Tune after Sprint 9 lands.
