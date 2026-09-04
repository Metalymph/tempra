# Tempra Capability Contracts v0.1

> [!NOTE]
> **Status: E1–E4 implemented.** E3 acquires/enforces capability contracts and prevents execution bypass. E4 normalizes `@pure` through the same machinery. `@unsafe` and `@verified` remain unimplemented.

Tempra's current capability-contract surface is small rather than a user-facing algebraic effect language. E1–E4 complete the currently meaningful surface, without closing TIR coverage gaps:

- `@deny(...)`
- `@pure`

The next active milestone is E6 diagnostics/inspection. `@unsafe` is a separate
conditional future boundary under U, not an immediate Phase E requirement.

There is no public `effect`, `handle`, `perform`, `allow`, `@no_alloc`, `@no_panic`, `@deterministic`, or family of aliases.

## `@deny(...)`

```tempra
@deny(alloc, panic)
fn hot_path(value: Int) -> Int {
    normalize(value)
}
```

`@deny(...)` expresses a compile-time contract: the annotated function and every
operation reachable through its calls must not require a denied capability.
E3.2 rejects fresh VM compilation when `denied` intersects the function's
transitive `required` set, including contracts on non-entry functions and each
emitted specialization. Without an intersecting contract, capability use remains valid.

Execution paths that cannot enforce contracts reject annotated input explicitly:
the HIR runner, REPL, test runner and `interpret_tir_source`. This includes unused
annotated declarations and generic templates. There is no automatic backend routing.
`emit-tir` may emit verified IR without enforcing contracts; emission is not
execution or certification. `check` currently certifies frontend/type correctness
only, not capability contracts. Raw TIR interpretation has no source-contract metadata.

### Current syntax and placement (E3.1)

Every `@deny` attribute requires at least one case-sensitive name from `alloc`, `panic`,
`io`, `process`, `spawn`, `nondeterminism`. `@deny()` and `@deny(unsafe)` are errors.
Unknown attributes, names and non-identifier arguments are rejected. Whitespace,
newlines and a trailing comma are accepted. Names remain ordinary identifiers
outside attributes.

Repeated names are deduplicated. Repeated attributes merge by union, retaining
all attribute spans in source order. An empty attribute is invalid even after
another valid attribute. Absence of both `@deny` and `@pure` produces no user contract.

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

Pure code may allocate, conservatively require `Panic`, and return `Result`.
`@pure` does **not** deny `Alloc`, `Panic` or internal `Unsafe`; it also does not
authorize unsafe operations. This is not a totality or verified-proof annotation.

Write bare `@pure`, before optional `pub`, on the same named top-level functions
and `extend` methods supported by `@deny`. It takes no parentheses or arguments:
`@pure()` and `@pure(io)` are errors. `pure` remains an ordinary identifier outside
attribute position. A repeated `@pure` is rejected at the second annotation,
including when `@deny` attributes occur between them.

Composition is an unordered union:

```tempra
@pure
@deny(panic)
fn add(a: Int, b: Int) -> Int { return a + b }
```

Reversing the attributes has the same effective restrictions. Explicitly denying
`io` alongside pure is redundant but valid; adding `alloc` or `panic` restricts
capabilities that pure alone allows. E1 still classifies division as may-panic,
even for a known nonzero divisor.

AST and HIR retain a separate `pure_span`; `DenyContract` still represents only
explicit `@deny` syntax. Only the driver defines the canonical four-capability
pure set. `FunctionCapabilityContract.denied` is the effective union;
`attribute_spans` contains explicit deny spans only, while `pure_span` identifies
the original pure marker. A pure-only contract has no synthetic deny spans.
Declaration spans and module/source identity retain their existing meaning.

E3.2's unchanged intersection/witness machinery enforces pure directly and
transitively, including non-entry functions and emitted specializations. There
is no purity analyzer or special enforcer branch. Process, Spawn and
Nondeterminism coverage gaps remain E1 gaps; E4 does not invent direct sources.

The shared HIR contract-presence scan includes pure. HIR/source-TIR execution,
test execution and REPL reject it explicitly even when its contract would pass;
REPL state remains unchanged. `emit-tir` and frontend-only `check` accept it
without certifying contracts. Raw TIR interpretation remains unchanged.

## U — Unsafe Boundary (DEFERRED / CONDITIONAL)

