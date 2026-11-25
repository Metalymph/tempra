# Tempra Roadmap — Status Snapshot

**Document revision:** 2026-11-16
**Status:** planning and implementation ledger; **not** a release certificate, proof, or CI attestation.
**Source boundary:** this snapshot is grounded in the accessible GitHub `main` documentation. The local `feature/k5-compiler-timings` branch has not been inspected; do not infer its current test outcomes from this table.

## Product identity

One language, two distributions:

- **Tempra Script:** scripting-first deployment through the verified stack-bytecode VM. This is the **target default**, not yet a verified migration of the historical HIR-default path.
- **Tempra Native:** native LLVM ahead-of-time compilation, planned after Script reaches the required maturity and semantic gates.

The source syntax, type system, value semantics, [Memory & Ownership v0.4](../Tempra_Memory_Ownership_v0.4.md), and capability contracts are shared. These are **not two dialects or separate languages**. There is no JIT target. The Cranelift code-removal change is independent of the remaining VM parity and ownership milestones.

## Implementation and release gates

| Workstream | Documented status | Open work / evidence needed |
| --- | --- | --- |
| E1–E6 capability/effect contracts | Implemented on bounded TIR/VM-enforced paths | Missing source coverage, unclassified externs, richer tooling projections; no general sandbox |
| K1 compile-time evaluation | HIR-based kernel implemented | Planned VM-backed `comptime` evaluation and parity gate |
| K2 reproducibility | Level 3 for documented same-build/host/target supported bytecode surface | No cross-machine Level 4 claim |
| K3 runtime parity | In progress | Supported-surface differential coverage and controlled failure/diagnostic checks |
| K4 specialized effects | Implemented on documented bounded surface | No general proof; production operation classifications are narrower than taxonomy |
| R1–R3 Result flow/provenance | Implementation gate reported passed on tested scope | Reproduce full workspace gate; R4 error compatibility, R5 parity and R.PROOF remain open |
| M / M5 Memory & Ownership | v0.4 **specified**; implementation gate not passed | Ownership-aware TIR/checking, nonduplicable resources, NO-CYCLE, cleanup, M5 tests/proofs as scoped |
| C actors/structured concurrency | Planned | M5 is mandatory before implementation can claim actor safety |
| K5 latency/phase measurements | Baseline work next according to `main`; local feature branch not verified | Commit-scoped measurements, published raw data and environment |
| K6 responsiveness budgets | Planned after K5 | Regression thresholds based on measured baselines |
| S1 Tempra Script | In progress | VM parity/default, scripting stdlib, CLI/REPL and distribution |
| S2 public website | Planned | Real reference, release/download flows, tested examples |
| S3 developer tooling | Required before alpha | Formatter, linter, LSP, MCP and agent workflows on declared scope |
| S4 Script alpha/beta | **Not released** | [Release gates](release-gates.md) must pass |
| A — Tempra Native LLVM AOT | Future | ABI, ownership-aware LLVM lowering, linking, executable parity and measurements |
| V / `@verified` | Deferred | Formal obligations and explicit model-to-implementation linkage |
| U / unsafe boundary | Deferred / conditional | Activated only by a real low-level requirement, not as speculative grammar |
| Self-hosting | Future | Validated Script semantics and mature toolchain |

## Safety and evidence policy

`Tempra Script` does **not** imply that ARC NO-CYCLE or complete memory safety has already passed conformance; these are language requirements whose implementation is subject to M5. `Tempra Native`'s zero GC/RC objective is research/optimization-oriented and **not proven**. AOT must preserve the same source semantics regardless of physical memory representation.

A passing local test, an existing crate, a grammar production, a phase-numbered log and a completed specification are different kinds of evidence. Promote a status only with an exact commit, test commands, environment, fixture scope, failures and gates documented.

See [milestone migration](migration.md), [Script plan](script.md), [K5 methodology](k5.md), [coverage](../VM_COVERAGE.md), and [formal verification](formal-verification.md).
