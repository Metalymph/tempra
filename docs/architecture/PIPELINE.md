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
Cache hits remain analysis/contract-free. E3.2 originally introduced the
`tempra-{CARGO_PKG_VERSION}-semantics-1` fingerprint; K1.7 raised it to revision
2, K2.2 raised current trust to revision 3, K2.4 raised it to revision 4, K3.2
raised it to revision 5, K4.5B raised it to revision 6, and K4.9D requires
revision 7; K3.3 requires revision 8. Older artifacts miss;
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

K1.2 now uses the same canonical facade for ordinary multi-module and file-based
typechecking. The interpreter receives separate module-owned HIR files and a
qualified lookup context; no synthetic aggregate HIR file is created.
`tempra-analysis`, LSP and REPL typechecking remain on their existing
non-comptime-aware paths. K1.3 routes supported integer arithmetic through a
panic-free shared kernel. K1.4 makes shared HIR call dispatch reject missing
executable bodies, including comptime calls to extern declarations, without
adding VM-native dispatch to the HIR evaluator. Further recursion and
specialization hardening remain later K1/K2 work. K1.5 applies one policy at
the interpreter's effect boundary: deterministic local work, allocation and
`cmd` construction remain allowed, while process execution, output/input,
filesystem, environment, host-OS access, `dbg` and `exit` are rejected.
K1.6 adds a deterministic evaluator-progress budget and a separate
compiler-defined call-depth guard. Expression and statement entries, one
entry per Tempra call frame, and each `for` body that receives an item consume
one fuel tick; an exhausted iterator does not create a phantom tick. The
default safety limit is 8 active frames, nested comptime shares the active
session, and builtins have constant semantic cost in v0.1. The compiler-defined
call-depth limit is revisionable as the evaluator evolves. These limits bound
evaluator progress only: there is no wall-clock, per-byte, CPU or memory quota.
The current HIR has no `while` surface. K1.7 introduced `semantics-2` so
revision-1 artifacts cannot bypass fresh K1 evaluation. K2.2 processes
specializations in deterministic first-request order and requires `semantics-3`
so revision-2 artifacts cannot preserve the former TIR-ID/bytecode-position
miscompilation. K2.4 makes interface candidate selection and requirement/field
diagnostics follow canonical source/module declaration order and requires
`semantics-4` so revision-3 artifacts cannot preserve unordered interface
selection. Bytecode format version and semantic revision remain independent;
compatible hits still
produce verified bytecode only, without reconstructing TIR or compiler facts.

K3.2 requires `semantics-5`: verified TIR capability analysis classifies checked
`Int64` add/sub/mul/div as conservative `Panic`, and the supported HIR, TIR, VM
and native JIT execution paths share one controlled overflow/division boundary.
The JIT propagates arithmetic status through native callees rather than
continuing with a placeholder or falling back. K4.5B requires `semantics-6` so
revision-5 artifacts retaining unenforced specialized `@deny` clauses cannot be
reused; specialized matching reuses the existing analysis and witness pipeline.
Bytecode opcodes and serialization format remain unchanged.

K4.9D requires `semantics-7` for v0.2 family/category enforcement. K4.9E confirms
that source hash, fingerprint and bytecode verification remain the cache-hit
boundary. Invalid call targets now produce a controlled stack-effect error
during verification, allowing cache fallback instead of a host indexing panic.
Valid trusted hits remain bytecode-only and do not re-enforce source contracts.
Family/category predicates, broad-first precedence, conservative unknown facts
and v0.1 resource-qualified operation matching use the existing analysis path.
Single-source compilation binds supported extend methods; module/file
compilation explicitly rejects annotated extend methods before their omitted
lowering could discard contracts. HIR/REPL/source-TIR annotation guards and
non-enforcing `check`/`emit-tir` boundaries remain unchanged. E6's specialized
count-only error and missing specialized inspection fields remain documented
tooling limits in [the tooling guide](../TOOLING.md).

K3.3 requires `semantics-8` for numeric backend parity. Verified TIR types are
retained for bytecode lowering so `Int64`, `UInt64` and `Float64` arithmetic and
ordering select their matching opcodes instead of defaulting to `Int64`. The
verifier rejects arithmetic on unsupported or unresolved numeric types before
emission. `UInt64` add/sub/mul/div use the shared checked runtime kernel and
`Float64` preserves IEEE 754 division, NaN, infinities, signed zero and ordered
comparisons. HIR, TIR interpretation, bytecode VM and source-level differential
tests agree. Native JIT support remains limited to eligible `Int64` functions;
`UInt64`/`Float64` signatures are rejected and interpreted, without native
execution. Effect analysis adds checked `UInt64` add/sub/mul to the conservative
`Panic` origins, while every `Div` remains `Panic`. TIR/bytecode formats are
unchanged.

