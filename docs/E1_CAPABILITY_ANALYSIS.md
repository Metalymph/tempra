# E1 Capability Analysis

> [!NOTE]
> **Status: E1–E4 implemented.** E1 provides pure analysis; E2 retains facts; E3 acquires/enforces `@deny` and prevents bypass; E4 adds `@pure` source intent and normalization through the same contracts.

## Goal and boundary

E1 adds one pure middle-end transformation:

```text
TirModule
  → direct capability scan
  → call graph
  → reverse internal edges
  → monotonic fixed-point propagation
  → CapabilityAnalysis
```

E1 does not modify the parser, AST, HIR, typechecker, driver, diagnostics, CLI, bytecode, VM, or public language semantics. It does not implement `@deny`, `@pure`, `@unsafe`, concurrency, acyclic ownership, ARC optimization, or `@verified`.

## Crate and dependency boundary

`crates/tempra-effects` is a workspace member. Its only workspace dependency is `tempra-tir`; E1 introduces no third-party dependency.

The crate owns:

- internal capability identifiers and sets;
- per-function direct and transitive summaries;
- direct TIR operation classification;
- classification of called compiler-known externs;
- call graph construction and propagation;
- deterministic structural provenance;
- structured analysis errors.

The crate does not mutate `TirModule` or attach data to bytecode. A caller must pass TIR that has already succeeded in `tempra_tir::verifier::verify_module`. E1 still returns structural errors for missing call targets rather than panicking on malformed input.

## Public Rust interface

The E1 crate exposes these responsibilities with documented Rust APIs:

- `Capability`: closed enum ordered as `Alloc`, `Panic`, `Io`, `Process`, `Spawn`, `Nondeterminism`, `Unsafe`;
- `CapabilitySet`: compact private-bit representation with `empty`, `singleton`, `contains`, `insert`, union, and iteration in `Capability` order;
- `TirLocation`: function ID, block ID, and zero-based instruction index;
- `CallEdge`: caller ID, callee ID, and callsite location;
- `DirectCapabilityOrigin` and `DirectCapabilitySource`: capability, TIR location, and whether the source is an instruction or a named extern call;
- `FunctionCapabilitySummary`: function ID plus direct and transitively required sets;
- `CapabilityAnalysis`: summaries for every defined TIR function, all observed call edges, direct origins, and deterministic witness lookup;
- `CapabilityAnalysisError`: duplicate function IDs, per-function duplicate block IDs, unknown call targets, unresolved declarations, and called unclassified externs, with structural locations where applicable;
- `analyze(&TirModule) -> Result<CapabilityAnalysis, CapabilityAnalysisError>`.

External declarations do not receive a misleading empty semantic summary. Their capabilities are classified only when a call instruction targets them. The external call becomes a direct origin on the calling function. Therefore an unused unknown extern is harmless, while an unknown extern called by any defined TIR function is an error.

`CapabilityAnalysis` must support:

- lookup of a defined function's summary by `TirFunctionId`;
- deterministic iteration by numeric function ID;
- lookup of direct origins for a function and capability;
- reconstruction of one canonical witness for a transitive requirement.

No serialization format is fixed in E1. The structures must remain typed so E6 can project them to JSON without parsing strings.

## Direct capability discovery

The classifier matches TIR instructions and fully qualified extern names. It must be exhaustive over `TirInstr`: instructions with no direct capability return an empty set explicitly rather than falling through a wildcard.

### Initial may-panic mapping

E1 classifies these operations conservatively as `Panic`:

- `Div`;
- `Unwrap`;
- `GetIndex`;
- `Slice`;
- `Get` with `TirKey::Index`.

`Set` with an index is not emitted by the current bytecode lowerer and remains a TIR coverage gap rather than receiving invented runtime semantics.

This is may-panic data, not a proof system. E1 does not attempt constant evaluation, bounds proofs, non-zero proofs, or control-flow refinement. Phase V may later discharge a particular failure obligation without changing E1's conservative baseline.

### Initial allocation mapping

`Alloc` means that the current semantic runtime representation may require allocation during execution. It is not defined by the spelling of a TIR instruction.

Current VM inspection establishes the following initial E1 mappings:

