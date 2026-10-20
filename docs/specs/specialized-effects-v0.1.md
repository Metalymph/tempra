# Tempra Specialized Effects v0.1

> [!NOTE]
> **Status: K4.2–K4.7 implemented.**
>
> K4.6A (boundary audit), K4.6B.1 (native registry/classifier/signature
> integrity), K4.6B.2 (compilation-boundary regressions), K4.6C
> (guarantee documentation), K4.7A (differential coverage audit), K4.7B
> (focused differential tests) and K4.7C (final parity gate) are complete.
> The K4.7 verdict is **PASS** for the supported same-build, same-host/target
> paths, with the unsupported and deferred paths listed in section 10.
>
> K4.8 adds a **specification-only** normative extension for families,
> parameterless operation denials, cross-family categories and the missing
> network taxonomy in
> [Specialized Effects v0.2](specialized-effects-v0.2.md). It is not
> implemented. This document remains the description of current behavior and
> `semantics-6`; K4.10 owns the final coverage ledger after the K4.9
> implementation sequence.
>
> This document records the guarantees that the current compiler actually
> provides, the trust assumptions behind them and the limits that must not be
> overstated. It is no longer a proposal for future syntax. Syntax shown here
> is implemented. A taxonomy operation without a production binding remains a
> coverage gap, not an enforceable operation.

## 1. Purpose, scope and guarantee summary

The existing capability surface remains:

- public contracts: `@deny(...)` and `@pure`;
- internal vocabulary: `Alloc`, `Panic`, `Io`, `Process`, `Spawn`,
  `Nondeterminism`, `Unsafe`;
- analysis: one per-function summary over verified TIR with transitive
  propagation and deterministic witnesses.

`@deny(io)` is all-or-nothing. Specialized effects add a **compile-time
refinement** over the existing broad capability bits. They do not add public
permissions, runtime authority or a second capability system.

The implemented K4.6 guarantee is:

> If a supported fresh VM compilation succeeds, then no statically reachable
> known specialized operation matched by the function's specialized `@deny`
> clauses remains unenforced. A definite or potential match is a compilation
> error. Broad `@deny`/`@pure` enforcement remains authoritative.

The guarantee is deliberately bounded:

- it applies to operations represented in verified TIR and classified by the
  compiler;
- it applies before bytecode lowering, so rejected programs do not execute;
- it is a source-contract check, not an operating-system permission or sandbox;
- it does not add runtime guards;
- it trusts the production native registry/resolver and the local bytecode
  cache model described below.

## 2. Architectural invariants and non-goals

1. **`CapabilitySet(u8)` and the seven capabilities are unchanged.**
   Specialized effects consume no capability bit and add no enum variant.
   Specialized facts are a compilation-local side product keyed by TIR
   identities.
2. **Broad `@deny` and `@pure` semantics are unchanged.** Broad capability
   denial remains authoritative. Specialized clauses only add restrictions;
   they never relax, replace or discharge a broad denial.
3. **Specialized effects refine one parent capability.** Every operation has
   exactly one parent (`Io`, `Process` or `Spawn`). No specialized clause grants
   authority.
4. **No general effect system.** There is no `effect`, `handle`, `perform`,
   `allow`, effect row, effect polymorphism, ambient authority or user-defined
   operation.
5. **`Unsafe` remains internal and separate.** `@unsafe` is deferred and
   specialized effects never authorize unsafe operations.
6. **One analysis, one call graph.** Specialized facts reuse the existing E1
   call graph, summaries, fixed point and witness machinery. There is no second
   analyzer and no second call graph.
7. **Fail closed.** Missing classification, missing resource knowledge or an
   unknown operation under a denied parent is never treated as permission.
8. **No runtime enforcement is claimed.** Dynamic resources are decided
   statically or fail closed at compile time. There is no runtime guard that
   re-checks paths, hosts, ports or streams.
9. **No filesystem or network sandbox is claimed.** Filesystem and network
   parameters are opaque source spellings. Symlinks, hard links, mount aliases,
   DNS results and TOCTOU races remain outside the guarantee.

Explicit non-goals are allocation budgets, panic categories, a proof of
termination, a general policy language, filesystem confinement, network
isolation, native-code sandboxing and unsafe authorization.

## 3. A. Taxonomy and current production coverage

The taxonomy is closed in v0.1. Operation declaration order is canonical.

