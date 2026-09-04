# Tempra Roadmap

This document defines Tempra's current implementation order. Phase-numbered prototype documents record completed experiments, but they do not override this roadmap or the current language specifications.

## Current implementation baseline

Tempra has a working lexer, parser, AST-to-HIR lowering, typechecker, HIR interpreter, module graph, native standard-library bridge, and language tooling. TIR, TIR verification, bytecode lowering, the bytecode VM, and the Cranelift JIT exist as experimental paths with a smaller supported language subset. MIR, general-purpose optimization, native AOT, and dedicated Wasm emission remain incomplete scaffolds.

## Language implementation order

```text
E — Light Capability / Effect System
        ↓
R — Result / Value-Use Hardening
        ↓
M — Acyclic Ownership / Complete ARC-Cycle Prevention
        ↓
C — Pluggable Actor Concurrency
        ↓
V — Verified Functions
        ↓
O — Deeper Memory Optimizations
```

This order is a dependency chain, not merely a list of preferences.

### E — Light Capability / Effect System

The E1–E4 semantic core is complete for the currently meaningful surface. The compiler computes stable per-function semantic summaries over verified TIR and propagates requirements transitively through calls. This does not close the existing TIR coverage gaps. E6 diagnostics/tooling is the next active milestone.

The public contract surface remains deliberately small:

- `@deny(...)`
- `@pure`

The internal initial vocabulary is `Alloc`, `Panic`, `IO`, `Process`, `Spawn`, `Nondeterminism`, and `Unsafe`. Public denial names exclude `unsafe`. `Unsafe` is reserved with no current E1 origins; the future `@unsafe` boundary belongs to conditional milestone U, not the immediate Phase E sequence.

Phase E is subdivided as follows:

1. **E1 — Internal capability model and TIR analysis (implemented).** `tempra-effects` provides pure analysis over `TirModule`, direct capability discovery, a reusable call graph, fixed-point propagation, deterministic provenance, and hand-built TIR fixture tests. It adds no public syntax.
2. **E2 — Minimal pipeline integration (implemented).** Fresh source and multi-module VM compilations verify TIR, analyze it once, and retain TIR, capability facts and verified bytecode in `VmCompilation`. Cache-hit execution, TIR emission, TIR interpretation and HIR paths remain analysis-independent. E2 itself added no enforcement, UI, attributes, CLI output or source-span expansion; E3 below defines the current contract behavior.
3. **E3 — `@deny(...)` (implemented).** E3.1 acquires nonempty typed contracts, stable compilation-local TIR function binding and declaration/attribute provenance. E3.2 enforces denied transitive requirements, retains structured violations/witnesses, rejects annotated execution on non-capability-aware paths and invalidates pre-enforcement cache artifacts through the semantic fingerprint.
4. **E4 — `@pure` (implemented).** The single convenience contract normalizes to denying `io`, `process`, `spawn`, and `nondeterminism` in the driver. Explicit deny is unioned; pure intent retains its own source span. E3 enforcement and cache semantic revision are unchanged; Alloc and Panic remain allowed by pure alone.

**E6 — Structured capability diagnostics and inspect/query surface (NEXT ACTIVE).** Reuse `CapabilityAnalysis`, `FunctionCapabilityContract`, `CapabilityContractViolation`, `CapabilityWitness`, `TirFunctionId`, and current structural/source provenance for human CLI, IDE/LSP, tools and agents. E6 is tooling over the implemented semantic core, not a new analyzer or a commitment to a particular CLI/JSON schema.

E1 is TIR-only. HIR-only operations are coverage gaps for later integration, not justification for a second analyzer.

The implemented E1–E4 design and boundaries are recorded in [E1 Capability Analysis](E1_CAPABILITY_ANALYSIS.md). The historical E5 audit found no intentional source-level primitive requiring unsafe authorization. It recorded narrow correctness debts in native signature integrity, comptime extern handling, and internal JIT pointer/ABI/lifetime assumptions, not new semantic phases or demonstrated exploits. `check` remains frontend/typechecking-only, not capability certification; E6 owns final diagnostic rendering.

### R — Result / Value-Use Hardening

