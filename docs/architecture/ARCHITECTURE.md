# Tempra Architecture

This document describes the compiler as it exists today and the intended insertion point for the next semantic analyses. Planned components are labeled explicitly.

## Design principles

- **Script first:** the default execution path favors a small, direct HIR interpreter.
- **Forward semantic flow:** source representations lower from AST to HIR and, for the experimental VM, to verified TIR.
- **Typed shared facts:** cross-cutting analyses should publish stable per-function facts instead of independently rebuilding semantic information.
- **Small public language:** compiler sophistication must not require a large source-level effect, ownership, or proof language.
- **Backend isolation:** native and Wasm code generation remain separate targets; Wasm output must not be routed through Cranelift.
- **Structured diagnostics:** analysis results should retain identities and provenance suitable for terminal, JSON, LSP, and inspection tools.

## Current compiler paths

### Default run path

```text
Source
  → Lexer / Parser
  → AST
  → HIR lowering
  → Reject capability contracts (execution only)
  → Type checking
  → HIR interpreter
```

`tempra run` uses this path unless the experimental VM is selected. Annotated execution is explicitly unsupported here, not silently unchecked. `tempra check` currently stops after parsing, HIR lowering, module resolution, and type checking; it does not certify capability contracts.

### Experimental VM path

```text
Source
  → Lexer / Parser
  → AST
  → HIR lowering
  → Type checking and comptime pruning
  → TIR lowering
  → TIR verification
  → Capability analysis (fresh compilation)
  → Deny contract enforcement
  → Bytecode lowering
  → Bytecode verification
  → VmCompilation (retained TIR, capability facts, bytecode)
  → Bytecode VM
  → optional experimental Cranelift JIT for eligible functions
```

TIR support covers a smaller language subset than the HIR interpreter. Generic templates are not emitted directly; requested concrete specializations are lowered where supported.

This is the fresh source/multi-module VM path. Bytecode cache hits verify and
execute cached bytecode without analysis or reconstructed TIR, but require an
enforcement-aware semantic fingerprint and matching source identity. TIR emission
verifies then formats. Source TIR interpretation rejects contracts before lowering;
raw TIR interpretation verifies then interprets. Neither needs analyzer coverage.
HIR execution, REPL and test execution reject annotated input through one shared
HIR presence scan before execution/state commit. See [pipeline boundaries](PIPELINE.md).

### Incomplete planned paths

The `tempra-mir`, `tempra-ir`, `tempra-opt`, native AOT, Wasm backend, linker, and build crates contain varying levels of scaffold or prototype code. They are not a single production pipeline today. Historical design documents that describe them as complete do not override this status.

## Capability analysis insertion point

Phase E uses verified TIR as its only semantic substrate:

```text
TIR lowering
  → TIR verification
  → capability analysis
  → later semantic analyses
  → bytecode, native, or Wasm backends
```

E1 provides a pure `tempra-effects` crate that depends only on `tempra-tir`. It consumes a `TirModule` and returns stable summaries, a call graph, and deterministic provenance. It does not mutate TIR, change bytecode formats, parse attributes, or add an HIR analyzer.

E2 implements minimal driver integration: private `AnalyzedTir` ties immutable
TIR to its facts, and public `VmCompilation` also retains verified bytecode.
Source and multi-module fresh compilation analyze once immediately after TIR
verification. E3.2 enforces contracts before bytecode lowering; typed failures
are retained in `DriverError`. Capability presence without a denying contract is accepted.

E3.1 preserves dedicated `@deny` syntax through AST/HIR. It adds no semantic
capability logic to frontend crates. Driver-owned conversion and registration
bind the concrete HIR declaration directly to the ID returned by lowering,
including specializations, without a post-hoc name lookup. Immutable contract
accessors on `VmCompilation` retain source/module identity and declaration and
attribute spans alongside facts. E3.2 compares denied and transitively required
sets, retaining deterministic violations and E1 witnesses; final rendering is E6.
Instruction source mapping remains absent. E3.2 uses the existing cache header's
fingerprint `tempra-{CARGO_PKG_VERSION}-semantics-1`; legacy artifacts miss and only
successfully enforced compilations can write replacements. No facts are serialized.

E4 adds only pure source intent and normalization: AST/HIR retain `pure_span`,
and the driver unions Io/Process/Spawn/Nondeterminism with explicit deny. Effective
contracts retain pure provenance separately from explicit deny spans. Neither
E1 analysis nor E3.2 enforcement changes; pure allows allocation and may-panic
operations unless explicitly denied. The shared HIR presence guard covers pure
without changing the execution adapters. `semantics-1` remains valid because the
old parser rejected pure and changed annotations change source identity.

```text
tempra-tir
    ↑ dependency
tempra-effects
    ↑ dependency
tempra-driver
    ↑ consumed by
tempra-analysis / tempra-lsp / tempra-cli
```