| Operation | Parent | Current production state |
|---|---|---|
| `fs.read` | `Io` | Enforceable through classified `std.fs.read_text`. |
| `fs.write` | `Io` | Enforceable through classified `std.fs.write_text`. |
| `fs.remove` | `Io` | Syntax accepted; no production native binding in v0.1. Coverage gap. |
| `net.connect` | `Io` | Syntax accepted; no production native binding in v0.1. Coverage gap. |
| `net.listen` | `Io` | Syntax accepted; no production native binding in v0.1. Coverage gap. |
| `io.read` | `Io` | Syntax accepted; no production native binding in v0.1. Coverage gap. |
| `io.write` | `Io` | Enforceable through classified `std.io.print` and `std.io.println`; stream is currently unknown. |
| `process.exec` | `Process` | Syntax accepted; no production native binding in v0.1. Coverage gap. |
| `process.exit` | `Process` | HIR-only builtin with no stable TIR mapping. Coverage gap. |
| `spawn.task` | `Spawn` | Syntax accepted; runtime and scope semantics are deferred. Coverage gap. |
| `spawn.actor` | `Spawn` | Syntax accepted; runtime and scope semantics are deferred. Coverage gap. |

An accepted clause for an operation without a reachable production binding does
not create authorization or enforcement. It also does not make the operation
safe; it records a restriction that currently has nothing to match. K4.8 is the
specification-only family/operation extension in
[Specialized Effects v0.2](specialized-effects-v0.2.md); K4.10 owns the explicit
coverage ledger after implementation.

`std.fs.exists` is deliberately classified as broad `Io` with **no specialized
operation**. A call therefore produces an `UnclassifiedOperation(Io)` fact.
Any specialized denial whose parent is `Io` treats that fact as a potential
match and fails closed. `std.path.*` and `std.strings.*` have no specialized
parent requirement and therefore produce no specialized facts.

### 3.1 `alloc` and `panic` refinements are deferred

`alloc` and `panic` are not part of the specialized taxonomy. They remain broad
capabilities with conservative semantics. In particular, `Alloc` is
representation-dependent and `Panic` is may-panic information; even `10 / 2`
can require `Panic`.

No v0.1 guarantee covers allocation budgets, `@no_alloc`, named panic
categories, `@no_panic`, totality or termination.

### 3.2 `spawn.*` scope is deferred

`spawn.task(scope: "workers")` and `spawn.actor(scope: "workers")` are accepted
as closed-taxonomy syntax, but the scope has no runtime semantics and the
operations have no production binding. The parameter cannot be treated as an
enforced concurrency boundary before milestone C defines the runtime and
ownership model.

## 4. B. Implemented syntax and typed parameters

### 4.1 Clause grammar

Specialized clauses appear inside the existing `@deny(...)` argument list:

```text
clause       := operation [ "(" parameter ("," parameter)* [","] ")" ]
parameter    := name ":" value
operation    := "fs.read" | "fs.write" | "fs.remove"
              | "net.connect" | "net.listen"
              | "io.read" | "io.write"
              | "process.exec" | "process.exit"
              | "spawn.task" | "spawn.actor"
name         := "source" | "destination" | "host" | "port"
              | "protocol" | "executable" | "stream" | "scope"
value        := "*" | string-literal | integer-literal
```

Implemented restrictions:

- operation names must contain exactly one `.` and match the closed taxonomy;
- only parameters meaningful for the operation are accepted;
- a duplicate parameter in one clause is a parse error;
- `port` accepts an integer in `0..=65535`;
- every other value except `*` is a quoted string literal, including
  `protocol: "tcp"` and `stream: "stdout"`;
- expressions, variables, function calls, regexes, globs, prefixes and ranges
  are not accepted;
- `*` is the complete value token, not a substring or pattern;
- a zero-parameter operation may be written `process.exit` or `process.exit()`.

There is no `=` form. The colon form is the implemented grammar.

### 4.2 Parameters by operation

| Operation | Meaningful parameters |
|---|---|
| `fs.read` | `source` |
| `fs.write` | `destination` |
| `fs.remove` | `destination` |
| `net.connect` | `host`, `port`, `protocol` |
| `net.listen` | `host`, `port`, `protocol` |
| `io.read` | `stream` |
| `io.write` | `stream` |
| `process.exec` | `executable` |
| `process.exit` | none |
| `spawn.task` | `scope` |
| `spawn.actor` | `scope` |

Text parameters (`source`, `destination`, `host`, `protocol`, `executable`,
`stream`, `scope`) are opaque source spellings. They are not canonical paths,
resolved host identities or OS resource handles. `port` is an exact integer.

### 4.3 Composition and canonicalization

- Repeated broad capabilities deduplicate and repeated `@deny` attributes
  union exactly as before.
- Repeated specialized clauses union. Identical clauses deduplicate while the
  first source occurrence and its spans are retained.
- Clause parameters are stored in canonical parameter order:
  `source`, `destination`, `host`, `port`, `protocol`, `executable`, `stream`,
  `scope`.
- Clauses are stored in taxonomy-operation order, then canonical parameter
  order.