- `TirConst::String`: `Alloc`, because `ConstString` clones the stored `String` into a runtime `VmValue`;
- `Make` for arrays, tuples, enums, `Option`, and `Result`: `Alloc`, because the current VM materializes owned vectors and, for arrays, an `Rc` allocation;
- `MakeStruct`: `Alloc`, because the current VM materializes an owned field vector and names;
- `Bytes`: `Alloc`, because the current VM builds a new byte vector and `Rc` backing;
- `ArrayPush`: `Alloc`, because the current VM clones and extends an array into new `Rc` storage.

These are implementation mappings, not permanent language rules. Each must have a focused test or code comment pointing to the current lowering/runtime behavior. If runtime representation changes before E1 lands, update the table and tests rather than preserving a stale classification.

Allocation caused only by incidental cloning inside reads, unwraps, tags, or diagnostic/error construction remains unresolved. Do not silently classify those operations as allocation-free in a public contract. E2 retains this E1 coverage limitation without changing the mapping.

### Known extern mapping

The initial table is keyed by fully qualified Tempra name:

| Extern | Direct capabilities |
| --- | --- |
| `std.fs.exists` | `IO` |
| `std.fs.read_text` | `IO`, `Alloc` |
| `std.fs.write_text` | `IO`, `Alloc` |
| `std.io.print` | `IO` |
| `std.io.println` | `IO` |
| `std.path.join` | `Alloc` |
| `std.path.basename` | `Alloc` |
| `std.strings.contains` | none |
| `std.strings.starts_with` | none |
| `std.strings.ends_with` | none |
| `std.strings.trim` | `Alloc` |
| `std.strings.replace` | `Alloc` |

The mapping reflects the current `tempra-native` registry and implementations. Filesystem observation is classified as `IO`; E1 does not additionally infer `Nondeterminism`. Process commands, input, environment, OS state, time, random behavior, spawn, and unsafe operations lack clean current TIR mappings and remain explicit gaps.

Native argument/type mismatch errors and host panics are not given new capability semantics in E1 or E2. They remain future native-boundary contract issues rather than expanding the public vocabulary.

## Call graph and propagation

Analysis order is deterministic:

1. Index functions by numeric `TirFunctionId`; reject a call target missing from the module.
2. Visit defined functions sorted by function ID, then blocks by block ID, then instructions by vector index.
3. Record direct instruction origins.
4. For internal calls, record a `CallEdge`.
5. For extern calls, resolve the table at that callsite and record the returned capabilities as direct extern origins on the caller. Return `UnclassifiedExtern` only here if no table entry exists.
6. Initialize `required` with each function's direct set.
7. Use a deterministic worklist over reverse internal call edges, unioning callee requirements into callers until no set changes.

The lattice contains only seven bits, so this terminates for recursion and arbitrary strongly connected components without a separate recursive-function policy.

## Provenance and canonical witnesses

E1 retains all direct origins and all internal call edges. It need not eagerly store a path for every propagated bit.

For `witness(function, capability)`, perform a deterministic breadth-first traversal from the queried function through callees that require the capability. Return the shortest path ending at a direct origin. Break ties by:

1. callee function ID;
2. callsite block ID;
3. callsite instruction index;
4. direct-origin block ID and instruction index.

The witness is structured as call steps followed by one direct instruction or extern origin. E2 retains this structural provenance, not source spans; future diagnostics can bridge it to source and E6 can render terminal or JSON output.

## Tests and acceptance criteria

Use hand-built `TirModule` fixtures. Parser, HIR, driver, filesystem, CLI, and native execution are outside E1 tests.

Required tests:

- every `CapabilitySet` operation and stable iteration order;
- empty direct and required sets for a capability-free defined function;
- each initial instruction mapping, including string allocation as a current-runtime mapping;
- every known extern table entry;
- an unused unknown extern declaration succeeds;
- a called unknown extern fails at the precise callsite;
- the same failure occurs when only a non-entry function calls it;
- direct capability collection retains multiple origins;
- `A → B → C` propagates C's requirements to B and A;
- independent capabilities union across multiple callees;
- direct recursion and multi-function SCCs reach a fixed point;
- a non-entry defined function receives a summary;
- missing internal target returns a structured error;
- witness lookup chooses the shortest path and applies every tie-break deterministically;
- repeated analysis of the same module yields identically ordered summaries, edges, origins, and witnesses.

E1 is complete when the new crate passes its unit tests, `cargo test --workspace`, formatting, and workspace clippy with no new warnings. Pre-existing workspace clippy failures must be reported separately rather than fixed opportunistically. The implementation meets this boundary; the workspace clippy baseline remains tracked separately.

## E2 integration (implemented)

