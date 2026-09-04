# Tempra Roadmap

This document defines Tempra's current implementation order. Phase-numbered prototype documents record completed experiments, but they do not override this roadmap or the current language specifications.

## Current implementation baseline

Tempra has a working lexer, parser, AST-to-HIR lowering, typechecker, HIR interpreter, module graph, native standard-library bridge, and language tooling. TIR, TIR verification, bytecode lowering, the bytecode VM, and the Cranelift JIT exist as experimental paths with a smaller supported language subset. MIR, general-purpose optimization, native AOT, and dedicated Wasm emission remain incomplete scaffolds.

## Language implementation order

```text
E — Capability / Effect Analysis
        ↓
M — Acyclic Ownership / Memory Model
        ↓
C — Concurrency
        ↓
V — Verified Functions
        ↓
O — Deeper Memory Optimizations
```

This order is a dependency chain, not merely a list of preferences.

### E — Capability / Effect Analysis

Capability analysis is the immediate language implementation target. The compiler will compute stable per-function semantic summaries over verified TIR and propagate requirements transitively through calls.

The public contract surface remains deliberately small:

- `@deny(...)`
- `@pure`
- `@unsafe`

The internal initial vocabulary is `Alloc`, `Panic`, `IO`, `Process`, `Spawn`, `Nondeterminism`, and `Unsafe`. Public denial names exclude `unsafe`; `@unsafe` is a separate low-level boundary.

Phase E is subdivided as follows:

1. **E1 — Internal capability model and TIR analysis (implemented).** `tempra-effects` provides pure analysis over `TirModule`, direct capability discovery, a reusable call graph, fixed-point propagation, deterministic provenance, and hand-built TIR fixture tests. It adds no public syntax.
2. **E2 — Minimal pipeline integration (implemented).** Fresh source and multi-module VM compilations verify TIR, analyze it once, and retain TIR, capability facts and verified bytecode in `VmCompilation`. Cache-hit execution, TIR emission, TIR interpretation and HIR paths remain analysis-independent. No enforcement, UI, attributes, CLI output or source-span expansion.
3. **E3 — `@deny(...)`.** E3.1 is implemented: nonempty typed contracts, stable compilation-local TIR function binding and declaration/attribute provenance, without enforcement. E3.2 remains pending: enforce denied transitive requirements and expose contract witnesses, with a mandatory cache non-bypass gate for artifacts from older non-enforcing compilers.
4. **E4 — `@pure`.** Add the single convenience contract, equivalent to denying `io`, `process`, `spawn`, and `nondeterminism`.
5. **E5 — `@unsafe`.** Connect the safe-by-default language direction to an explicit low-level boundary without redesigning the entire unsafe subsystem.
6. **E6 — Diagnostics and introspection.** Expose structured paths and stable data for future `tempra inspect`, `tempra effects`, and JSON diagnostics.

E1 is TIR-only. HIR-only operations are coverage gaps for later integration, not justification for a second analyzer.

The implemented E1–E3.1 design and boundaries are recorded in [E1 Capability Analysis](E1_CAPABILITY_ANALYSIS.md). E3.2 is next; cache invalidation/metadata/bypass strategy must be settled before enforcement can be considered complete.

### M — Acyclic Ownership / Memory Model

Memory correctness is distinct from memory optimization. The intended invariant is:

> Safe Tempra must not permit construction of strong ownership cycles that prevent ARC from reclaiming otherwise unreachable values.

The dedicated Phase M specification must define this rule across value semantics, `let`/`var`, `ref`, `inout`, `consume`, explicit closure captures, recursive closure `self`, structs, enums, and escaping values. The current interpreter representation does not yet establish this guarantee.

Phase M must keep the source language substantially simpler than Rust lifetimes or Pony reference capabilities. Formal acyclic ownership rules are the next design and implementation phase after E.

### C — Concurrency

Concurrency depends on both capability summaries and the acyclic ownership model. The retained direction is:

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

## Terminology

- **checked**: normal Tempra semantics; safe and implicit, with no `--checked` flag;
- **verified**: stronger local static guarantees requested by future `@verified`;
- **unsafe**: an explicit boundary for low-level operations unavailable to normal safe Tempra.

The former "Spark mode" and global checked/verified mode proposals are not part of the current design.

## Other implementation tracks

Native AOT, dedicated Wasm output, package maturity, REPL work, richer collections, and experimental JIT research remain valid engineering tracks. They must not reorder or weaken the E → M → C → V → O language dependency chain.

The Phase 15–18 JIT documents are historical implementation records for an opt-in experiment. They are not the normative language roadmap.
