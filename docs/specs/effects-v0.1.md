# Tempra Capability Contracts v0.1

> [!NOTE]
> **Status: E1, E2 and E3.1 implemented.** `@deny(...)` is parsed and retained as typed intent, without enforcement. E3.2 enforcement, `@pure`, `@unsafe` and `@verified` remain unimplemented.

Tempra's intended capability-contract surface is small rather than a user-facing algebraic effect language. Only `@deny(...)` is currently accepted; the complete planned surface is:

- `@deny(...)`
- `@pure`
- `@unsafe`

There is no public `effect`, `handle`, `perform`, `allow`, `@no_alloc`, `@no_panic`, `@deterministic`, or family of aliases.

## `@deny(...)`

```tempra
@deny(alloc, panic)
fn hot_path(value: Int) -> Int {
    normalize(value)
}
```

`@deny(...)` expresses a compile-time contract. Once E3.2 implements enforcement,
the annotated function and every operation reachable through its calls must not
require a denied capability. **E3.1 only acquires this intent:** compilation
currently succeeds even when the retained summary requires a denied capability.

### Current syntax and placement (E3.1)

Every attribute requires at least one case-sensitive name from `alloc`, `panic`,
`io`, `process`, `spawn`, `nondeterminism`. `@deny()` and `@deny(unsafe)` are errors.
Unknown attributes, names and non-identifier arguments are rejected. Whitespace,
newlines and a trailing comma are accepted. Names remain ordinary identifiers
outside attributes.

Repeated names are deduplicated. Repeated attributes merge by union, retaining
all attribute spans in source order. An empty attribute is invalid even after
another valid attribute. Absence of `@deny` produces no user contract.

Place attributes before the optional `pub` on named top-level functions or
methods inside `extend`. They cannot decorate externs, closures, interface
requirements, structs/enums, fields, imports, statements or `extend` itself.
There is no new method syntax inside structs or local named-function syntax.

Frontend `DenyCapability` is validated syntax only, not an analyzer enum. HIR
preserves it; only the driver converts it to canonical `CapabilitySet`.
No frontend crate depends on `tempra-effects`.

Capability requirements propagate transitively. If `A` calls `B`, `B` calls `C`, and `C` may allocate, then `Alloc` is required by `C`, `B`, and `A`.

The analysis does not promise general optimization-sensitive elimination. It observes the semantic TIR presented to it. Operations already removed before TIR—such as unselected `comptime` branches pruned from HIR—do not contribute; operations that merely appear optimizable still contribute.

## `@pure`

`@pure` is the only convenience contract:

```text
@pure = @deny(io, process, spawn, nondeterminism)
```

Pure code may allocate. Pure code is not automatically panic-free.

## `@unsafe`

Tempra is safe and checked by default. `@unsafe` marks an explicit boundary where low-level operations unavailable to normal safe Tempra may be used.

Unsafe is conceptually separate from capability denial:

- `@unsafe` does not waive a `@deny(...)` contract;
- `@pure` does not authorize unsafe operations;
- `unsafe` is not a public argument accepted by `@deny(...)`;
- this phase does not define the complete unsafe subsystem.

## Internal capability vocabulary

The initial internal vocabulary is:

- `Alloc`
- `Panic`
- `IO`
- `Process`
- `Spawn`
- `Nondeterminism`
- `Unsafe`

The public denial spellings are `alloc`, `panic`, `io`, `process`, `spawn`, and `nondeterminism`. `Unsafe` remains internal and is represented publicly only by the separate `@unsafe` boundary.

The vocabulary is intentionally broader than current TIR coverage. Phase E must record unmapped operations as coverage gaps rather than invent semantics.

## Analysis point

The current default runtime evaluates checked HIR directly. Fresh experimental
VM compilation lowers checked HIR to TIR, analyzes verified TIR, then produces bytecode:

```text
Source
  ↓
Parser / AST
  ↓
HIR
  ↓
Type checking and comptime pruning
  ↓
TIR
  ↓
TIR verification
  ↓
Capability analysis
  ↓
Later analyses and backends
```

E1 and E2 do not add an HIR analyzer. Operations that cannot yet reach TIR remain explicit coverage gaps, including after E2 integration.

## Function summaries

