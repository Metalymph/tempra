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

The semantic dependency chain remains a design constraint. The immediate
implementation sequence also includes tooling, consistency and performance work:

```text
E — Capability / Effect Semantic Core (E1–E4 DONE)
        ↓
E6 — Structured Effects Tooling (DONE)
        ↓
K  — Comptime Consistency & Compiler Responsiveness
        ↓
R  — Result / Value-Use Hardening
        ↓
M  — Acyclic Ownership
        ↓
C  — Pluggable Actor Concurrency
        ↓
V  — @verified
        ↓
O  — Deeper Memory Optimization
        ↓
P  — Fast Compilation & Incremental Builds
        ↓
U  — Unsafe Boundary (conditional)
```

### E — Capability / Effect Semantic Core (E1–E4 DONE)

The E1–E4 semantic core and E6 tooling projection are complete for the currently meaningful surface. The compiler computes stable per-function semantic summaries over verified TIR, propagates requirements transitively through calls, and exposes those facts without a second analysis. This does not close the existing TIR coverage gaps.

The public contract surface remains deliberately small:

- `@deny(...)`
- `@pure`

The internal initial vocabulary is `Alloc`, `Panic`, `IO`, `Process`, `Spawn`, `Nondeterminism`, and `Unsafe`. Public denial names exclude `unsafe`. `Unsafe` is reserved with no current E1 origins; the future `@unsafe` boundary belongs to conditional milestone U, not the immediate Phase E sequence.

Phase E is subdivided as follows:

1. **E1 — Internal capability model and TIR analysis (implemented).** `tempra-effects` provides pure analysis over `TirModule`, direct capability discovery, a reusable call graph, fixed-point propagation, deterministic provenance, and hand-built TIR fixture tests. It adds no public syntax.
2. **E2 — Minimal pipeline integration (implemented).** Fresh source and multi-module VM compilations verify TIR, analyze it once, and retain TIR, capability facts and verified bytecode in `VmCompilation`. Cache-hit execution, TIR emission, TIR interpretation and HIR paths remain analysis-independent. E2 itself added no enforcement, UI, attributes, CLI output or source-span expansion; E3 below defines the current contract behavior.
3. **E3 — `@deny(...)` (implemented).** E3.1 acquires nonempty typed contracts, stable compilation-local TIR function binding and declaration/attribute provenance. E3.2 enforces denied transitive requirements, retains structured violations/witnesses, rejects annotated execution on non-capability-aware paths and invalidates pre-enforcement cache artifacts through the semantic fingerprint.
4. **E4 — `@pure` (implemented).** The single convenience contract normalizes to denying `io`, `process`, `spawn`, and `nondeterminism` in the driver. Explicit deny is unioned; pure intent retains its own source span. E3 enforcement and cache semantic revision are unchanged; Alloc and Panic remain allowed by pure alone.

5. **E6 — Structured capability diagnostics and inspect/query surface (implemented).** `tempra effects [--json] <path>` compiles fresh and projects `CapabilityAnalysis`, contracts, violations, witnesses, TIR identities and current provenance for humans and tools. `TirFunctionId` remains authoritative; names are display-only. The current JSON schema is deterministic but not promised as a permanent protocol. E6 does not implement LSP integration or the future TIR Graph.

E1 is TIR-only. HIR-only operations are coverage gaps for later integration, not justification for a second analyzer.

The implemented E1–E6 design and boundaries are recorded in [E1 Capability Analysis](E1_CAPABILITY_ANALYSIS.md). The historical E5 audit found no intentional source-level primitive requiring unsafe authorization. It recorded narrow correctness debts in native signature integrity, comptime extern handling, and internal JIT pointer/ABI/lifetime assumptions, not new semantic phases or demonstrated exploits; K1.4 has since resolved the bodyless-dispatch portion of the comptime debt. `check` remains frontend/typechecking-only, not capability certification.

### K — Comptime Consistency & Compiler Responsiveness