K4.10's [final gate](../K4_SPECIALIZED_EFFECTS_GATE.md) confirms these boundaries
with ordinary/JIT workspace tests and existing differential subprocess coverage.
Malformed nonempty bytecode functions with more parameters than local slots now
fail the verifier's existing InvalidLocalId boundary instead of indexing outside
local state; matching cache headers still lead to a verified fresh rebuild for
that case. `semantics-7`, bytecode format, matching and valid runtime behavior are
unchanged. Production-classified operations remain FsRead, FsWrite and IoWrite;
the other taxonomy identities are not claimed as executable native coverage.

`TirFunctionId` is a compilation-local identity rather than an index into the
physical function vector. Bytecode lowering uses an explicit mapping for calls
and emits functions in ascending identity order. Filtering specialization
requests by owning module preserves the registry's deterministic order; no
unordered regrouping participates in emission.

The file/module entrypoints resolve standard-library location before module graph
construction. `CompilationConfig` carries either an explicit absolute `std/` root
or the default found relative to the current executable. The module resolver has
no cwd fallback: it consumes that root unchanged, and a standard-library import
without one produces a controlled diagnostic. Explicit relative roots are resolved
once by the configuration API, so cwd may interpret a user-supplied relative path
but never discovers std implicitly. This input does not participate in the
single-source bytecode cache because that cache path does not construct a module
graph; the semantic revision was then `4`.

K2.5 verifies the supported bytecode path as a whole in independent compiler
processes. With source/module order, build, host/target, options and resolved stdlib
contents held constant, simple, multi-function, multi-module, specialization,
comptime, capability-contract and stdlib fixtures produce identical raw artifacts
and runtime results. Equivalent absolute roots, cwd changes after path resolution,
inode/mtime differences and cache state do not change those artifacts. Interface
selection and diagnostic ordering are checked at the frontend boundary where TIR/
bytecode lowering does not yet support that surface. This bounded Level 3 guarantee
does not imply Level 4 reproducibility across machines or targets.

## Planned backends

MIR, general Tempra IR, native AOT, and dedicated Wasm output remain incomplete. The intended long-term direction is that verified TIR and stable per-function semantic facts feed backend-independent later analyses before target-specific lowering.

Native AOT is first-class milestone A in the [roadmap](../roadmap.md); the existing Cranelift backend scaffold is its initial direction. A may progress incrementally and does not wait for O, NR or P. NR is standalone experimental RC-free research and never blocks baseline AOT. Dedicated Wasm/WASI remains a separate track and must not depend on Cranelift. Wasm hosting and Wasm code generation remain separate concerns.

## Language analysis order

The [roadmap](../roadmap.md) is authoritative for sequencing, statuses,
dependencies and acceptance gates. The layered model is:

```text
Semantic: E → R → M → M5 → C → V
Backend:  A (native AOT, first-class; Cranelift initial)
Optimization: O after M ownership facts; P after K5/K6 measurements
Research: NR (standalone experimental RC-free; never blocks A)
Conditional: U
```

C (pluggable actor concurrency) implementation requires the M5 memory
stabilization gate. A does not wait for O, NR or P. This ordering separates
memory correctness from memory optimization. R is a future value-use obligation,
not a capability or an implemented pass.

E1–E6 complete the currently meaningful semantic and tooling surface; TIR
coverage gaps remain. `tempra effects` projects existing analysis, contracts,
violations, witnesses and provenance without a second analyzer. A richer TIR
Graph requires later ownership/actor facts. The K1 audit selected the HIR
interpreter kernel as the backend-independent comptime authority, and K1.1 now
provides its canonical evaluation facade. K1 and K2 are complete; K3 runtime
parity is IN PROGRESS for the currently executable HIR/TIR/VM/JIT surfaces,
K4 is DONE, and K5/K6 timings and budgets are PLANNED. Wasm and AOT join
runtime parity when those backends exist. Deep compilation optimization and
incremental builds belong to P.

U — Unsafe Boundary is standalone DEFERRED / CONDITIONAL work in the
[roadmap](../roadmap.md), triggered only by a real source operation requiring
explicit memory/type-safety authorization. No current E1 origin emits Unsafe, and no
unsafe pass or syntax is inserted into this pipeline.