- Broad denial and specialized denial are retained separately.
  `specialized_denied` never contributes to `FunctionCapabilityContract.denied`.
- `@pure` plus a specialized clause is valid. `@pure` already denies the parent
  capability broadly; the specialized clause is redundant but does not change
  pure semantics.

### 4.4 Implemented syntax examples

```tempra
// Exact filesystem destination.
@deny(fs.write(destination: "build/out.txt"))
fn emit_report() -> Int64 { return 0 }

// Any outbound TCP connection, if a production net.connect binding exists.
@deny(net.connect(protocol: "tcp", port: *))
fn connect_anywhere() -> Int64 { return 0 }

// Exact host and port.
@deny(net.connect(host: "db.internal", port: 5432))
fn connect_db() -> Int64 { return 0 }

// Stdout only, if the stream can be identified.
@deny(io.write(stream: "stdout"))
fn quiet() -> Int64 { return 0 }

// A specific executable.
@deny(process.exec(executable: "/usr/bin/rm"))
fn guard_exec() -> Int64 { return 0 }
```

The `net.*`, `io.write(stream: "stdout")` and `process.exec` examples currently
have no reachable production binding or, for streams, no statically known
stream value. They document syntax and intent, not current enforcement coverage.

## 5. C. Contract semantics, facts and matching

### 5.1 Broad versus specialized denial

- Broad denial `@deny(io)` denies every `Io` operation, including all `fs.*`,
  `net.*` and `io.*` siblings.
- A specialized denial restricts one identified operation and its matching
  resource parameters.
- Broad denial subsumes specialized denial of the same parent. If both broad
  and specialized violations are present, enforcement returns the broad
  `CapabilityContractViolations` error first. A specialized
  `SpecializedContractViolations` error is returned only when broad enforcement
  passes.
- Specialized denial never implies broad denial.

### 5.2 Direct and transitive facts

- Direct facts originate at a call to a compiler-known extern with an explicit
  specialized operation binding.
- Transitive facts reuse the existing E1 internal call graph and fixed point.
- A caller receives the **same origin fact** as the callee: operation identity,
  resource knowledge, original TIR location and extern identity are preserved.
  Current implementation does not degrade an exactly known resource merely
  because the requirement crossed a call boundary. Resource values do not flow
  through function parameters; if the origin call has a dynamic value, the fact
  is unknown at the origin.
- A canonical witness is the shortest deterministic internal call path to the
  original fact. The fact's location is the origin and is never rewritten to a
  caller callsite.
- A known `fs.write` is never collapsed into generic `Io`. Operation identity
  and resource knowledge remain separate.

### 5.3 Exact values, unknown values and wildcards

The two known-operation states are represented distinctly:

1. **Known operation, exact resource** — a dominating literal was proved at the
   callsite.
2. **Known operation, unknown resource** — the operation is identified but the
   relevant resource value could not be proved.

Exact resource extraction is deliberately narrow. A value is exact only when it
has exactly one definition in the function, that definition is a `Const` earlier
in the same block and before the call, and the constant's kind matches the
parameter. Parameters, loads, stores, calls, values from other blocks, multiple
definitions and branches all degrade to `Unknown`.

Matching rules:

| Denied parameter | Observed knowledge | Result |
|---|---|---|
| `*` | exact, unknown, missing or duplicate | definite match |
| exact | equal exact value of the correct kind | definite match |
| exact | different exact value of the correct kind | disjoint |
| exact | unknown | potential match (violation) |
| exact | missing parameter | potential match (violation) |
| exact | wrong value kind or duplicate metadata | potential match (violation) |

Multiple denied parameters are conjunctive. If any parameter is disjoint, the
whole clause is disjoint. Otherwise, any potential parameter makes the match
potential; only all-definite parameters produce a definite match. A clause with
no parameters matches every resource of the same known operation. Extra
observed parameters do not narrow the denial.

### 5.4 Different operations and unclassified operations

- Different known operations are disjoint even when they share a parent.
  `fs.read` never matches `fs.write`; `io.write` never matches `fs.write`.
- A known extern with a parent capability but no specialized operation produces
  an `UnclassifiedOperation` fact. It is a potential match for every specialized
  denial whose operation has the same parent, and disjoint from every other
  parent. For example, `std.fs.exists` is potential against any `fs.*` clause.
- An extern with no compiler-known capability classification at all is a
  compilation error when called (`UnclassifiedExtern`), before specialized
  matching. An unused unknown extern remains harmless.
- An operation that reaches TIR without a taxonomy entry is a coverage gap; it
  is never implicitly authorized.

### 5.5 Dynamic resource values

A resource computed at runtime is `Unknown`, not generic `Io`. An exact
specialized clause then produces a potential match and fails closed. A wildcard
clause still matches the identified operation.