Tempra is safe and checked by default. The historical E5 audit found no intentional
source-level primitive requiring unsafe authorization. `@unsafe` is not accepted
or implemented. U is a standalone milestone listed after O in the
[roadmap](../roadmap.md), triggered only by the first real source-level operation
capable of violating memory/type invariants unless explicitly authorized.

The future direction is authorization local to small audited implementation
islands, without automatically contaminating callers. Safe wrapper interfaces
should remain callable from ordinary safe Tempra and preserve safety for their
admitted inputs. This does not define unsafe-to-call rules or new primitives.

Unsafe is conceptually separate from capability denial:

- `@unsafe` does not waive a `@deny(...)` contract;
- `@pure` does not authorize unsafe operations;
- `unsafe` is not a public argument accepted by `@deny(...)`;
- no current E1 instruction or extern origin produces `Unsafe`.

## Internal capability vocabulary

The initial internal vocabulary is:

- `Alloc`
- `Panic`
- `IO`
- `Process`
- `Spawn`
- `Nondeterminism`
- `Unsafe`

The public denial spellings are `alloc`, `panic`, `io`, `process`, `spawn`, and `nondeterminism`. `Unsafe` remains internal/reserved; the separate `@unsafe` boundary is conditional future work, not current public syntax.

The vocabulary is intentionally broader than current TIR coverage. Phase E must record unmapped operations as coverage gaps rather than invent semantics.

## Analysis point

The current default runtime evaluates checked HIR directly, rejecting annotated
execution until it supports contract enforcement. Fresh experimental VM compilation
lowers checked HIR to TIR, analyzes verified TIR, enforces contracts, then produces bytecode:

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
Contract enforcement (E3.2)
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

1. **E1 (implemented):** internal capability model and pure TIR analysis;
2. **E2 (implemented):** post-verification integration in fresh VM compilation, retaining immutable TIR, capability facts and bytecode in `VmCompilation`;
3. **E3 (implemented):** typed `@deny(...)` acquisition and declaration provenance (E3.1), then enforcement, retained witnesses and execution/cache non-bypass (E3.2);
4. **E4 (implemented):** `@pure` syntax/provenance and driver-only normalization through existing E3 enforcement;

**E6 (NEXT ACTIVE):** structured capability diagnostics and an inspect/query
surface, reusing `CapabilityAnalysis`, `FunctionCapabilityContract`,
`CapabilityContractViolation`, `CapabilityWitness`, function IDs and existing
provenance for human CLI, IDE/LSP, tools and agents. No new analysis or CLI schema
is specified here. The complete future TIR Graph is a later tooling milestone.

The semantic order is E → R → M → C → V → O. R adds future Result/value-use
obligations, not capabilities. U remains conditional. The
[post-E4 state and audit record](../E1_CAPABILITY_ANALYSIS.md) tracks native
signature integrity, comptime extern handling and JIT API correctness debts;
none is a justification for blanket `extern == unsafe`.

E1 remains a pure `tempra-effects` crate depending only on `tempra-tir`.
E2 invokes it once after verification in fresh source and multi-module VM
compilations. E3.2 combines its summaries with E3.1 contracts in the driver.
`CapabilityContractViolations` retains every violation in ascending numeric
function ID and stable capability order, with the unchanged E1 witness and
declaration provenance. Missing functions, summaries or witnesses produce typed
compiler invariant errors, never synthetic success. Final diagnostic rendering
and capability CLI output remain E6 work.

The bytecode cache stores no facts: trusted hits verify and execute bytecode
without analysis. TIR emission remains analyzer-independent; HIR and source TIR
execution reject any contract before execution. REPL rejection precedes state
registration/commit. All use the single structural HIR contract-presence scan.

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

### E3.2 cache non-bypass

E3.2 reuses the existing compiler fingerprint as
`tempra-{CARGO_PKG_VERSION}-semantics-1`. `semantics-N` versions the guarantees
required for artifact reuse, independently of bytecode serialization format.
Pre-enforcement fingerprints do not match and cause fresh compilation. Cache
writes occur only after enforcement and bytecode verification succeed; a failure
does not replace an older artifact or populate a new one. Post-E3.2 hits require
matching source identity and fingerprint plus bytecode verification, without
reconstructing TIR or serializing contracts/facts. See [cache policy](../BYTECODE_CACHE.md).

E4 retains `semantics-1`: the existing enforcement guarantee is reused, and a
pre-E4 compiler rejected the new attribute syntax. Adding/removing/changing pure
changes source bytes and therefore cache identity. No pure metadata is serialized.