The prerequisite correctness gate repairs expected-type lowering of `Ok`/`Err`
(including nested payloads), uses the actual operand `Result` type for `?`,
and reconstructs the enclosing function's `Result` on its error branch.
`Tag` produces `String`, matching bytecode and runtime. Comparison, store and
return checks remain strict; no fake `Result(Unit, Unit)` or verifier exception.

The driver owns private `AnalyzedTir` and public immutable `VmCompilation`:

```text
fresh source or module graph → TIR → verify_module → analyze (once)
  → AnalyzedTir + completed bindings → enforce (E3.2)
  → bytecode lowering → bytecode verification → VmCompilation
```

`compile_to_bytecode_with_analysis(source, cache_path)` always compiles fresh
and writes the existing bytecode-only cache artifact. The legacy
`compile_to_bytecode` projects `into_bytecode()` from that result.
`compile_file_to_bytecode_with_analysis(path)` reuses module-graph lowering
without introducing a cache. Accessors `tir()`, `capabilities()`, `bytecode()`
and `into_bytecode()` expose the result without mutation or recomputation.

`DriverError::TirVerificationError` retains verifier errors;
`DriverError::CapabilityAnalysisError` retains the structured E1 failure.
Capability presence alone is accepted; E3.2 rejects intersecting deny contracts.
A called unclassified extern fails
fresh capability-aware compilation, including in non-entry functions; an unused
unknown extern does not fail it.

Cache-hit execution verifies cached bytecode and executes it without rebuilding
TIR or synthesizing capability facts. It is not an E2 analysis product.
`emit-tir` verifies then formats. Raw TIR interpretation verifies then interprets.
Neither invokes capability analysis. E3.2 rejects annotated source before HIR or
source TIR execution. `check` remains frontend/typechecking-only, not contract
certification. HIR-only operations remain uncovered, not analyzed in parallel.

### Provenance decision B

TIR has no reliable source mapping. E2 retains function ID, function-scoped
block ID, instruction index, edges, origins and deterministic witnesses.
Debug names are not source maps. A future lowering-owned side table should map
`TirLocation → { module identity, existing Span }`; file-relative offsets need
module identity. No instruction spans are added in E2.

E1's representation-dependent `Alloc`, conservative may-panic `Panic`, limited
extern registry and unresolved native/cloning semantics remain unchanged.

## E3.1 contract acquisition (implemented)

The lexer adds only `@`. The parser accepts dedicated `@deny(...)` syntax,
not a generic attribute framework. Its six-name `DenyCapability` enum and
`DenyContract` in AST are syntax-only; HIR copies this data without semantic
capability logic or a dependency on the analyzer. The driver owns the sole
exhaustive conversion to canonical E1 `CapabilitySet`.

Each attribute must contain at least one public capability. Empty lists,
`unsafe`, unknown names/attributes and non-identifier arguments fail parsing.
Duplicates normalize; repeated attributes union while preserving every span.
Only named top-level functions and `extend` methods accept attributes, placed
before optional `pub`. Externs, closures, interface requirements and other
items do not. No capability attribute means no synthetic empty user contract.

At each successful lowering of a concrete declaration, the driver registers
the returned `TirFunctionId` together with that exact HIR function and its source
identity. Methods use their original declaration provenance; specializations
inherit their template's contract while retaining separate concrete IDs.
Specialization requests are selected by qualified declaration, not the final
name segment across modules. A temporary registry also tracks unannotated
definitions: an ambiguous overwrite involving a contract returns structured
`ContractBindingError`; repeated identical bindings are idempotent.

Private `AnalyzedTir` retains the sorted side table. `VmCompilation::contract(id)`
and `contracts()` expose immutable `FunctionCapabilityContract` records:
`denied`, `declaration_span`, `attribute_spans`, `source`. Source is the single
`InMemory` input or the existing module graph's name and path. Spans remain
byte offsets. IDs are local to the retained compilation, not persistent cache
identifiers. No spans, source maps or contracts are added to `TirModule`.

E3.1 originally tested `deny(Io)` and required `Io` coexisting successfully.
E3.2 updates that regression to require a typed violation retaining provenance.
Int and String specializations are separately lowered,
bound, verified and executed from one source declaration; source callback
specialization and cross-module isolation also have integration coverage.

### Retained coverage limits

Only functions emitted by the existing TIR paths receive bindings. The source
VM path supports `extend` lowering; multi-module, emission and interpreter loops
are not broadened. Generic templates without emitted specializations have no
synthetic TIR contract. General direct generic calls and cross-module callback
resolution retain their existing limitations.