v0.1 provides no runtime guard that could recover a precise compile-time
decision. A program that needs a broad guarantee for dynamic resources must deny
the parent capability instead of relying on a specialization.

### 5.6 Fail-closed summary

Compilation fails closed when any of the following holds. Items 1–3 produce a
specialized violation; items 4–7 fail before or independently of specialized
matching and are never treated as contract success.

1. an unclassified operation has the denied parent capability;
2. a known operation has an unknown resource and the clause restricts that
   resource;
3. resource metadata is missing, duplicated or has the wrong kind;
4. an operation is HIR-only or otherwise cannot be represented in the supported
   TIR path (the compilation is rejected, not treated as disjoint);
5. a called extern has no capability classification (analysis error
   `UnclassifiedExtern`);
6. a specialized summary or witness is missing (typed invariant error);
7. contract binding is ambiguous or missing (typed error).

For matching itself, only `Disjoint` is safe. `DefiniteMatch` and
`PotentialMatch` both reject compilation.

### 5.7 Specialization and function identity

Contracts bind to concrete `TirFunctionId`s. Names are display-only. Generic
specializations inherit the template's contract and declaration-site literals
unchanged. Resource values are not substituted by type parameters. Ambiguous
binding remains a structured `ContractBindingError`.

## 6. D. Compile-time enforcement, native trust and file/network limits

### 6.1 Enforcement point

Specialized classification runs in the same middle-end window as E1, after TIR
verification and alongside broad capability analysis, before contract
enforcement and bytecode lowering:

```text
verified TIR
  → direct broad and specialized facts
  → existing call graph and fixed point
  → broad denied ∩ required
  → specialized clause matching
  → bytecode lowering and verification only if enforcement succeeds
```

A violation therefore prevents bytecode lowering and execution. VM, profiled VM
and JIT paths consume already-enforced bytecode. A JIT-enabled option does not
move enforcement into native code.

### 6.2 There is no runtime enforcement

K4.6 adds no runtime guard, native wrapper, syscall interception or OS sandbox.
The compiler either proves a match, proves disjointness, or fails closed. If a
program passes compilation because the observed resource was disjoint, later
runtime behavior is still performed by the trusted native implementation using
the host process's operating-system permissions.

There is no guarantee that a native implementation performs only the operation
named by its classification. That is a native trust issue, not a runtime
specialized-effect check.

### 6.3 Native trust model

Compiler-owned classification is explicit. It is not inferred from a native
function's name, module, arguments or return type.

The current production mapping is:

| Extern | Broad capabilities | Specialized operation | Resource binding |
|---|---|---|---|
| `std.fs.read_text` | `Io`, `Alloc` | `fs.read` | `source` = argument 0 |
| `std.fs.write_text` | `Io`, `Alloc` | `fs.write` | `destination` = argument 0 |
| `std.io.print` | `Io` | `io.write` | `stream` = unknown |
| `std.io.println` | `Io` | `io.write` | `stream` = unknown |
| `std.fs.exists` | `Io` | none | none |
| `std.path.*` | `Alloc` | none | none |
| `std.strings.*` | `Alloc` or none | none | none |

`std.fs.write_text` argument 1 is file content, not a second resource.
`std.io.print`/`println` have no stream argument, so their stream remains
`Unknown`; a wildcard `io.write` clause matches, while an exact stream clause
fails closed as a potential match.

Trust boundaries:

- **Explicit compiler-owned classification:** the classifier table in
  `tempra-effects` is the single authority for operation identity, parent
  capability and positional resource binding. Classification is never inferred
  from a native function's name or signature.
- **Production native registry:** `tempra-native` owns the closed native
  registration table and resolver. The classifier and registry are separate
  compiler-owned tables; neither is generated from the other.
- **Cross-crate consistency tests (K4.6B.1):** tests derive names from the
  production registry, require exact bidirectional agreement with the
  classifier and declared `std/*.tmpr` signatures, check parent capabilities and
  verify resource argument positions/types. They are regression coverage, not a
  runtime ABI guard.
- **Trusted resolver identity:** the VM executes the function pointer returned
  by the supplied resolver. The compiler trusts the driver's production
  resolver; it cannot authenticate the implementation behind a different
  resolver with the same module/function names.
- **No arbitrary-native sandbox:** native code is not sandboxed. The classifier
  describes known effects; it does not confine memory, filesystem, network or
  process access.
- Native handler argument validation remains manual. Static signature/registry
  tests do not add a runtime signature check.

### 6.4 Filesystem limitations

`source` and `destination` are opaque source-literal spellings compared as
written. v0.1 provides no path canonicalization and no filesystem confinement.
Specifically, it makes no guarantee about:

- relative versus absolute equivalence;
- `.`/`..`, duplicate separators or trailing separators;
- symbolic links or hard links;
- bind mounts, mount aliases or namespace aliases;
- case-insensitive or Unicode-normalizing filesystems;
- time-of-check/time-of-use races between compilation and execution;
- files reached indirectly by a helper that opens a computed path;
- a native implementation doing more than its classified operation.

A filesystem specialization is therefore a source-level spelling check. It is
not a sandbox, an ACL or an OS permission.

### 6.5 Network identity limitations

There are currently no production `net.*` natives, so no `net.*` clause is
reachable for enforcement in v0.1. If a future binding is added, `host`,
`protocol` and `port` remain opaque declared values. No guarantee would cover
DNS results, CNAME chains, multiple `A`/`AAAA` records, DNS rebinding, IP
equivalence, NAT, proxies, tunnels, TLS SNI, certificates or redirects.

`Nondeterminism` classification remains independent and follows actual
operation semantics. No `net.*` clause implies or discharges it.

### 6.6 Capability denial is not an operating-system permission

`@deny`/`@pure` are compile-time source contracts. They do not grant OS
permissions, and they are not a replacement for OS access control. A program
that passes effect enforcement runs native code under the host process's
ordinary filesystem/network/process permissions. Conversely, an OS permission
does not satisfy or weaken a Tempra contract.

## 7. E. Cache assumptions

The bytecode cache remains bytecode-only and locally trusted.

- **Compiler fingerprint:** `tempra-{CARGO_PKG_VERSION}-semantics-6`.
- **Semantic revision:** `6`. K4.5B raised revision 5 to 6 because revision-5
  artifacts could retain specialized `@deny` clauses without static enforcement.
  K4.6C does not change the revision.
- **Source hash:** a non-cryptographic source identity hint. It is not SHA-256,
  not an integrity check and not a security primitive.
- **Trusted artifact:** a compatible hit must match the fingerprint and source
  identity and pass bytecode verification. It is then trusted as the product of
  a prior successful, enforced compilation by the same build/semantics.
- **No re-analysis on hit:** cache hits do not rebuild TIR, capability facts,
  contracts or specialized facts. They execute verified bytecode only.
- **Writes after success only:** fresh compilation writes an artifact only after
  contract enforcement and bytecode verification succeed. A failed compilation
  does not create or replace an artifact.
- **Stale revisions miss:** older fingerprints are incompatible and trigger
  fresh compilation. A failure leaves an older artifact unchanged and unusable.
- **No cache signing or authentication:** headers are not authenticated against
  deliberate modification. The cache is assumed to be local and
  compiler-owned. An attacker who can rewrite cache artifacts or the compiler
  is outside this guarantee.
- **No specialized metadata is serialized.** Contracts, facts, resource
  knowledge, witnesses and source spans are not part of the cache format.
- File/module compilation and effect inspection compile fresh; they do not use
  the bytecode cache.

## 8. F. Supported and unsupported execution paths

### 8.1 Enforcing paths

| Path | Behavior |
|---|---|
| Fresh source VM compilation (`compile_to_bytecode_with_analysis`) | Verifies, analyzes, enforces broad and specialized contracts, then lowers bytecode. |
| Fresh file/module VM compilation (`compile_file_to_bytecode_with_analysis`) | Same enforcement over the module graph; no bytecode cache. |
| Ordinary VM execution | Executes only enforced compilation output. |
| Profiled VM execution | Uses the same compile/load and enforcement path. |
| JIT-enabled VM | Enforcement occurs before bytecode lowering; JIT executes already-enforced bytecode. |
| Compatible cache hit | Requires `semantics-6`, matching source identity and verified bytecode; trusts prior enforcement. |
| `tempra run --experimental-vm` | Uses the enforcing VM path. |
| `tempra effects` | Compiles fresh and enforces, but its specialized diagnostic projection is limited (section 9). |

### 8.2 Explicitly rejecting paths

The HIR executor, source TIR executor, test runner and REPL reject **any**
annotated input before execution, including unused declarations, templates and
specialized-only clauses:

- `run_source`, `run_source_with_args`, `run_source_hir`, `run_file`,
  `test_source`;
- `interpret_tir_source`;
- REPL evaluation, before type/runtime registration or state commit.

This is explicit rejection, not an alternate enforcement implementation. A valid
contract is still rejected on these paths because they cannot enforce it.

### 8.3 Non-executing tooling

- `check`/`check_file` are frontend/typechecking-only. They do not certify broad
  or specialized capability contracts.
- `emit-tir` verifies and formats TIR. It does not enforce contracts and is not
  execution or certification.
- Raw TIR interpretation verifies and interprets TIR but has no source-contract
  metadata. It is a low-level API, not a supported source execution path for
  annotated programs.
- `tempra effects` is inspection, not execution, but it does compile fresh and
  enforce contracts before producing an inspection result.