This is the next semantic foundation after E, not an implemented rule or a capability/effect. Result-returning values should not be silently discarded: handling should use `?`, `match`, return/propagation, or another meaningful use. The future treatment of `let _ = result_returning_call()` must not create an accidental escape hatch; this does not add wildcard-binding syntax today. An explicit opt-out such as `@discardable` may be evaluated later and is not committed syntax.

### M — Acyclic Ownership / Complete ARC-Cycle Prevention

Memory correctness is distinct from memory optimization. The intended invariant is:

> Safe Tempra must not permit construction of strong ownership cycles that prevent ARC from reclaiming otherwise unreachable values.

The dedicated Phase M specification must define this rule across value semantics, `let`/`var`, `ref`, `inout`, `consume`, explicit closure captures, recursive closure `self`, structs, enums, and escaping values. The current interpreter representation does not yet establish this guarantee.

Phase M must keep the source language substantially simpler than Rust lifetimes or Pony reference capabilities. Formal acyclic ownership rules follow R in the semantic roadmap.

### C — Pluggable Actor Concurrency

Pluggable actor concurrency depends on both capability summaries and the acyclic ownership model. No runtime/plugin interface is specified here. The retained direction is:

- `Task[T]`, `spawn`, and `.await()`;
- structured `TaskGroup`;
- `actor`;
- `Channel[T]` and `select`.

Normal Tempra will not use `async fn`, locks, raw threads as its primary model, or user-facing atomics. `ref` and `inout` must not cross task, actor, or channel boundaries. Owned values, immutable share-safe values, and concurrency-safe handles will form the communication basis; their exact formal rules belong to Phase C after M.

### V — Verified Functions

`@verified` will request stronger static proof that runtime failures are impossible or explicitly represented. It is not an alias for capability denial.

Future checks include panic/crash paths, unsafe operations, overflow and underflow, division by zero, bounds accesses, numeric narrowing, fallible allocation, partial operations, and transitive calls. The initial design has no `requires`, `ensures`, `invariant`, separate proof language, or global `--verified` mode. Normal Tempra syntax, types, control-flow narrowing, `Result`, `Option`, and checked APIs express proof conditions.

### O — Deeper Memory Optimizations

Only after the memory model and concurrency boundaries are established should optimization aggressively reduce ARC overhead through:

- uniqueness analysis;
- escape analysis;
- stack promotion;
- Perceus-inspired reuse;
- retain/release elimination.

These are performance techniques, not prerequisites for memory correctness or concurrency.

### U — Unsafe Boundary (DEFERRED / CONDITIONAL)

U is a small standalone milestone, listed after O but activated by a concrete trigger, not a prerequisite for completing E: the first real source-level operation capable of violating Tempra memory/type invariants unless explicitly authorized. No such intentional primitive exists today, so `@unsafe` is not implemented or accepted.

The intended direction is local authorization within small audited implementation islands, without automatically requiring annotations throughout the caller graph. Safe wrapper interfaces should remain callable from ordinary safe Tempra and must preserve safety for their admitted inputs. This does not settle unsafe-to-call rules or introduce hypothetical primitives. `@unsafe` must never disable `@deny` or `@pure`; `@deny(unsafe)` remains invalid and `Capability::Unsafe` remains internal/reserved.

## Terminology

- **checked**: normal Tempra semantics; safe and implicit, with no `--checked` flag;
- **verified**: stronger local static guarantees requested by future `@verified`;
- **unsafe**: an explicit boundary for low-level operations unavailable to normal safe Tempra.

The former "Spark mode" and global checked/verified mode proposals are not part of the current design.

## Other implementation tracks

Native AOT, dedicated Wasm output, package maturity, REPL work, richer collections, and experimental JIT research remain valid engineering tracks. They must not reorder or weaken the E → R → M → C → V → O language dependency chain.

The richer **TIR Graph** is a later tooling milestone, not E6. It may eventually combine calls, CFG, capabilities, allocations, ownership edges, escapes, actor boundaries, specializations and source provenance. Some facts require M, C and later compiler work and cannot be promised by current capability inspection.

The Phase 15–18 JIT documents are historical implementation records for an opt-in experiment. They are not the normative language roadmap.