K is the next active compiler milestone. It must establish one canonical,
backend-independent comptime semantics: VM, JIT, Wasm and future AOT must observe
the same comptime result or error. The K1 audit/spec selected the existing HIR
interpreter as the semantic kernel. K1.1 adds one canonical facade that owns
comptime mode/fuel setup, value/error/control-flow normalization and state
restoration. K1.2 extends the same authority to ordinary multi-module and
file-based typechecking through a module-aware lookup adapter over separate HIR
files. K1.3 makes supported integer arithmetic in the shared HIR kernel
panic-free and profile-independent without changing float semantics. The
K1.4 shared call dispatch rejects functions without executable HIR bodies rather
than producing synthetic `Unit`; comptime still does not execute VM-native
externs. K1.5 makes the HIR interpreter the sole effect-policy authority for
currently executable comptime operations: local `cmd` construction and
allocation remain allowed, while process execution, output/input, filesystem,
environment, host-OS access, `dbg` and `exit` are rejected at their effect
boundaries. The typechecker remains the owner of primitive HIR materialization,
and TIR/backends consume resolved code. K1.6 adds deterministic
evaluator-progress fuel ticks and a conservative compiler-defined v0.1
call-depth safety limit of 8 active Tempra frames. The limit is revisionable as
the evaluator evolves; nested comptime shares the active session without
resetting its budget.

Current status:

```text
K1 audit/spec       DONE
K1.1 facade         DONE
K1.2 multi-module   DONE
K1.3 arithmetic     DONE
K1.4 extern/bodyless DONE
K1.5 effect policy   DONE
K1.6 termination     DONE
K1.7 cache revision  DONE
K1 implementation   DONE
```

K1.7 raises the cache trust fingerprint from `semantics-1` to `semantics-2`, so
artifacts predating K1.3–K1.6 cannot bypass current comptime semantics. K2.1's
determinism audit is complete. K2.2 gives specialization requests deterministic
first-request order, emits bytecode functions in ascending `TirFunctionId`, and
uses an explicit TIR-to-bytecode function map instead of treating identity as a
vector position. It raises current cache trust to `semantics-3` because revision-2
artifacts may contain the former specialization miscompilation. Bytecode format
and semantic revision remain independent.
Native signature integrity remains separate from the now-explicit HIR bodyless
rejection. K2 remains in progress: explicit stdlib discovery, diagnostic ordering
and the global cross-process reproducibility gate follow K2.2. VM/JIT/Wasm runtime
parity continues in K3; phase timings, latency baseline
and regression budgets continue in K4–K6. Analysis/LSP/REPL comptime integration
remains tooling work, while full comptime specialization is separate compiler
feature/debt. Compile latency remains monitored throughout development; no deep
optimization is implied by K1.

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

### P — Fast Compilation & Incremental Builds

P is the deep optimization milestone for compilation responsiveness. It may
address cold-compile latency, incremental compilation, module-level
invalidation/cache, specialization deduplication and control, reuse of stable
facts, and interning/arena/data-structure optimization. Parallelization follows
profiling rather than preceding it. Agent and IDE edit-to-compile latency is a
product requirement. E6 introduces no incremental cache, daemon or persistent
capability facts.

### U — Unsafe Boundary (DEFERRED / CONDITIONAL)

U is a small standalone conditional milestone shown after P but activated by a
concrete trigger, not by list position and not as a prerequisite for completing
E: the first real source-level operation capable of violating Tempra memory/type
invariants unless explicitly authorized. No such intentional primitive exists
today, so `@unsafe` is not implemented or accepted.

The intended direction is local authorization within small audited implementation islands, without automatically requiring annotations throughout the caller graph. Safe wrapper interfaces should remain callable from ordinary safe Tempra and must preserve safety for their admitted inputs. This does not settle unsafe-to-call rules or introduce hypothetical primitives. `@unsafe` must never disable `@deny` or `@pure`; `@deny(unsafe)` remains invalid and `Capability::Unsafe` remains internal/reserved.

## Terminology

- **checked**: normal Tempra semantics; safe and implicit, with no `--checked` flag;
- **verified**: stronger local static guarantees requested by future `@verified`;
- **unsafe**: an explicit boundary for low-level operations unavailable to normal safe Tempra.

The former "Spark mode" and global checked/verified mode proposals are not part of the current design.

## Other implementation tracks

Native AOT, dedicated Wasm output, REPL work, richer collections, experimental
JIT research, the future TIR Graph and a safe CLI package manager remain explicit
engineering tracks. They are not folded into E6 and must not weaken the semantic
dependency chain.

The richer **TIR Graph** is a later tooling milestone. It may eventually combine calls, CFG, capabilities, allocations, ownership edges, escapes, actor boundaries, specializations and source provenance. Some facts require M, C and later compiler work and cannot be promised by current capability inspection.

The Phase 15–18 JIT documents are historical implementation records for an opt-in experiment. They are not the normative language roadmap.