E1 produces stable per-function semantic facts containing at least direct and transitively required capability sets. The exact Rust representation belongs to `tempra-effects`; it must not be embedded in surface syntax or bytecode formats.

Later consumers include acyclic ownership analysis, concurrency-boundary verification, `@verified`, diagnostics, and tooling. These consumers must reuse the summaries and call graph rather than rediscovering transitive calls independently.

## Direct classification policy

E1 classifies operations conservatively according to current runtime semantics:

- `Alloc` applies only when the current representation semantically requires runtime allocation. A TIR constant or construction is not automatically allocating; string constants and composite construction must be confirmed against the runtime and lowering.
- `Panic` is an initial may-panic classification. Division, unwraps, indexed accesses, and slices may require it even when later proof could establish safety.
- known filesystem and console externs introduce `IO`;
- `Process`, `Spawn`, `Nondeterminism`, and `Unsafe` currently have no complete TIR mapping.

Phase V may prove that a specific division or bounds access cannot fail. That future proof discharges a verification obligation; it does not change E1's conservative collection semantics.

An unknown extern declaration does not fail analysis by itself. `UnclassifiedExtern` is reported only when a TIR call actually targets an extern without a capability classification. E1 analyzes every TIR function, so calls from non-entry functions are still checked; entirely unused extern declarations are ignored.

Current HIR-only operations—including process commands, environment access, input, exit, and parts of `os`—remain documented gaps until they have a stable TIR representation.

## Propagation and provenance

E1 constructs a call graph from TIR calls, initializes each function with its direct capabilities, and propagates callee requirements to callers until a fixed point. This finite monotone analysis handles recursion and strongly connected components.

The result retains direct origins and callsite coordinates using stable TIR identities: function ID, block ID, and instruction index. A deterministic shortest witness path, with structural IDs as tie-breakers, must remain reconstructable for diagnostics such as:

```text
error: function 'hot_path' denies capability 'alloc'
required capability:
  alloc
introduced by:
  normalize
call path:
  hot_path
  └── normalize
      └── Array.new
```

E1 need not implement this presentation or source-span mapping. It must preserve the structured information required by E2 and E6.

## Phase E milestones

1. **E1:** internal capability model and pure TIR analysis;
2. **E2 (implemented):** post-verification integration in fresh VM compilation, retaining immutable TIR, capability facts and bytecode in `VmCompilation`;
3. **E3:** typed `@deny(...)` acquisition and declaration provenance (E3.1 implemented), then enforcement and contract witnesses (E3.2 pending);
4. **E4:** `@pure`;
5. **E5:** `@unsafe` integration;
6. **E6:** diagnostics, inspection, and structured output.

E1 remains a pure `tempra-effects` crate depending only on `tempra-tir`.
E2 invokes it once after verification in fresh source and multi-module VM
compilations. Capability presence does not reject compilation; structural
analysis failures are retained as typed driver errors. The bytecode cache
stores no facts: cache hits verify and execute bytecode without analysis.
TIR emission and TIR interpretation verify TIR without analyzing it; HIR paths
remain analyzer-independent. E3.1 parses `@deny`; no enforcement, new diagnostic UI
or capability CLI output exists.

E2 retains structural TIR locations, call edges, direct origins and witnesses,
not source spans. TIR has no reliable source map. The future bridge is a
lowering side table from `TirLocation` to module identity plus existing `Span`,
not instruction metadata or debug-name interpretation. E1 classification and
coverage limits remain unchanged. See [integration details](../E1_CAPABILITY_ANALYSIS.md#e2-integration-implemented).

E3.1 adds a declaration-only side table to `VmCompilation`, exposed through
`contract(id)` and ordered `contracts()`. It retains canonical denied sets,
declaration and attribute spans, and source/module identity. Binding uses the
actual HIR declaration together with the ID just returned by lowering, including
concrete specializations. It does not add spans or contracts to `TirModule`.

### Mandatory E3.2 cache gate

E3.1 leaves cache behavior and artifact format unchanged because it does not
enforce contracts. **E3.2 must prevent cache hits from bypassing `@deny`.**
Bytecode cached by a pre-enforcement compiler must not execute unchecked after
an E3.2 upgrade. E3.2 must choose and validate semantic/version invalidation,
sufficient verified artifact metadata, or a necessary cache bypass against the
actual repository. No strategy is implemented or selected in E3.1.