During tests, unqualified local calls in file compilation failed with
`unknown function`. A mixed Int/String specialization fixture, also reproduced
without attributes, intermittently failed bytecode verification with
`StackTypeMismatch { expected: String, found: Int64 }` (13/20 fresh probes).
The existing bytecode lowerer indexes its function vector directly with TIR IDs,
while specialization emission follows a hash-set order. This is not a general
String restriction. These existing lowering issues are not repaired or relaxed
by E3.1. Int/String binding is tested directly through the existing specialization
lowerer; the integration tests use the established single-callback path.

## E3.2 enforcement and execution boundaries (implemented)

The driver checks `contract.denied ∩ summary.required` after verified TIR analysis
and before bytecode lowering. It uses exact emitted function IDs, including
non-entry functions and specializations. There is no additional analysis or
call-graph construction. `CapabilityContractViolations` aggregates all violations
in numeric function ID then stable capability order. Each record retains the
function, capability, unchanged E1 witness and full E3.1 declaration contract.
`MissingFunction`, `MissingSummary` and `MissingWitness` are typed invariant
failures. Generic `Display` is not the final E6 diagnostic renderer.

`Alloc` remains representation-dependent and `Panic` remains may-panic: even
`10 / 2` can violate `@deny(panic)`. E3.2 is not a verified proof layer.

HIR execution, `test_source`, REPL and `interpret_tir_source` explicitly reject
any contract before execution, including unused functions, templates and methods.
The sole presence scan is `HirFile::contains_capability_contracts`; shared
interpreter policy adapts this to an unsupported-enforcement error. REPL checks
before type/runtime registration or state commit. No HIR analyzer or backend
auto-routing is added. `emit-tir` and frontend-only `check` remain non-enforcing;
raw TIR interpretation cannot recover contracts absent from its input.

### Cache strategy A: existing compiler fingerprint

E3.2 uses `tempra-{CARGO_PKG_VERSION}-semantics-1` in the existing header.
`semantics-N` versions the guarantees required for reuse, not the bytecode format.
Both ordinary and profiled VM execution use the same validity/load-or-compile
path. Legacy fingerprints miss. Only successfully enforced, bytecode-verified
fresh compilations write cache artifacts. Failure leaves an older artifact
unchanged and incompatible; no deletion is required. Valid hits retain no TIR,
capability facts, contracts or witness metadata. Source/contract edits are
checked by the existing source identity mechanism. This is a local compiler cache,
not an authenticated artifact format; see [cache policy](BYTECODE_CACHE.md).

Regressions cover direct/transitive violations, exact witnesses, deterministic
aggregation, non-entry functions, specialization IDs, invariant failures,
legacy cache rejection, absence of writes after failure, valid bytecode-only
hits, source/contract changes and transactional rejection by other execution paths.

## E4 pure normalization (implemented)

`@pure == @deny(io, process, spawn, nondeterminism)`. Pure does not deny Alloc,
Panic or internal Unsafe, does not grant unsafe permissions, and may return Result.
The only canonical expansion lives in the driver's contract conversion module.
E1 analysis/propagation and E3.2's `denied ∩ required` enforcement are unchanged.

`FunctionItem` and `HirFunction` retain `pure_span: Option<Span>` alongside the
unchanged explicit `DenyContract`. The marker spans `@` through `pure`; it is not
expanded into synthetic source attributes. `@pure` takes no parentheses or
arguments and shares deny's placement. A second pure annotation is a parse error
at that marker. Both orders with explicit deny are valid; overlapping capabilities
normalize by union. Ordinary identifiers named `pure` remain valid.

The driver creates a real `FunctionCapabilityContract` for either attribute.
Its `denied` set is effective, `attribute_spans` contains explicit deny spans only,
and `pure_span` separately preserves pure intent for future E6 consumers. Both
markers absent still means no contract. Existing ID binding, specialization
inheritance, immutable accessors and structured violation provenance are reused.
The enforcer does not know which source spelling contributed a capability.

`HirFile::contains_capability_contracts` now checks pure presence too; existing
HIR, test, source-TIR and REPL guards require no second scanner or semantic pass.
REPL rejection occurs before state registration/commit. `check` remains frontend
correctness only, and `emit-tir` remains non-enforcing. Raw TIR interpretation
has no source-contract metadata. E1's incomplete Process/Spawn/Nondeterminism
coverage and known lowering limitations remain unchanged.

