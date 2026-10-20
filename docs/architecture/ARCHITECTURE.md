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
      → canonical comptime facade when evaluation is required
  → HIR interpreter
```

`tempra run` uses this path unless the experimental VM is selected. Annotated execution is explicitly unsupported here, not silently unchecked. `tempra check` currently stops after parsing, HIR lowering, module resolution, and type checking; it does not certify capability contracts.

### Experimental VM path

```text
Source
  → Lexer / Parser
  → AST
  → HIR lowering
  → Type checking
      → canonical comptime facade and HIR pruning/materialization
  → TIR lowering
  → TIR verification
  → Capability analysis (broad and specialized facts, fresh compilation)
  → Broad and specialized contract enforcement
  → Bytecode lowering
  → Bytecode verification
  → VmCompilation (retained TIR, capability facts, bytecode)
  → Bytecode VM
  → optional experimental Cranelift JIT for eligible functions
```

TIR support covers a smaller language subset than the HIR interpreter. Generic templates are not emitted directly; requested concrete specializations are lowered where supported.

This is the fresh source/multi-module VM path. K2.2 processes unique
specialization requests in deterministic first-request order. Bytecode lowering
emits functions by ascending `TirFunctionId` and resolves calls through an
explicit `TirFunctionId`-to-`FunctionId` map; TIR identity is never inferred from
physical vector position. Bytecode cache hits verify and
execute cached bytecode without analysis or reconstructed TIR, but require an
enforcement-aware semantic fingerprint and matching source identity. TIR emission
verifies then formats. Source TIR interpretation rejects contracts before lowering;
raw TIR interpretation verifies then interprets. Neither needs analyzer coverage.
HIR execution, REPL and test execution reject annotated input through one shared
HIR presence scan before execution/state commit. See [pipeline boundaries](PIPELINE.md).

File/module compilation receives a resolved standard-library root through
`CompilationConfig`. The root denotes the `std/` directory itself; the module
resolver only maps `import std.path` to `<root>/path.tmpr` and does not perform
environment discovery. The default configuration discovers the nearest `std/`
ancestor of the current compiler executable, never the process cwd. Embedders and
tests may provide an explicit root; a relative explicit path is made absolute once
at that API boundary. Analysis/LSP retain their existing adapter boundary and do
not make `CompilationConfig` a new frontend authority.

### Incomplete planned paths

The `tempra-mir`, `tempra-ir`, `tempra-opt`, native AOT, Wasm backend, linker, and build crates contain varying levels of scaffold or prototype code. They are not a single production pipeline today. Historical design documents that describe them as complete do not override this status.

## Canonical comptime authority

The K1 audit selected the existing HIR interpreter as the semantic kernel for
comptime. K1.1 adds a single public compiler-facing facade in
`tempra-interpreter`; it owns comptime mode and fuel installation, boundary
normalization, and restoration of prior interpreter state. `tempra-typeck`
invokes that facade and separately materializes supported primitive results into
HIR. Nested comptime syntax shares the active evaluation session.

This establishes one entrypoint, not complete K1 semantics. K1.2 wires the same
facade into ordinary multi-module and file-based typechecking through a
module-aware lookup adapter while retaining separate HIR files. Analysis/LSP
and REPL typechecking remain deferred. K1.3 makes supported shared-kernel integer
arithmetic panic-free. K1.4 rejects functions without executable HIR bodies,
including extern declarations reached during comptime, instead of synthesizing
`Unit`; it does not route comptime through the VM native resolver. Recursion and
specialization behavior remain later hardening. K1.5 applies one
interpreter-boundary policy to currently executable host-observable operations:
deterministic local work, allocation and `cmd` construction remain allowed,
while process execution, output/input, filesystem, environment, host-OS access,
`dbg` and `exit` are rejected. TIR, VM, JIT,
Wasm and future AOT remain consumers of already resolved semantics rather than
comptime authorities. K1.6 centralizes deterministic evaluator-progress fuel:
expression/statement entries, Tempra call frames and executed `for` bodies are
explicit ticks, while builtins remain constant-cost in v0.1. A call-depth guard
allows 8 active frames by default and rejects the 9th; nested comptime shares
the active session. This is a conservative compiler-defined v0.1 safety limit,
revisionable as the evaluator evolves, not
a timeout, wall-clock limit, or resource quota. K1.7 completes the K1 trust
boundary by requiring `semantics-2`; revision-1 artifacts miss and must pass
through fresh K1 evaluation. K2.2 raises current trust to `semantics-3` so older
artifacts cannot retain the specialization ID/index miscompilation. K2.4 raises
current trust to `semantics-4` after making interface selection and requirement/
field diagnostics follow canonical source/module declaration order. These semantic
revisions are independent of bytecode format version.

K3.2 raises current trust to `semantics-5` because checked `Int64` add/sub/mul
now contribute `Panic` and revision-4 hits could bypass that enforcement.
K4.5B raises current trust to `semantics-6` because revision-5 hits could bypass
static specialized `@deny` enforcement. The shared `tempra-runtime` arithmetic
contract is consumed by the HIR interpreter, TIR interpreter and VM. Eligible JIT functions implement equivalent checked
Cranelift operations and propagate a C-compatible `{ value, status }` result
through pure and heap-aware native calls. A non-zero status from native code is
returned to the VM as a typed runtime failure and is never reclassified as
fallback or re-executed in the interpreter.

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
without changing the execution adapters. `semantics-1` remained valid at that
point because the old parser rejected pure and changed annotations change source
identity.

### K4.6 specialized refinements (implemented)

K4.6 adds compiler-owned specialized operations as a refinement over broad
capabilities. `Capability`, `CapabilitySet`, `@pure`, broad `@deny` and bytecode
are unchanged. Specialized clauses live in the compilation-local contract side
table and reuse the same E1 summaries, call graph and witnesses. Operation
identity and resource knowledge stay separate: exact literals and `*` are
supported; unknown resources, missing classification and unclassified operations
under a denied parent fail closed. Different known operations are disjoint.
Broad violation results remain authoritative and are returned before specialized
results.

Classification is explicit compiler metadata, not inference from native names or
types. The production `tempra-native` registry and the `tempra-effects`
classifier remain separate tables; K4.6B.1 cross-crate tests require exact name,
parent-capability, signature and resource-position agreement. Resolver identity
is trusted, native code is not sandboxed and native handlers still validate their
arguments manually.

Enforcement runs before bytecode lowering. VM, profiled VM and JIT consume only
already-enforced bytecode; there is no runtime specialized-effect guard. HIR,
test, REPL and source-TIR execution reject any annotated input; `check` and
`emit-tir` remain non-enforcing tooling. Effect inspection compiles fresh and
enforces, but specialized violations currently surface as a generic compiler
error rather than an E6 structured diagnostic; the E6 schema is unchanged.

K4.5B raised cache trust to `semantics-6`, so revision-5 artifacts retaining
unenforced specialized clauses cannot be reused. Hits remain trusted
bytecode-only artifacts with no reconstructed facts. Filesystem parameters are
opaque source spellings: no canonicalization, symlink/hard-link confinement,
mount-alias protection or TOCTOU guarantee is claimed. K4.6A found no bypass;
K4.6B.2 covers fresh source/file, VM/profiling/JIT, cache, unsupported paths,
unknown externs and inspection boundaries. K4.6C records the guarantees in
[Specialized Effects v0.1](../specs/specialized-effects-v0.1.md). K4.7A audited
differential coverage, K4.7B added focused source/file, enforcement, determinism,
inspection and VM/JIT regressions, and K4.7C established independent-process
canonical violation parity. The gate verdict is **PASS** for supported
same-build, same-host/target paths. Native `std.*` calls remain outside JIT v1:
the calling entry is rejected and the interpreter executes the native call. This
is documented as an unsupported execution path, not an enforcement bypass; no
runtime guard or JIT architecture change was introduced. K4.8 owns the full
per-operation coverage-gap ledger.

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

`Alloc` classification follows semantic runtime allocation in the current representation. TIR string constants and construction instructions are not normative allocation sites without runtime confirmation. `Panic` is a conservative may-fail capability in E1, including checked `Int64` arithmetic; it is not a runtime error hierarchy. Later verified analysis may prove individual operations safe.

## Memory architecture boundary

The current HIR runtime uses reference-counted and interior-mutable Rust representations for arrays, slices, closures, and captured cells. In particular, recursive closure `self` currently uses a strong cell-to-closure link. The runtime implementation therefore must not be cited as proof that safe Tempra already prevents ownership cycles.

The immediate order is K → R → M → C → V → O → P, after completed E6, with U
conditional. K keeps comptime semantics canonical and backend-independent. Its
audit selected the existing HIR interpreter kernel, and K1.1 provides the
canonical session facade without making a runtime backend authoritative. R is
future Result/value-use hardening, not an effect. Phase M follows R and will define which ownership
graphs are legal across value semantics, `let`, `var`, `ref`, `inout`, `consume`,
closure captures, recursive `self`, structs, enums, and escaping values. It
precedes pluggable actor concurrency.

ARC optimization is a separate later concern. Escape analysis, stack promotion, uniqueness, reuse, and retain/release elimination belong to O and are not prerequisites for C.

E1–E6 complete the currently meaningful capability-contract and tooling core
while retaining known coverage gaps. E6 projects existing facts through
`tempra effects` and immutable driver types; it does not add analysis. K1's
audit/spec and K1.1–K1.7 implementation are complete, including the semantic
cache revision that prevents older artifacts from bypassing current comptime
guarantees. K2.1's audit, K2.2's specialization ordering/function-ID integrity,
K2.3's explicit standard-library discovery input, K2.4's deterministic
interface/field iteration and K2.5's cross-process byte-for-byte gate are complete.
Level 3 applies to the supported same-build, same-host/target bytecode surface;
cross-machine Level 4 reproducibility is not claimed.
Runtime
parity follows in K3, and responsiveness
measurements in K4–K6. Compile latency is monitored during development; deep
latency and incremental-build optimization belongs to P. The richer future TIR Graph still
requires later ownership and actor facts.

U — Unsafe Boundary is DEFERRED / CONDITIONAL standalone work at the end of the roadmap.
The trigger is the first real source operation requiring authorization to preserve
memory/type invariants; none is intentionally exposed today. Future local audited
islands must not automatically contaminate callers or waive deny/pure contracts.
The [historical E5 audit](../E1_CAPABILITY_ANALYSIS.md) records native signature,
the now-resolved comptime bodyless-dispatch gap, and internal JIT API debts
separately from unsafe authorization.

## Major crate responsibilities

- `tempra-lexer`, `tempra-parser`, `tempra-ast`: surface syntax and AST construction.
- `tempra-hir`: normalized semantic representation and AST lowering.
- `tempra-typeck`: type checking, specialization facts, narrowing, interfaces, and pattern analysis.
- `tempra-driver`: module loading and orchestration of current execution and checking paths.
- `tempra-runtime`: shared host-safe runtime contracts, including checked `Int64` arithmetic.
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