### 8.4 Runtime enforcement does not exist

No supported path performs a runtime specialized-effect check. Programs either
pass compile-time enforcement or are rejected. There is no fallback that
"tries" a specialized restriction at runtime.

## 9. Effect-inspection diagnostic limitation

E6 was not redesigned by K4.6, and its schema is unchanged.

Broad capability violations are projected into `EffectDiagnostic` and appear
with the stable `capability-denied` code in human and JSON output.

Specialized violations currently follow a narrower path:

- enforcement returns `DriverError::SpecializedContractViolations`;
- `inspect_effects_file` maps that to `EffectQueryError::Compiler`;
- `tempra effects --json` emits the existing generic compiler-error diagnostic
  shape with an empty `functions` array;
- human `tempra effects` returns the generic specialized-violation message;
- successful inspection's contract JSON does not currently expose
  `specialized_denied` clauses or specialized requirements.

The internal `SpecializedContractViolation` retains the clause, clause and
parameter spans, source contract, matched fact, `DefiniteMatch`/`PotentialMatch`
classification and canonical witness. That data is available to Rust callers
and regression tests but is **not** projected into E6.

This is the current documented limitation. K4.6C does not add a specialized
diagnostic code, a specialized E6 schema, structured witness output or JSON
fields.

## 10. G. Milestone status and specialized gates

```text
K4.1 specialized-effects specification       DONE
K4.2 taxonomy and typed parameter model       DONE
K4.3 TIR specialized classification           DONE
K4.4 propagation, summaries and witnesses     DONE
K4.5A surface syntax and provenance            DONE
K4.5B enforcement and semantics-6 cache trust  DONE
K4.6A compilation/runtime/FFI boundary audit   DONE
K4.6B.1 native registry/classifier/signature   DONE
K4.6B.2 compilation-boundary regressions       DONE
K4.6C guarantee documentation                  DONE
K4.6  specialized-effect guarantee milestone   DONE
K4.7A differential coverage audit              DONE
K4.7B focused differential tests               DONE
K4.7C final specialized parity gate            PASS
K4.7  backend parity/reproducibility gate      DONE
K4.8  family/operation semantics spec          DONE (specification only)
K4.9A taxonomy and parser/AST targets          PLANNED
K4.9B canonical contracts and driver           PLANNED
K4.9C analysis and classifier metadata         PLANNED
K4.9D family/category matcher and enforcement  PLANNED
K4.9E semantic revision and cache migration    PLANNED
K4.9  family/operation implementation          PLANNED
K4.10 final effects gate and coverage ledger   PLANNED
```

K4.6A found no contract-enforcement bypass in the audited compilation, runtime
or FFI boundaries. K4.6B.1 added native registry/classifier/signature integrity
regressions. K4.6B.2 added fresh source/file compilation, module-graph,
VM/profiling/JIT, bytecode-cache, HIR/source-TIR/REPL unsupported-path,
unknown-extern and effect-inspection boundary regressions while retaining broad
`@deny`/`@pure` parity coverage.

K4.7A audited the existing differential surface. K4.7B added focused
source/file, explicit-stdlib-root, accepted/rejected, deterministic violation,
broad-fact invariance, non-certifying inspection and VM/JIT regressions. K4.7C
adds independent-process canonical violation evidence and fixes the native-JIT
limitation in the ledger below. It does not change source, runtime, cache
format, diagnostics or `semantics-6`.

K4.8 is documentation-only. It records the proposed v0.2 family, parameterless
operation, cross-family category and network semantics in
[Specialized Effects v0.2](specialized-effects-v0.2.md). That proposal is not
implemented and does not change this document's current guarantees. The K4.9
implementation sequence must add and test those semantics; K4.10 then publishes
the final per-operation coverage ledger.

### 10.1 Coverage ledger

