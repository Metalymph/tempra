# Tempra — Milestone Identity and Migration Map

**Revision:** 2026-11-16. This file records naming, milestone identity, and transitions. It does not rewrite historical results or certify implementations.

## 1. Public naming: definitive

| Official name | Execution model | Product role | Status |
| --- | --- | --- | --- |
| **Tempra Script** | Verified stack-bytecode VM | Scripting, CLI, REPL, small startup and distributable tooling | Planned default; VM parity not yet certified |
| **Tempra Native** | LLVM AOT | Native binaries and performance-focused applications | Future distribution/backend |

Both are modes of **Tempra**, using **one source language, semantic model, type system, effect contracts, and memory rules**. No separate feature dialect, new ownership syntax, or dual standard is introduced.

The prior metal-based naming scheme has been retired from **active** documentation. Its appearance in archived historical documents does not convey ongoing architectural authority. The legacy scripting-plan file is now [`script.md`](script.md); update inbound links. The historical JIT/Cranelift experiment is likewise not a future Tempra execution mode.

## 2. Canonical milestones

| ID | Meaning | Current interpretation |
| --- | --- | --- |
| **E** | Capability/effect analysis, contracts and inspection (E1–E6) | Implemented on the specified supported surface, not universal execution coverage |
| **K1** | Deterministic, restricted `comptime` kernel | Historical HIR implementation complete; VM migration separate |
| **K2** | Determinism/reproducibility | Level 3 on bounded same-build/host/target bytecode cases |
| **K3** | Runtime semantic parity | In progress, with supported paths stated explicitly |
| **K4** | Specialized effects | Implemented bounded contract vocabulary and matching; not Result hardening |
| **K5** | Compiler phase timings, process latency and memory baseline | Empirical milestone; not a proof; branch progress requires fresh evidence |
| **K6** | Responsiveness and regression budgets | Planned after K5 |
| **R** | Result/value-use obligation and error compatibility | R1–R3 tested gate reported; R4/R5 and formal linkage open |
| **M** | Ownership, acyclic values and memory semantics | [v0.4 specified](../Tempra_Memory_Ownership_v0.4.md); implementation/stabilization open |
| **M5** | Mandatory memory stabilization gate | Must pass before safe actor concurrency implementation |
| **C** | Structured tasks, actors, channels and selection | Planned; safety gated by M5 |
| **V** | Future `@verified` functions | Deferred; separate from formal `.PROOF` milestones |
| **A** | Tempra Native LLVM AOT | Future, after scripting maturity and required semantics |
| **O** | Advanced memory/ownership optimizations | After ownership facts; does not define language correctness |
| **NR** | RC-free research | Experimental and optional; cannot block correctness or baseline AOT |
| **P** | Deep compiler performance/incremental work | Guided by K5/K6 data |
| **U** | Unsafe boundary | Deferred until a concrete safety-relevant source operation requires it |

**Historical alias resolution:** references to “K4 Result/Error Hardening” mean canonical **R1–R3**, not another K4. `K4` remains the specialized-effects milestone. Retain original milestone numbers and audit records; do not retroactively renumber them.

## 3. Product gates and dependencies

`S1`–`S4` are product gates, distinct from compiler milestones:

1. **S1:** Tempra Script scripting achievement: VM-first supported subset, standard library, CLI, cache, distribution and safety/error parity.
2. **S2:** public site and tested documentation.
3. **S3:** formatter, linter, LSP, MCP, agent workflows and coherent edit-to-execution developer experience.
4. **S4:** Tempra Script alpha and beta release gates; S2/S3 may overlap S1 but must pass where required before release.

Native AOT is not needed to establish the Script runtime, and actor concurrency is not a prerequisite for the first Script release. The dependency chain for concurrency remains **R → M → M5 → C**. Native AOT consumes approved ownership/effect facts and should be sequenced after Script maturity; deeper optimizations and NR research may proceed independently without becoming safety dependencies. Self-hosting follows validated language/runtime maturity.

## 4. Formal status vocabulary

- **IMPLEMENTATION_GATE_PASS:** tests/audits passed for a declared bounded surface; not a proof.
- **DRAFT / SPECIFIED:** obligation or semantics documented, not proved or implemented by that fact alone.
- **MODEL_PROVEN:** a theorem checked for an explicitly named model and assumptions.
- **IMPLEMENTATION_LINKED:** the implementation-to-model relation has separately been justified or proved.
- **FORMALLY_VERIFIED:** all declared obligations for the stated scope are discharged, never implying broader guarantees.

Where a phase has a meaningful formal obligation, use `<canonical-id>.PROOF` and keep `formal/manifest.toml` separate from implementation status. Performance baselines, web sites and packaging do not need decorative proof labels.

## 5. Migration and cache policy

The public renaming is **documentation/product identity only**: it is not a source syntax, bytecode, compiler semantic revision or cache-format change. The last documented remote-main bytecode trust revision is `semantics-8`; do not increment it because a document changed.

The *actual* future implementation of ownership-aware verification or changed `comptime` semantics may require fresh analysis, bytecode metadata and a new semantic fingerprint. Follow [BYTECODE_CACHE.md](../BYTECODE_CACHE.md); never trust old artifacts for guarantees they did not establish.

Historical Cranelift JIT and backend scaffolds must be removed in a separate code change with workspace, dependency, feature, test, CLI and CI checks. Do not announce the removal as completed while those paths still exist.

## 6. Source-of-truth and status updates

- [`status.md`](status.md) records test-backed implementation gates.
- [`script.md`](script.md) records the Script product and release path.
- [`release-gates.md`](release-gates.md) defines alpha/beta exit conditions.
- [`../roadmap.md`](../roadmap.md) remains the main roadmap entrypoint.
- [`formal-verification.md`](formal-verification.md) records proof policy.
- [`../Tempra_Memory_Ownership_v0.4.md`](../Tempra_Memory_Ownership_v0.4.md) defines shared memory/ownership semantics.

No source document overrides a current source-code conformance test or a documented failed release gate.
