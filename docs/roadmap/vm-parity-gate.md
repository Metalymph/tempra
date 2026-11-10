# Ergal VM-only acceptance gate — before cross-runtime comparisons

**Decision date:** 2026-11-10. **Status:** accepted sequence; gate not yet passed.

This document refines [PRE-AOT](pre-aot.md), [unified VM](unified-vm.md),
[K5 measurements](k5.md), and [release gates](release-gates.md). It introduces
**no new K milestone ID**. S1 is the product track; R, M and E retain their
semantic authorities. No existing HIR or TIR interpreter may be retired solely
because an experimental VM test passes.

## Non-negotiable order

1. **Keep internal instrumentation:** finish K5.2c (generic specialization,
   comptime cost and instrumentation overhead). No language-to-language speed
   comparisons yet. Phase timings are diagnostic, not optimization results.
2. **Audit the complete execution chain:** Source -> HIR -> checked TIR ->
   verified bytecode -> stack VM. Record all supported syntax, unsupported
   lowering cases, verifier failures, semantic mismatches and error categories.
3. **Close functionality gaps** on the declared Ergal surface: normal scripts
   without `main`, module graph, functions/generics, patterns, closures,
   collections, Result/Option, error and effect contracts, stdlib/FFI and CLI
   arguments. Anything intentionally deferred must be documented and must fail
   explicitly; a green test that accepts `UnsupportedExperimentalVm` does
   **not** prove parity.
4. **Unify compile-time execution** via incremental typed TIR and the same VM,
   with correct stage separation, diagnostics, effects, deterministic replay
   and execution budget. Compare with the current comptime authority first.
5. **Switch all non-AOT paths:** `run`, tests, REPL and Shell profile (when
   implemented), plus `comptime`, use bytecode VM. No implicit HIR or TIR
   interpreter fallback. Preserve failed-compilation behavior and exit codes.
6. **Retire obsolete execution engines:** after validated parity, remove the
   general HIR runtime interpreter and general TIR interpreter; JIT is
   independently retired as a policy decision after its dependency/test audit.
   The stack VM is itself a bytecode interpreter, not a forbidden interpreter.
7. **Optimize only with internal evidence:** typecheck, TIR lowering, bytecode
   lowering/verifier, cache, allocation and VM dispatch; measure pre/post on
   identical workloads and retain semantic, diagnostic and reproducibility
   regression gates. Prefer correctness and clean architecture over micro-gains.
8. **Then K5.3–K5.5/K6:** measure VM-only release binary startup, RSS, size,
   cold/warm cache and representative workload distributions. Only after this
   may controlled comparisons with CPython/Node/Bun/TypeScript begin.

M/M5 remains the mandatory actor-safety prerequisite. VM runtime parity must
not be interpreted as ownership or no-cycle correctness, and neither substitutes
for formal R.PROOF, M.PROOF, E.PROOF or VM.PROOF evidence.

## Audit matrix (each row needs recorded evidence)

| Surface | Required check | Current status |
| --- | --- | --- |
| Source/frontend | AST/HIR typing and diagnostics agree with compiled path | Audit pending |
| HIR -> TIR | Every declared supported expression/statement lowers or errors explicitly; no dropped side effects | Audit pending |
| TIR verifier | Invalid types, blocks, terminators, function IDs and references rejected | Audit pending |
| TIR -> bytecode | Stack effects, local slots, branches, calls, payloads, cleanup and control flow preserved | Audit pending |
| Bytecode verifier | Reject malformed cache and invalid instructions; never bypass enforcement | Audit pending |
| VM semantics | Same values, failures and observable effects as canonical language semantics | Audit pending |
| Result/effects | No omitted Result obligations, `@deny` bypass or unhandled failure path | Audit pending |
| Runtime coverage | Top-level scripts, args, modules, imports, stdlib, generics and closures | Audit pending |
| Comptime | Early lowering, normal function calls, budgets, stage values and deterministic output | Audit pending |
| REPL/shell | Incremental VM evaluation, state, process/job handling where declared | Planned |
| Build footprint | JIT/old runtime removed from shipping closure; LLVM absent from Ergal | Planned |
| Performance | Internal before/after timings, wall time, RSS and binary footprint | K5 underway |

### Explicitly weak evidence

- A test that allows either execution success or `UnsupportedExperimentalVm`
  validates controlled rejection, **not supported behavior**.
- HIR-vs-TIR-interpreter differential tests do not verify bytecode lowering.
- A one-shot timing is not a statistical baseline; cache-hit paths must not
  recompile solely for instrumentation.
- A clean Cargo workspace does not imply the VM covers every HIR feature.
- Scan hits for `TODO`, `unsupported` or `unimplemented!` are leads for human
  review, not demonstrated defects or exhaustive coverage.

## Required artifacts and acceptance

The read-only `scripts/vm_parity_audit.py` creates a local JSON/Markdown
inventory under `target/k5/vm-parity/`, with git revision, selected differential
and lowering test evidence, and source review hints. Run `just vm-parity-audit`.

Before switching default execution, commit a **versioned VM parity matrix** with
runnable positive and negative Tempra examples, expected values/typed errors,
associated test names, and CI enforcement. For each uncovered gap, require a
minimal regression test before the fix. Differential runners must validate
exit codes, stdout/stderr and observable effects where applicable, not merely
`Ok()` and equal final integers. Never hide regressions with an HIR fallback.

The VM-only gate is **not passed** until all declared Ergal supported cases
have explicit evidence on bytecode VM and comptime is parity-validated.