| Verified property | Primary evidence | State |
|---|---|---|
| Fresh source and file paths agree on broad facts, witnesses, contracts and execution. | `specialized_differential_tests::source_and_file_paths_agree_on_broad_facts_witnesses_and_contracts` | Supported |
| Equivalent file paths agree on specialized facts, exact/unknown resources, provenance and bytecode across default and explicit stdlib roots. | `specialized_differential_tests::equivalent_file_paths_agree_on_specialized_facts_resources_and_provenance` | Supported |
| Accepted, definite, potential, transitive, broad-first and `@pure` outcomes agree across equivalent file paths. | `specialized_differential_tests::equivalent_file_paths_agree_on_accepted_and_rejected_enforcement_outcomes` | Supported |
| Repeated fresh compilations preserve violation order, classification, origin, resources and witness lengths. | `specialized_differential_tests::specialized_violations_are_deterministic_across_fresh_compilations` | Supported |
| Independent processes with different cwd values produce one canonical report of denied clause, classification, fact, origin identity, resource knowledge, witness call sequence and extern identity. | `specialized_differential_tests::independent_processes_have_one_canonical_specialized_violation_report` | Supported |
| Adding specialized clauses does not change broad facts, broad witnesses or bytecode. | `specialized_differential_tests::adding_specialized_classification_does_not_change_broad_facts_or_witnesses` | Supported |
| `check_file` does not certify enforcement; inspection compiles fresh and enforces specialized contracts. | `specialized_differential_tests::non_executing_check_file_does_not_certify_specialized_enforcement` | Supported |
| A valid specialized program has equivalent VM and actually-compiled JIT results for the JIT-supported arithmetic subset. | `specialized_differential_tests::valid_specialized_program_has_equivalent_vm_and_jit_results` | Supported subset |
| JIT options do not bypass compile-time specialized enforcement; JIT consumes already-enforced bytecode. | `compilation_boundary_tests::jit_options_do_not_bypass_specialized_compile_enforcement`, `jit_compiled_programs_retain_specialized_contracts` | Supported |
| Cache miss enforces; compatible hit is verified-bytecode-only; stale revisions, source changes and failed compilation are incompatible misses. | `compilation_boundary_tests::cache_miss_enforces_and_compatible_hit_is_reused`, `stale_semantic_revision_is_rejected_before_execution`, `source_changes_select_a_new_verified_cache_identity`, `failed_compilation_does_not_replace_a_valid_specialized_artifact`; `deny_enforcement_tests::normal_and_profiled_vm_execute_a_real_enforcement_aware_cache_hit` | Supported; K4.7C added no duplicate cache test |
| HIR, source TIR, test and REPL paths reject annotated input before execution. | `compilation_boundary_tests::hir_and_source_tir_entry_points_reject_specialized_annotations`; existing source-execution adapter tests | Explicit rejection |
| Native registry, classifier and declared signatures agree on names, parent capabilities and resource positions. | `native_boundary_integrity::registry_classifier_and_declared_signatures_agree_exactly`, `specialized_bindings_follow_declared_signatures_and_native_positions`, `native_handlers_honor_verified_resource_positions` | Supported regression coverage |

Cross-process evidence compares structured, normalized fields rather than debug
formatting. Function names are module-unqualified; identity is the tuple of
name, block and instruction index; resource values are canonical text/port or
`unknown`; witness entries are caller callsite to callee. The report contains no
absolute paths and is compared across eight fresh processes run from two
different working directories.

### 10.2 Intentionally unsupported paths

These paths are not silently counted as passing parity:

- **Runtime dynamic-resource guards:** unsupported. There is no runtime
  specialized-effect check, syscall interception, filesystem sandbox or native
  confinement. Dynamic values are decided statically or fail closed.
- **Native `std.*` calls under JIT:** unsupported by the current JIT v1. JIT
  eligibility recursively requires callees to be eligible and requires an
  `Int64` return for source-level JIT functions; none of the relevant
  `std.fs`/`std.io` native externs returns `Int64` (`Bool`, `String!`, `Unit!`
  and `Unit` forms are present). Entries that call them are rejected by the JIT
  and execute through the interpreter fallback.
  `native_std_calls_are_not_jit_eligible` proves both
  `std.fs.exists` and `std.io.println` are rejected, execute correctly and make
  zero JIT runtime calls. This is a JIT implementation limitation, not a
  specialized-enforcement bypass; K4.7C does not change JIT architecture.
- **HIR, source TIR, test runner and REPL:** annotated input is rejected before
  execution, so there is no alternate enforcement backend to compare.
- **Source-only compilation of `std.*` imports:** the source-only API has no
  module graph and rejects imported standard-library names. Specialized-fact
  differential coverage therefore uses file/module paths. This is an API
  limitation, not a missing enforcement check.
- **Wasm/AOT and future backends:** no specialized-enforcement implementation is
  claimed. A future backend must reuse the same analysis or reject annotated
  input.
- **Structured specialized diagnostics:** E6 still projects specialized
  violations as generic compiler errors with no specialized schema or witness
  JSON.
- **Cross-machine/cross-target reproducibility:** not claimed. K4.7 covers the
  supported same-build, same-host/target surface.
- **Taxonomy operations without production bindings:** `fs.remove`, `net.*`,
  `io.read`, `process.exec`, `process.exit` and `spawn.*` remain syntax-only or
  HIR-only coverage gaps. A clause for them has no reachable production fact to
  match.

### 10.3 Deferred coverage gaps

- Full accept/reject differentials for unbound taxonomy operations require
  production bindings and remain deferred to later language milestones.
- Native `std.*` VM/JIT differential execution is blocked by the JIT limitation
  above; it must not be fabricated by changing the JIT architecture for this
  gate.
