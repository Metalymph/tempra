# Tempra Pipeline

## Current paths

Tempra does not yet have one fully unified execution and emission pipeline.

### Default interpreter

```text
source
  → lexer / parser
  → AST
  → HIR
  → type checking
  → HIR interpreter
```

This is the default `tempra run` path and supports the broadest language surface. `tempra check` performs the frontend and semantic stages but does not currently lower to TIR.

### Experimental VM

```text
source
  → lexer / parser
  → AST
  → HIR
  → type checking and comptime pruning
  → TIR lowering
  → TIR verification
  → capability analysis (once; fresh compilation only)
  → internal AnalyzedTir
  → bytecode lowering
  → bytecode verification
  → VmCompilation (TIR + capabilities + bytecode)
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
source VM cache hit → cached bytecode verification → execution
emit-tir           → TIR → verification → formatting
TIR interpreter    → TIR → verification → interpretation
```

Cache hits neither reconstruct TIR nor produce capability facts. Fresh source
compilation still writes bytecode-only artifacts. Neither emission nor TIR
interpretation requires extern capability coverage. The default HIR runtime and
frontend-only `check` also remain outside capability analysis.

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
into fresh VM compilation, retaining the typed result without enforcement or
UI. TIR verification precedes analysis; both retain structured errors in the
driver. No path currently enforces capability contracts.

E3.1 adds a parallel metadata flow, not another capability analyzer:

```text
@deny syntax → typed AST contract → HIR preserved
  → concrete function lowering → ID + declaration contract registration
  → VmCompilation contracts (alongside TIR, capability facts and bytecode)
```

The frontend does not depend on `tempra-effects`; canonical conversion belongs
to the driver. `contract(id)` and ordered `contracts()` expose explicit intent
immutably, including declaration/attribute spans and source/module identity.
Templates only acquire concrete bindings when their specializations are emitted.
E3.1 cache hits remain analysis/contract-free; E3.2 must introduce and validate a
cache non-bypass policy for older non-enforcing artifacts before enforcement ships.

E2 retains structural provenance only: function/block/instruction identities,
call edges, direct origins and witnesses. TIR has no reliable source mapping.
The future bridge is a lowering-owned side table
`TirLocation → { module identity, existing Span }`, not per-instruction metadata.

Only operations present in TIR are analyzed. HIR-only builtins remain explicit coverage gaps; the architecture must not grow a parallel HIR capability analyzer.

## Comptime interaction

The typechecker removes unselected `if comptime` branches and `comptime match` arms from HIR. TIR therefore contains only the selected semantic branch where specialization is available. Capability analysis may rely on that semantic pruning, but it must not assume later dead-code elimination, inlining, escape analysis, or allocation elimination.

## Planned backends

MIR, general Tempra IR, native AOT, and dedicated Wasm output remain incomplete. The intended long-term direction is that verified TIR and stable per-function semantic facts feed backend-independent later analyses before target-specific lowering.

Cranelift remains native-only. Dedicated Wasm generation must not depend on Cranelift. Wasm hosting and Wasm code generation remain separate concerns.

## Language analysis order

```text
E capability analysis
  → M acyclic ownership
  → C concurrency boundaries
  → V verified functions
  → O deeper memory optimizations
```

This ordering separates memory correctness from memory optimization.
