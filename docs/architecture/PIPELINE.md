# Tempra Pipeline

## Current paths

Tempra does not yet have one fully unified execution and emission pipeline.

### Default interpreter

```text
source
  → lexer / parser
  → AST
  → HIR
  → reject capability contracts (execution only)
  → type checking
      → canonical comptime facade where evaluation is required
  → HIR interpreter
```

This is the default `tempra run` path and supports the broadest language surface. It explicitly rejects annotated programs rather than bypassing contracts. `tempra check` performs frontend/typechecking stages only: it neither lowers to TIR nor certifies capability contracts.

### Experimental VM

```text
source
  → lexer / parser
  → AST
  → HIR
  → type checking
      → canonical comptime facade and HIR pruning/materialization
  → TIR lowering
  → TIR verification
  → capability analysis (once; fresh compilation only)
  → internal AnalyzedTir
  → deny contract enforcement
  → bytecode lowering
  → bytecode verification
  → VmCompilation (TIR + capabilities + contracts + bytecode)
  → VM
  → optional Cranelift JIT for eligible bytecode functions
```

Experimental execution requires `main` and supports a narrower subset. The
compilation API can also return modules without `main`. Source fresh compilation
uses `compile_to_bytecode_with_analysis`; the legacy `compile_to_bytecode`
returns only its bytecode. Multi-module compilation uses
`compile_file_to_bytecode_with_analysis`, without a new cache.

### Analysis-independent paths

```text
source VM cache hit → semantic fingerprint + source identity + bytecode verification → execution
emit-tir           → TIR → verification → formatting
raw TIR interpreter → TIR → verification → interpretation
source TIR interpreter → reject annotated HIR → TIR → verify → interpret
```

Cache hits neither reconstruct TIR nor produce capability facts. Fresh source
compilation writes bytecode-only artifacts after enforcement succeeds. Neither
emission nor TIR interpretation requires extern capability coverage. HIR execution,
`test_source`, REPL and source TIR interpretation reject any capability contract.
They share `HirFile::contains_capability_contracts` through the interpreter's
unsupported-execution guard; REPL checks before registration or session mutation.
Frontend-only `check` remains non-enforcing.

## Phase E insertion point

Capability analysis belongs after semantic lowering and TIR verification:

```text
verified TIR
  → direct capability scan
  → call graph construction
  → fixed-point propagation
  → CapabilityAnalysis
  → later analyses and backends
```

E1 provides this transformation as a pure library operation. E2 integrates it
into fresh VM compilation, retaining the typed result. E3.2 adds enforcement
before bytecode lowering. TIR verification precedes analysis; verifier, analyzer,
contract violations and invariant failures retain structured errors in the driver.

E3.1 adds a parallel metadata flow, not another capability analyzer:

```text
@deny syntax → typed AST contract → HIR preserved
  → concrete function lowering → ID + declaration contract registration
  → VmCompilation contracts (alongside TIR, capability facts and bytecode)
```

E4 also preserves a separate `pure_span` in AST/HIR. Driver normalization alone
unions the four pure restrictions (Io, Process, Spawn, Nondeterminism) with explicit
deny. `DenyContract` and its spans remain explicit-deny-only; immutable driver
contracts retain both provenance forms. Alloc and Panic are allowed by pure alone.
The existing enforcer is unchanged. The single HIR presence scan includes pure,
so all existing execution guards reject it before execution/REPL state commit.
No new analysis, execution routing or instruction metadata is introduced.

The frontend does not depend on `tempra-effects`; canonical conversion belongs
to the driver. `contract(id)` and ordered `contracts()` expose explicit intent
immutably, including declaration/attribute spans and source/module identity.
Templates only acquire concrete bindings when their specializations are emitted.
E3.2 compares denied sets with transitive requirements and retains the unchanged
E1 witness per violation. Errors are ordered by function ID then capability.
Cache hits remain analysis/contract-free but require the shared
`tempra-{CARGO_PKG_VERSION}-semantics-1` fingerprint. Older artifacts miss;
fresh failures never populate or replace cache entries. The semantic revision
versions reuse guarantees, not bytecode format. See [cache policy](../BYTECODE_CACHE.md).

E4 keeps this fingerprint: pre-E4 parsing rejected pure and attribute edits alter
source identity. Pure does not add cache metadata. `check` remains frontend-only,
and emission of pure-annotated TIR does not certify the contract.

E2 retains structural provenance only: function/block/instruction identities,
call edges, direct origins and witnesses. TIR has no reliable source mapping.
The future bridge is a lowering-owned side table
`TirLocation → { module identity, existing Span }`, not per-instruction metadata.

Only operations present in TIR are analyzed. HIR-only builtins remain explicit coverage gaps; the architecture must not grow a parallel HIR capability analyzer.

## Comptime interaction

The K1 audit selected the existing HIR interpreter as the canonical comptime
semantic kernel. K1.1 places a single facade in `tempra-interpreter` around that
kernel. The facade alone starts a comptime session, installs the current fuel
policy, normalizes its boundary result, and restores prior interpreter state.
`tempra-typeck` consumes this API and retains ownership of value-to-HIR-literal
materialization. A nested `HirExpression::Comptime` continues within the active
session and shares its remaining budget.

The typechecker removes unselected `if comptime` branches and `comptime match`
arms from HIR. TIR therefore contains only the selected semantic branch where
specialization is available. Capability analysis may rely on that semantic
pruning, but it must not assume later dead-code elimination, inlining, escape
analysis, or allocation elimination.

K1.1 covers the existing working single-source entrypoints only. It does not
initialize the evaluator for multi-module checking, file-based `check`, analysis,
LSP or REPL typechecking. Arithmetic, extern/effect policy, recursion and
specialization hardening also remain pending K1/K2 work.

## Planned backends

MIR, general Tempra IR, native AOT, and dedicated Wasm output remain incomplete. The intended long-term direction is that verified TIR and stable per-function semantic facts feed backend-independent later analyses before target-specific lowering.

Cranelift remains native-only. Dedicated Wasm generation must not depend on Cranelift. Wasm hosting and Wasm code generation remain separate concerns.

## Language analysis order

```text
E capability analysis
  → R Result / value-use hardening
  → M acyclic ownership
  → C pluggable actor concurrency boundaries
  → V verified functions
  → O deeper memory optimizations
```

This ordering separates memory correctness from memory optimization. R is a
future value-use obligation, not a capability or an implemented pass.

E1–E6 complete the currently meaningful semantic and tooling surface; TIR
coverage gaps remain. `tempra effects` projects existing analysis, contracts,
violations, witnesses and provenance without a second analyzer. A richer TIR
Graph requires later ownership/actor facts. The K1 audit selected the HIR
interpreter kernel as the backend-independent comptime authority, and K1.1 now
provides its canonical evaluation facade. Remaining K1 work must connect the
other frontend paths and harden semantics before parity, deterministic
reproducibility and phase-timing/latency measures can close K. Deep compilation
optimization and incremental builds belong to P.

U — Unsafe Boundary is standalone DEFERRED / CONDITIONAL work at the end of the roadmap,
in the roadmap and triggered only by a real source operation requiring explicit
memory/type-safety authorization. No current E1 origin emits Unsafe, and no
unsafe pass or syntax is inserted into this pipeline.