Cache code and the `semantics-1` fingerprint are unchanged: E3 already required
enforcement, pre-E4 parsing rejected `@pure`, and source hashing distinguishes
attribute edits. Tests cover real temporary-cache population/hits, source changes
and failed pure-plus-deny compilation leaving no new/replacement artifact.
No pure metadata, capability facts, contracts, spans or witnesses are serialized.

Regressions cover parser placement/duplicates, canonical expansion and composition,
frontend-to-driver provenance, method/specialization bindings, direct/transitive
Io witnesses, non-entry enforcement, allowed Alloc/Panic/Result, explicit additional
restrictions, deterministic aggregation and transactional execution rejection.

## Post-E4 state and next tooling milestone

E1–E4 complete the currently meaningful light capability/effect surface, not all
TIR coverage. **E6 is NEXT ACTIVE:** structured capability diagnostics and an
inspect/query surface for human CLI, IDE/LSP, tools and agents. It reuses
`CapabilityAnalysis`, `FunctionCapabilityContract`, `CapabilityContractViolation`,
`CapabilityWitness`, `TirFunctionId`, and current structural/source provenance.
This is not a new analyzer or a commitment to a CLI/JSON schema.

The richer TIR Graph is later tooling: calls, CFG, capabilities, allocations,
ownership edges, escapes, actor boundaries, specializations and source provenance
may eventually be combined, but some facts require M/C and later compiler work.

Instruction-level source-span bridging, broader TIR coverage and native-boundary
policy remain deferred; E3.1 adds declaration provenance only.

The semantic roadmap is **E → R → M → C → V → O**. R is Result/value-use
hardening, not an effect: future rules should prevent silent discard, including
an accidental `let _ = ...` escape hatch, while allowing `?`, `match`, return/
propagation and meaningful use. This does not implement wildcard bindings or
commit an opt-out such as future `@discardable`. M then defines acyclic ownership;
pluggable actor concurrency C, verified functions V and memory optimizations O follow.

## Historical E5 audit and conditional U

The audit found no intentional source-level primitive requiring unsafe authority.
`@unsafe` is not implemented and no E1 instruction or extern origin produces
`Capability::Unsafe`. The reserved bit and generic propagation/witness machinery
remain available. Unsafe is removed from the immediate E sequence and retained
as **U — Unsafe Boundary (DEFERRED / CONDITIONAL)** at the end of the roadmap.

U activates only for the first real source-level operation that can violate
memory/type invariants unless explicitly authorized. The future direction is
small locally authorized, audited implementation islands with safe wrapper
interfaces, not automatic caller contamination. Such interfaces must preserve
safety for their admitted inputs; no unsafe-to-call syntax is settled here.
Authorization must not disable deny/pure enforcement, and `@deny(unsafe)` remains
invalid. Native implementation, IO, panic and allocation do not imply unsafe.

### Correctness debts recorded by the audit

These are separate narrow remediation items, not new roadmap phases or operations
to authorize through `@unsafe`:

- **Native/extern signature integrity.** The native registry resolves names to
  `fn(&[VmValue]) -> Result<VmValue, String>` without canonical signature metadata.
  Frontend and IR checks trust declared signatures; VM dispatch does not validate
  the returned value against the declared result. The audit reproduced a verified
  TIR call to `std.strings.contains` declared as Int returning Bool. Handler
  argument checks do not close this type-preservation gap. See
  [registry](../crates/tempra-native/src/registry.rs) and
  [VM dispatch](../crates/tempra-vm/src/interpreter.rs).
- **Comptime extern handling.** The HIR evaluator has no native registry dispatch;
  a bodyless function can yield Unit rather than fail explicitly. The audit also
  observed incorrect comptime materialization of that result. Unsupported extern
  execution needs explicit rejection, not unsafe authorization. See
  [HIR function evaluation](../crates/tempra-interpreter/src/interpreter.rs) and
  [comptime evaluation](../crates/tempra-typeck/src/check_expr.rs).
- **Internal JIT API audit.** Raw code/context pointers, ABI conversions and
  lifetime/aliasing assumptions in public Rust wrappers require runtime/embedding
  review. A null check is not a proof of pointer validity. These are not source
  primitives; the audit did not demonstrate memory corruption or an exploit.
  See [JIT wrappers](../crates/tempra-jit-cranelift/src/engine.rs) and
  [context helpers](../crates/tempra-jit-cranelift/src/helpers.rs).