- Cross-process cache-hit specialized side-effect parity is not separately
  exercised. Cache hits are bytecode-only and K4.6B.2 already covers compatible
  hit/miss, stale revision, source identity, failed-write and normal/profiled
  execution behavior.
- Cross-process filesystem/native side effects are not compared. K4.7C's
  cross-process evidence covers violation data, not host side effects.
- Full E6 specialized diagnostic projection and any specialized diagnostic code
  remain deferred without a schema change.
- Cross-machine artifact equality and native-code portability remain outside the
  Level 3 reproducibility claim.

### 10.4 Final gate verdict

**PASS** for the supported K4.7 specialized-effect parity gate: same build,
same host and target, fresh source/file compilation, compatible cache hits,
ordinary/profiled VM, JIT for its supported subset, and explicit rejection on
unsupported execution paths.

No enforcement bypass or demonstrated parity discrepancy was found in those
supported paths. The native-JIT call limitation, Wasm/AOT absence, unbound
taxonomy operations, E6 diagnostic projection and cross-machine scope are
explicitly unsupported or deferred above and are not counted as passing.
K4.8 is the specification-only v0.2 extension; K4.10 will publish the full
per-operation enforceable/deferred ledger after the K4.9 implementation.

## 11. Deferred work and explicit coverage gaps

- Runtime guards for dynamic resources: deferred; no cross-backend guard exists.
- Filesystem prefix/glob/regex matching: not supported; a normalization and
  runtime-confinement contract would be required first.
- `fs.remove`, `net.connect`, `net.listen`, `io.read`, `process.exec`:
  syntax-only coverage gaps with no current production binding.
- `process.exit`: HIR-only; no stable TIR mapping.
- `spawn.task`/`spawn.actor` and `scope`: deferred with milestone C.
- `alloc`/`panic` refinements: deferred pending accounting/failure-taxonomy
  feasibility.
- Wasm/AOT enforcement: future backends must reuse the same analysis and reject
  annotated input if they cannot enforce it.
- Native `std.*` calls under JIT v1: the JIT rejects the calling entry and the
  interpreter executes the native call. Supporting native calls in JIT is
  deferred; this gate does not modify the JIT architecture.
- Cross-machine/cross-target specialized reproducibility: not claimed; K4.7 is a
  same-build, same-host/target Level 3 result.
- Cross-process specialized cache-hit side effects and native filesystem side
  effects: not separately compared; K4.6B.2 covers cache behavior and K4.7C
  covers canonical violation data.
- Structured specialized diagnostics and E6 JSON projection: deferred without
  schema changes.
- Specialized diagnostic code selection: undecided; currently a generic compiler
  error is used.
- Path canonicalization, symlink/hard-link policies, mount aliases and TOCTOU
  guarantees: deferred.
- DNS/address identity and per-operation `Nondeterminism` classification:
  deferred with production `net.*` bindings.
- Native sandboxing and resolver authentication: out of scope.

## 12. Binding decision summary

- Specialized effects are compile-time refinements of the seven existing
  capabilities. They add no capability bits and no public permission.
- Broad `@deny`/`@pure` remains authoritative; specialized clauses only add
  restrictions.
- Operation identity and resource knowledge are separate. A known `fs.write`
  never collapses into generic `Io`.
- Exact values and `*` are the only resource forms. Unknown values under an
  exact restriction fail closed.
- Unclassified operations under a denied parent fail closed. Different known
  operations are disjoint.
- Direct and transitive facts reuse the existing call graph and witnesses.
- Enforcement is compile-time only. There is no runtime guard, filesystem
  sandbox, network isolation or native-code confinement.
- The native resolver identity and the local bytecode cache are trusted. Cache
  artifacts are not signed or authenticated and carry no specialized metadata.
- Unsupported execution paths reject all annotated input.
- `check`, `emit-tir` and raw TIR interpretation are not contract certification.
- K4.7 passes the supported same-build, same-host/target differential parity gate.
  Native `std.*` JIT execution, Wasm/AOT, E6 structured specialized diagnostics,
  unbound taxonomy operations and cross-machine parity are explicitly excluded.
- Effect inspection enforces specialized contracts but currently reports them
  through a generic compiler error, not the E6 structured diagnostic schema.

## 13. References

- [Specialized Effects v0.2](specialized-effects-v0.2.md) — proposed,
  specification-only family/operation extension
- [Capability contracts v0.1](effects-v0.1.md)
- [E1 capability analysis](../E1_CAPABILITY_ANALYSIS.md)
- [Bytecode cache](../BYTECODE_CACHE.md)
- [Architecture](../architecture/ARCHITECTURE.md)
- [Pipeline](../architecture/PIPELINE.md)
- [Roadmap](../roadmap.md)