Source-span mapping remains deferred: TIR has no reliable source mapping.
E2 retains structural locations, edges, origins and witnesses. The future
bridge belongs in a lowering side table from `TirLocation` to module identity
plus existing `Span`; debug names are not a source map. HIR-only operations,
cache-hit facts and unresolved E1 native/cloning mappings remain coverage gaps.

## Semantic analysis order

The language-level dependency order is:

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

Capability summaries become reusable inputs for ownership checks, concurrency-boundary checks, verified functions, diagnostics, and tooling.

## Capability summary architecture

E1 derives direct capability requirements by scanning verified TIR instructions and called extern functions. A call graph built from `TirInstr::Call` propagates requirements to callers through reverse internal edges until a fixed point.

Provenance must retain:

- the function containing a direct requirement;
- its `TirBlockId` and instruction index;
- each callsite along a transitive path;
- deterministic ordering and tie-breaking.

Unknown extern declarations are harmless until called. A call to an extern without a known classification is an analysis error, including calls in non-entry TIR functions.

`Alloc` classification follows semantic runtime allocation in the current representation. TIR string constants and construction instructions are not normative allocation sites without runtime confirmation. `Panic` is a conservative may-panic capability in E1; later verified analysis may prove individual operations safe.

## Memory architecture boundary

The current HIR runtime uses reference-counted and interior-mutable Rust representations for arrays, slices, closures, and captured cells. In particular, recursive closure `self` currently uses a strong cell-to-closure link. The runtime implementation therefore must not be cited as proof that safe Tempra already prevents ownership cycles.

The immediate order is K → R → M → C → V → O → P, after completed E6, with U
conditional. K keeps comptime semantics canonical and backend-independent; it
does not yet choose the HIR interpreter, VM, JIT, Wasm or another evaluator as
authority. R is future Result/value-use hardening, not an effect. Phase M follows R and will define which ownership
graphs are legal across value semantics, `let`, `var`, `ref`, `inout`, `consume`,
closure captures, recursive `self`, structs, enums, and escaping values. It
precedes pluggable actor concurrency.

ARC optimization is a separate later concern. Escape analysis, stack promotion, uniqueness, reuse, and retain/release elimination belong to O and are not prerequisites for C.

E1–E6 complete the currently meaningful capability-contract and tooling core
while retaining known coverage gaps. E6 projects existing facts through
`tempra effects` and immutable driver types; it does not add analysis. K1
comptime authority/evaluator parity, determinism and initial responsiveness
measurements are next. Compile latency is monitored during development; deep
latency and incremental-build optimization belongs to P. The richer future TIR Graph still
requires later ownership and actor facts.

U — Unsafe Boundary is DEFERRED / CONDITIONAL standalone work at the end of the roadmap.
The trigger is the first real source operation requiring authorization to preserve
memory/type invariants; none is intentionally exposed today. Future local audited
islands must not automatically contaminate callers or waive deny/pure contracts.
The [historical E5 audit](../E1_CAPABILITY_ANALYSIS.md) records native signature,
comptime extern and internal JIT API debts separately from unsafe authorization.

## Major crate responsibilities

- `tempra-lexer`, `tempra-parser`, `tempra-ast`: surface syntax and AST construction.
- `tempra-hir`: normalized semantic representation and AST lowering.
- `tempra-typeck`: type checking, specialization facts, narrowing, interfaces, and pattern analysis.
- `tempra-driver`: module loading and orchestration of current execution and checking paths.
- `tempra-interpreter`: default checked-HIR runtime.
- `tempra-tir`, `tempra-tir-lower`, `tempra-tir-interp`: typed backend-facing IR, lowering, verification, and focused interpretation tests.
- `tempra-bytecode`, `tempra-bytecode-lower`, `tempra-vm`: experimental bytecode path.
- `tempra-native` and `std/`: native implementations and Tempra extern declarations for the current standard library.
- `tempra-diagnostics`: structured diagnostic data and rendering support.
- `tempra-analysis`, `tempra-lsp`: editor-facing semantic analysis and LSP behavior.
- `tempra-jit-cranelift`: opt-in JIT experiment over eligible bytecode functions.
- `tempra-effects`: pure TIR capability analysis, summaries, call edges, structured errors and deterministic provenance; E2 integration is owned by `tempra-driver`.

## Public semantic terminology

- **checked:** normal safe Tempra behavior, implicit and without a mode flag;
- **verified:** future stronger local proof through `@verified`;
- **unsafe:** future explicit low-level boundary through `@unsafe`.

There is no current Spark mode, global `--checked`, or global `--verified` design.

See the [roadmap](../roadmap.md), [pipeline](PIPELINE.md), [TIR specification](../TIR_SPEC.md), and [capability contract specification](../specs/effects-v0.1.md) for the corresponding details.
