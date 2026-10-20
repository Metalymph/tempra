# Tempra Specialized Effects v0.1

> [!NOTE]
> **Status: PROPOSED — documentation only, not implemented.**
>
> This document specifies specialized effect operations and resource-aware
> restrictions as a conservative *extension* of the existing Tempra capability
> contracts ([`effects-v0.1.md`](effects-v0.1.md)). Every syntax example in this
> document is labeled **PROPOSED** and describes intent, not current behavior.
>
> No parser, typechecker, HIR, TIR, driver, runtime, bytecode or cache change is
> part of K4.1. K4.2–K4.8 below are the proposed implementation milestones.
> Nothing here authorizes an implementation to weaken the existing
> `@deny`/`@pure` semantics or to add implicit authorization.

## 1. Purpose and scope

The existing capability surface is deliberately small:

- public contracts: `@deny(...)` and `@pure`;
- internal vocabulary: `Alloc`, `Panic`, `Io`, `Process`, `Spawn`,
  `Nondeterminism`, `Unsafe`;
- analysis: one per-function summary over verified TIR with transitive
  propagation and deterministic witnesses.

`@deny(io)` is all-or-nothing. A function that writes one audit log file and a
function that deletes an arbitrary user directory both require the single `Io`
capability, so they cannot be distinguished. Specialized effects add a
*refinement* layer: a narrow, typed, compile-time restriction on top of the
existing capability bits.

This document specifies:

- the canonical operation taxonomy (section A);
- typed refinement parameters and their closed grammar (section B);
- contract semantics: broad vs. specialized, composition, transitivity,
  unknown operations, dynamic values, specialization (section C);
- enforcement, including fail-closed behavior and the filesystem, DNS and
  native/FFI trust limits (section D);
- backend and tooling parity (section E);
- compatibility with the implemented E1–E6 surface (section F);
- implementation milestones K4.2–K4.8 (section G).

Explicit non-goals, restated as invariants in section 2, are a first-class part
of the specification.

## 2. Architectural invariants

These invariants are binding on all of K4 and on any future revision.

1. **Preserve `CapabilitySet(u8)` and the seven internal capabilities.** The
   `Capability` enum order (`Alloc`, `Panic`, `Io`, `Process`, `Spawn`,
   `Nondeterminism`, `Unsafe`), its discriminants and the bit layout of
   `CapabilitySet` are unchanged. `CapabilitySet` keeps exactly one free bit in
   `u8`; specialized effects **must not** consume it or any capability bit.
   Specialized information lives in a separate, compilation-local side table.
2. **Preserve current `@deny` and `@pure` semantics.** Existing spellings,
   placement, deduplication, union composition, transitive enforcement, cache
   non-bypass and non-enforcing-path rejection are unchanged. `@pure` remains
   `@deny(io, process, spawn, nondeterminism)` and continues to *not* deny
   `Alloc`, `Panic` or internal `Unsafe`.
3. **Specialized effects refine existing capabilities; they are not independent
   permissions.** Every specialized operation has exactly one parent
   capability. Requiring `fs.write` implies requiring `Io`. There is no
   specialized effect that exists without a parent, and no specialized denial
   grants anything. **Operation identity is tracked independently of resource
   knowledge:** a known `fs.write` remains `fs.write` when its destination is
   dynamic; it is never collapsed into generic `Io` merely because the resource
   value is unknown (section 5.4).
4. **No new public general effect system.** No `effect`, `handle`, `perform`,
   `allow`, effect rows, effect polymorphism, implicit authorization, ambient
   authority, or user-defined effects. The public surface stays attribute-shaped
   and closed.
5. **`Unsafe` remains internal and separate.** `@unsafe` stays a deferred,
   conditional boundary (milestone U). Specialized effects never authorize
   unsafe operations, and `@deny(...)` never accepts `unsafe`.
6. **Existing transitive capability analysis remains authoritative** for broad
   capability requirements. Specialized refinements reuse the existing call
   graph, summaries, fixed point and witness machinery; they do not create a
   second analyzer or an independent call graph.
7. **Fail closed.** If an operation cannot be classified to a refinement, or if
   a specialized restriction cannot be decided from available facts, the
   restriction is treated as *violated*, never as satisfied. Absence of evidence
   is never permission.

## 3. A. Canonical operation taxonomy

The v0.1 taxonomy is closed. Adding an operation requires a taxonomy revision,
not a user-defined extension.

| Operation | Parent capability | Meaning | v0.1 classification status |
|---|---|---|---|
| `fs.read` | `Io` | Read filesystem content | PROPOSED |
| `fs.write` | `Io` | Create or modify filesystem content | PROPOSED |
| `fs.remove` | `Io` | Delete filesystem content | PROPOSED |
| `net.connect` | `Io` | Establish an outbound connection | PROPOSED |
| `net.listen` | `Io` | Accept inbound connections | PROPOSED |
| `io.read` | `Io` | Read from a console/stream | PROPOSED |
| `io.write` | `Io` | Write to a console/stream | PROPOSED |
| `process.exec` | `Process` | Execute an external process | PROPOSED |
| `process.exit` | `Process` | Terminate the current process | PROPOSED |
| `spawn.task` | `Spawn` | Spawn structured concurrent work | PROPOSED |
| `spawn.actor` | `Spawn` | Spawn an actor | PROPOSED |

Notes:

- `fs.*`, `net.*` and `io.*` are **siblings** under `Io`. Denying `fs.write`
  does not deny `io.write`, and vice versa.
- `process.exit` reuses the `Process` parent. It terminates the current process
  and is currently an HIR-only operation with no stable TIR mapping, so it is
  **explicitly outside enforceable coverage** until TIR support exists
  (sections 6.3 and 8.2). It is listed for taxonomy completeness only.
- `net.connect` / `net.listen` require `Io`. `Nondeterminism` classification
  remains independent and must follow the actual operation semantics: a
  `net.*` operation requires `Nondeterminism` if and only if the operation
  itself is classified as nondeterministic. A `net.*` clause never implies or
  discharges `Nondeterminism`, and vice versa.
- `spawn.task` / `spawn.actor` are listed so that concurrency restrictions have
  a home before milestone C defines the runtime. They are not enforceable until
  the corresponding runtime operations reach verified TIR. The `scope`
  parameter is deferred with them (section 3.2).

### 3.1 `alloc` and `panic` refinements (separately deferred)

`alloc` and `panic` are **not** part of the v0.1 specialized taxonomy. They are
broad capabilities with deliberately conservative semantics:

- `Alloc` is representation-dependent;
- `Panic` is a may-panic classification, not a proof or a runtime-error
  hierarchy; even `10 / 2` may require `Panic`.

No v0.1 proposal promises:

- allocation budgets, allocation ceilings, or `@no_alloc`;
- verified panic categories, `@no_panic`, or a panic taxonomy;
- totality, termination, or freedom from `Result` errors;
- any correspondence between a specialized `panic` refinement and the future
  `@verified` milestone.

A future `alloc`/`panic` refinement (for example a budgeted allocation contract
or a named panic category) requires its own feasibility contract covering
VM/JIT/Wasm accounting, a stable failure taxonomy, and cross-backend parity
before any syntax is proposed. This document deliberately leaves those slots
empty rather than inventing guarantees.

### 3.2 `spawn.*` scope parameter (deferred)

`spawn.task` and `spawn.actor` are present in the taxonomy so that concurrency
restrictions have a defined home, but their `scope` parameter is **deferred**
until the milestone C runtime and ownership model exist. Until then:

- `spawn.task`/`spawn.actor` remain unenforceable and are recorded as coverage
  gaps, not silently authorized;
- no `scope` semantics, scope name resolution, nesting rule or default is fixed
  here;
- a specialized `spawn.*` clause with a `scope` argument cannot be frozen as
  syntax, because its feasibility prerequisite does not exist yet.

## 4. B. Typed refinement parameters

Each specialized operation may carry a small, closed set of **typed
parameters**. Parameters are compared statically; they are **not** general
predicates.

### 4.1 Closed parameter grammar (PROPOSED)

```text
param        := name ":" value
name         := "source" | "destination" | "host" | "port"
              | "protocol" | "executable" | "stream" | "scope"
value        := "*"                          // explicit any resource value
              | string-literal               // exact match only
              | integer-literal              // port
              | protocol-literal             // tcp | udp
              | stream-literal               // stdin | stdout | stderr
              | scope-identifier             // statically named scope
```

Restrictions:

- **Exact literals and `*` only.** No regular expressions, globs, substrings,
  filesystem prefix matching, CIDR/prefix matching, ranges, or boolean
  combinators in v0.1.
- No expressions, variables, function calls or runtime values as parameters.
  Only literals written at the declaration site.
- `*` matches any resource value of an **identified operation**. It does not
  match a different operation in the same parent capability. Omitting a field is
  equivalent to `*` for that field.
- A field not listed as meaningful for an operation is a syntax/semantic error,
  not an ignored parameter.
- Multiple fields on one clause are **conjunctive** (all must match). Multiple
  clauses for the same operation are **disjunctive** (any may match).

> **Grammar compatibility (approval required).** The preferred surface uses the
> `name: value` colon form inside the existing `@deny(...)` argument list. This
> document does **not** freeze that grammar: compatibility with the current
> `DenyContract` argument grammar, the lexer and the parser must be confirmed
> before syntax is specified as final (section 12.2, item 1). The `=` form is not
> proposed.

### 4.2 Meaningful parameters per operation

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
| `process.exit` | *(none)* |
| `spawn.task` | `scope` |
| `spawn.actor` | `scope` |

`source` and `destination` are path spellings compared as opaque strings.
They are **not** canonical, absolute, or filesystem-resolved identities
(section 6.4). `host` is an opaque name compared as written (section 6.5).
`scope` is a statically named grouping label; its semantics for
`spawn.task`/`spawn.actor` are deferred (section 3.2).

### 4.3 PROPOSED syntax examples (not implemented)

```tempra
// PROPOSED — exact filesystem destination only.
@deny(fs.write(destination: "build/out.txt"))
fn emit_report() { ... }

// PROPOSED — any outbound TCP connection is denied (port wildcard).
@deny(net.connect(protocol: tcp, port: *))
fn connect_anywhere() { ... }

// PROPOSED — exact host and port.
@deny(net.connect(host: "db.internal", port: 5432))
fn connect_db() { ... }

// PROPOSED — stdout only; other streams remain allowed.
@deny(io.write(stream: stdout))
fn quiet() { ... }

// PROPOSED — a specific executable.
@deny(process.exec(executable: "/usr/bin/rm"))
fn guard_exec() { ... }
```

## 5. C. Contract semantics

### 5.1 Broad vs. specialized denial

- **Broad denial** `@deny(io)` denies every `Io` operation, including all
  `fs.*`, `net.*` and `io.*` siblings. This is exactly today's behavior and
  continues to be decided by `denied ∩ required` over capability bits. **Broad
  capability denial stays authoritative**; specialized clauses only *add*
  restrictions and never relax, replace or override it.
- **Specialized denial** `@deny(fs.write(destination: "x"))` adds a restriction
  on `fs.write` clauses that match. Other `Io` operations — `fs.read`,
  `io.write`, `net.connect`, a *different* `fs.write` destination — remain
  allowed.
- Broad denial **subsumes** specialized denial of the same parent: if the
  contract already denies the parent capability, every refinement is denied and
  the specialized clause is redundant (valid, but not required). This mirrors
  how explicit `io` alongside `@pure` is redundant but accepted today.
- Specialized denial never implies broad denial. `@deny(fs.write)` does **not**
  imply `@deny(io)`.

Because a broad capability requirement alone cannot distinguish
`fs.write(destination: "x")` from `fs.read`, specialized enforcement requires the
refinement layer from section 6.1. When that layer is unavailable for a required
parent capability, specialized denial fails closed (invariant 7, section 5.4).

### 5.2 Repeated annotations and composition

- Repeated specialized clauses for the same operation **union** into a set of
  denials; each clause keeps its own parameter list and source span.
- Repeated identical clauses deduplicate.
- Specialized and broad clauses compose by union, matching existing `@deny`
  composition: order does not matter, and the effective restriction is the
  union of every clause.
- `@pure` plus a specialized clause is valid; `@pure` already denies the parent
  capability broadly, so the specialized clause is subsumed.
- A function may not both require and deny the same operation by construction;
  enforcement compares declared denial against required operations, not against
  other annotations.

### 5.3 Direct and transitive effects

- Direct specialized requirements originate at a classified TIR instruction or
  a classified known extern call, exactly like today's direct capability
  origins.
- Transitive requirements propagate along the existing internal call graph to a
  fixed point, reusing E1 edges and summaries. A callee that requires
  `fs.write(destination: "x")` makes every caller require `fs.write` and, through
  the parent rule, `Io`.
- **Least-refinement rule (operation identity preserved).** Parameters do not
  flow through calls as values. If a callee requires `fs.write` with a
  statically known destination, a caller observes `fs.write` with that same
  declaration-site literal if and only if the requirement can be attributed
  without value propagation. Otherwise the caller observes the *least refined*
  form of the requirement: **the same operation identity** (`fs.write`) with an
  **unknown resource**. Resource knowledge degrades; operation identity does
  not. A known `fs.write` is never collapsed into generic `Io` merely because its
  destination became dynamic. A specialized denial can only be discharged by an
  exact match; unknown resource values therefore violate any restrictive
  resource clause of the same operation and are only covered by broad denial.
  This is the fail-closed default, not a best effort (section 5.4).
- Broad `Io`/`Process`/`Spawn` propagation is unchanged and remains
  authoritative for callers that only deny broad capabilities.

### 5.4 Operation identity versus resource knowledge

Operation identity and resource knowledge are separate axes. Enforcement must
model these states distinctly instead of collapsing every `Io` requirement into
one generic fact:

1. **Known operation, known resource.** A classified operation with a
   declaration-site literal resource, for example
   `fs.write(destination: "x")`.
2. **Known operation, unknown resource.** The operation is identified
   (`fs.write`), but the relevant resource is dynamic or was lost at a call
   boundary, for example `fs.write(destination: ?)`.
3. **Unknown operation within a known parent capability.** A requirement of
   `Io` (or `Process`, `Spawn`) that cannot be refined to a specific operation.

Matching is conservative:

- **Different known operations do not match.** An `fs.read` clause never
  matches an `fs.write` requirement, and an `io.write` clause never matches an
  `fs.write` requirement.
- **Same operation, unknown relevant resource cannot satisfy a restrictive
  resource clause.** `fs.write(destination: "x")` cannot be discharged by
  `fs.write(destination: ?)`; unknown resource knowledge violates a restrictive
  clause and is only covered by broad denial.
- **Unknown operation within the denied parent cannot be assumed safe.** Any
  specialized denial under that parent (`fs.*`, `net.*`, `io.*`, ...) is
  violated by an unrefined parent requirement, because the unknown operation
  might be the denied one.
- **`*` matches any resource value of an identified operation.** A clause with
  `*` (or an omitted resource field) accepts any resource value of that
  operation, but still does not match a different operation or an unknown
  operation.

Known operation identity is preserved across calls; only resource knowledge may
degrade (section 5.3). A specialized denial is never satisfied by guessing that
an unidentified operation is a different sibling.

### 5.5 Unknown or unclassified operations

- If an operation requires a parent capability but cannot be classified to a
  known refinement, it is recorded as an **unrefined** requirement of that
  parent capability (section 5.4, state 3).
- Broad denial of the parent still rejects it (`denied ∩ required`).
- Any specialized denial whose parent equals that capability is **violated** by
  an unrefined requirement, because the unrefined operation might be the denied
  operation. The requirement is never silently assumed to be a different
  sibling (section 5.4).
- An unknown or unclassified extern that is actually called is already an
  error today (`UnclassifiedExtern`); specialized effects do not relax that.
  An unused unknown extern remains harmless.
- New operations that reach TIR without a taxonomy entry are coverage gaps to be
  recorded, not implicitly authorized and not implicitly mapped.

### 5.6 Dynamic resource values

- A resource parameter is decidable only when it is a declaration-site literal.
- If the actual resource is computed at runtime (a path built from input, a port
  from a variable, a host from configuration), the requirement becomes
  **known operation, unknown resource** (section 5.4, state 2). It does **not**
  become generic `Io`.
- Unknown resource knowledge violates every restrictive resource clause of that
  operation (sections 5.3 and 5.4). v0.1 does **not** promise a runtime guard
  that could recover a precise decision (section 6.2).
- A program that needs a dynamic resource and wants a broad guarantee should
  deny the parent capability, not a specialization.

### 5.7 Specialization and function identity

- Contracts bind to concrete `TirFunctionId`s exactly as today. `TirFunctionId`
  remains authoritative; names are display-only.
- A generic template's specialized instances inherit the template's contract
  and declared intent unchanged, including declaration-site literal parameters.
  Type substitution does not substitute resource values, because resource
  parameters are not type parameters; **no resource-value substitution is
  supported**.
- Ambiguous or conflicting contract binding is a structured error
  (`ContractBindingError`), unchanged.
- A generic function that would need a *resource-polymorphic* contract cannot
  express one in v0.1; that is a non-goal (section 2, invariant 4).

## 6. D. Enforcement

### 6.1 Compile-time classification

- Specialized classification runs in the same middle-end window as E1: after
  TIR verification, alongside capability analysis, before contract enforcement
  and bytecode lowering.
- The classifier is exhaustive over TIR instructions and the known extern
  table, exactly like E1. Instructions with no specialized meaning map to an
  empty refinement set rather than falling through a wildcard.
- Specialized facts are stored in a compilation-local side table keyed by
  `TirFunctionId`, `TirLocation` and parent `Capability`. They are **not**
  encoded in `CapabilitySet`, `Capability`, bytecode, or cache artifacts.
- The existing `CapabilityAnalysis` remains the authority for broad
  requirements. Specialized enforcement consumes it and does not rebuild the
  call graph.

### 6.2 Runtime guards

v0.1 does **not** promise runtime guards. This is a deliberate, conservative
choice:

- A runtime guard can only be correct if it fails closed, and it introduces
  backend-visible behavior that would need parity across HIR/VM/JIT and future
  Wasm/AOT.
- Dynamic filesystem and network identities are exactly the cases where guards
  are most tempting and least trustworthy (sections 6.4, 6.5).
- If a future milestone adds a guard, this document requires: (a) fail-closed on
  guard failure, (b) identical accept/reject behavior across backends, (c) an
  explicit diagnostic distinguishing a runtime denial from a compile-time
  denial, and (d) a reproducible parity test. Until then, an operation with
  unknown parameters is treated as unrefined and fails closed for specialized
  denial.

### 6.3 Fail-closed behavior

A specialized denial is treated as violated (never satisfied) when any of the
following holds (see the states in section 5.4):

1. the requirement is an **unknown operation** within the denied parent
   capability (state 3);
2. the requirement is a **known operation with unknown relevant resource**
   (state 2), and the clause restricts that resource;
3. the requirement is a known operation whose refinement cannot be classified
   from verified TIR;
4. the operation is HIR-only with no stable TIR mapping (for example
   `process.exit` today);
5. a called extern has no refinement classification for a parent capability the
   contract specializes;
6. contract binding, summaries or witnesses are missing (typed invariant
   failure, never synthetic success).

Broad denial continues to use the existing typed invariant failures for missing
functions, summaries and witnesses.

### 6.4 Filesystem normalization and symlink limitations

v0.1 compares `source`/`destination` as **opaque source-literal strings**. It
does not promise filesystem-level confinement. Specifically, v0.1 makes no
guarantee about:

- relative vs. absolute equivalence;
- `.`/`..` normalization, duplicate separators, or trailing separators;
- symbolic links, hard links, bind mounts, or mount aliases;
- case-insensitive or Unicode-normalizing filesystems;
- time-of-check/time-of-use races between analysis and execution;
- files reached indirectly by a helper that opens a computed path.

Therefore a `fs.*` specialization is a *source-level spelling check*, not a
sandbox. Documentation and diagnostics must not describe it as filesystem
confinement. Prefix, canonicalization and symlink-aware policies are future
work requiring a feasibility contract and runtime enforcement.

### 6.5 DNS and network identity limitations

`host` is an opaque source-literal string. v0.1 makes no guarantee about:

- DNS resolution results, CNAME chains, round-robin or multiple `A`/`AAAA`
  records, or DNS rebinding;
- IP address equivalence, NAT, proxies, tunnels, or port forwarding;
- TLS SNI, certificate identity, or redirects;
- dynamic ports (`port: 0`) or ports selected at runtime.

A `net.*` specialization is therefore a *declared-intent check*, not network
isolation. `net.connect`/`net.listen` require `Io`. `Nondeterminism`
classification remains independent and follows the actual operation semantics:
a `net.*` operation requires `Nondeterminism` only if the operation itself is
classified as nondeterministic. No `net.*` guarantee here implies DNS
resolution, address identity, or nondeterminism enforcement.

### 6.6 Native/FFI trust boundaries

- The known extern table is the authority for extern refinement classification.
  Its accuracy is bounded by the native registry and by the existing
  native/extern signature-integrity debt recorded in the E5 audit.
- The native registry must **explicitly classify** each known operation it
  exposes. Classifications are not inferred from a function's name, module,
  arguments or return type. An extern that is not explicitly classified fails
  closed.
- K4.6B.1 makes the current registry/classifier agreement testable. Tests derive
  native names from the production registry, compare them in both directions
  with the compiler-owned classifier, and check the declared `std/*.tmpr`
  signatures and the native argument positions. This is regression coverage, not
  a generated single source of truth.
- Classified operation identity, resource parameter keys and positional argument
  bindings are **compiler-owned metadata**. The tests prove agreement with the
  current native implementations; they do not let a native function choose or
  relax its own classification.
- Arbitrary native code is **not** sandboxed. A specialized effect can only
  describe what the classifier knows; it is not a memory- or capability-safety
  boundary for host code.
- An extern call with no specialized classification for a denied parent
  capability fails closed (section 6.3).
- Filesystem paths remain opaque source strings and are not canonicalized.
  Symlinks, hard links, mount aliases and time-of-check/time-of-use races remain
  outside static guarantees (section 6.4).
- The runtime native resolver's identity is trusted. A different resolver may
  execute different code with the same names; the compiler cannot authenticate
  the implementation behind a resolved function pointer.
- `Unsafe` and any future FFI authorization remain out of scope for K4.

## 7. E. Backend and tooling parity

### 7.1 Backends

- Enforcement is a front/middle-end acceptance decision made before bytecode
  lowering. VM and JIT therefore inherit the same accept/reject outcome; JIT
  executes already-enforced bytecode.
- HIR and raw source-TIR execution currently reject any annotated input because
  they cannot enforce contracts. That policy is unchanged: specialized
  annotations are also rejected on those paths (invariant 2, section F.2).
- A future Wasm/AOT backend must invoke the same analysis and enforcement
  pipeline. A backend that cannot enforce a specialized contract must reject
  annotated input rather than executing it.
- If runtime guards are ever added (section 6.2), per-backend parity tests are
  mandatory.

### 7.2 Diagnostics, witnesses, provenance

- Diagnostics reuse E6's immutable `EffectDiagnostic`/`EffectInspection`
  projections. The existing broad diagnostic code `capability-denied` is
  **retained**. Whether specialized violations also use a distinct code such as
  `capability-specialized-denied` or `capability-unclassified` is **explicitly
  undecided** and must not be assumed by an implementation (section 12).
- Specialized facts are **not** added to the E6 `--json` document until
  specialized enforcement is implemented. The current schema stays a current
  tooling schema, not a permanent protocol (section 12).
- A witness for a specialized violation extends the existing canonical witness
  with the refinement kind and the matched declaration-site literal. Witness
  lookup keeps the existing deterministic shortest-path and tie-break rules,
  extended with a stable refinement/parameter ordering.
- Provenance reuses existing declaration spans, attribute spans, `pure_span` and
  source identity. Specialized parameters must remain byte-offset-addressable
  within their attribute so diagnostics can point at the clause.
- Intermediate callsites remain structural TIR coordinates until a lowering
  side table maps them to source spans; K4 does not invent instruction spans.

### 7.3 Determinism and reproducibility

- Iteration order is: ascending `TirFunctionId`, then capability declaration
  order, then a stable taxonomy order for operations, then clause source order,
  then parameter order.
- Hash containers are lookup-only at semantic and diagnostic boundaries.
- Repeated analysis of identical input yields identically ordered summaries,
  refinements, violations and witnesses, consistent with K2's determinism
  levels.

## 8. F. Compatibility

### 8.1 Existing `@deny`/`@pure` unchanged

Existing broad spellings keep their exact meaning and enforcement. Adding
specialized syntax must not change the accepted set of any program that uses
only broad capabilities, except that programs which previously could not name a
specialized restriction remain under broad semantics. `@pure` continues to
deny `io`, `process`, `spawn`, `nondeterminism` broadly.

### 8.2 Current E1 coverage gaps remain explicit

`Process`, `Spawn`, `Nondeterminism` and parts of `Io` still have incomplete TIR
mapping, and several operations (process commands, environment access, input,
exit, parts of `os`) are HIR-only. Specialized effects must **record** these as
gaps rather than inventing semantics. A specialized restriction that cannot be
enforced against an unclassified operation fails closed; it never assumes the
operation is a different sibling.

### 8.3 No bytecode or cache format assumptions

- No specialized facts, contracts or witnesses are serialized into bytecode.
- The bytecode cache remains bytecode-only. Trusted hits verify and execute
  without rebuilding TIR or synthesizing specialized facts.
- This specification does **not** pre-commit a `semantics-N` bump. At
  implementation time, if specialized enforcement changes which programs are
  accepted, the trust fingerprint must be raised then so pre-K4 artifacts cannot
  bypass it. Bytecode serialization format stays independent of `semantics-N`.

### 8.4 No sandbox claim

Nothing in K4 authorizes claiming that arbitrary native code, filesystem
access, or network access is sandboxed. Specialized effects are compile-time
source-contract checks with the explicit limitations of sections 6.4–6.6.

## 9. G. K4.2–K4.8 implementation milestones

K4.1 (this document) is the specification. The milestones below are ordered by
dependency. Each lists its deliverable, acceptance criteria and non-goals.

### K4.2 — Taxonomy and typed parameter model

- **Depends on:** K4.1.
- **Deliverable:** an internal, syntax-free Rust representation of the closed
  taxonomy and typed parameters from sections 3–4, with parent-capability
  mapping and a stable order.
- **Acceptance:** every operation has exactly one parent; parameter validity per
  operation is enforced; `Capability`/`CapabilitySet` are untouched (including
  the free `u8` bit); unit tests cover taxonomy/parameter validity and ordering.
- **Non-goals:** no parser/HIR/typechecker changes, no diagnostics, no
  enforcement, no public syntax.

### K4.3 — TIR specialized classification

- **Depends on:** K4.2.
- **Deliverable:** an exhaustive specialized classifier over `TirInstr` and the
  known extern table, producing compilation-local `(function, location,
  operation, resource knowledge)` facts. Operation identity and resource
  knowledge are recorded separately; unclassified parent-capability operations
  are recorded explicitly as unknown-operation-within-parent facts.
- **Acceptance:** every classified operation carries its parent capability and
  retains its operation identity when the resource is dynamic; the classifier is
  exhaustive with an explicit empty case rather than a catch-all; known externs
  map deterministically through explicit registry classification; called
  unclassified externs still error; the existing call graph is reused, not
  rebuilt.
- **Non-goals:** no transitive propagation, no enforcement, no syntax.

### K4.4 — Specialized propagation, summaries and witnesses

- **Depends on:** K4.3.
- **Deliverable:** specialized requirements propagated to a fixed point over
  existing call edges using the three-state knowledge model (section 5.4), with
  the least-refinement rule (operation identity preserved, resource knowledge
  degraded) and canonical witnesses extended to refinements and resource
  knowledge.
- **Acceptance:** recursion/SCCs terminate; the least-refinement rule is tested,
  including that a known `fs.write` never collapses into generic `Io`; the
  conservative matching rules of section 5.4 are tested; witnesses are
  deterministic under the extended tie-break order; broad E1 summaries and
  behavior are unchanged.
- **Non-goals:** no surface syntax, no user-facing diagnostics, no runtime
  guards.

### K4.5 — Surface syntax and frontend provenance

- **Depends on:** K4.4.
- **Deliverable:** PROPOSED syntax from sections 4–5 acquired as syntax-only AST
  and HIR data, with the preferred `name: value` clause grammar confirmed for
  compatibility, placement, deduplication and union composition matching
  pending approval (section 12), and a single driver-side canonical conversion.
- **Acceptance:** parser rejects malformed clauses, unknown fields, non-literal
  parameters, prefix/glob/regex-looking values and unsupported operations; HIR
  copies syntax without importing `tempra-effects`; repeated clauses union while
  preserving spans; broad syntax is unchanged.
- **Non-goals:** no enforcement yet, no cache change, no second analyzer, no
  general attribute framework, no grammar change frozen before compatibility is
  confirmed.

### K4.6 — Enforcement, diagnostics and cache trust

- **Depends on:** K4.5.
- **Deliverable:** refinement-aware enforcement combining broad
  `denied ∩ required` with specialized clause matching over the three-state
  knowledge model, fail-closed decisions, diagnostics and inspection
  projection, and a conditional `semantics-N` bump (only if acceptance changes).
- **Acceptance:** broad behavior is unchanged and remains authoritative;
  specialized violations carry exact provenance and witnesses; every
  fail-closed case in section 6.3 has a regression; conservative matching
  (different operations, unknown resource, unknown operation) is tested;
  HIR/source-TIR/test/REPL paths reject annotated input before execution; no
  facts are serialized; cache writes occur only after successful enforcement and
  bytecode verification.
- **Non-goals:** no runtime guards, no filesystem resolution, no network
  isolation, no sandbox claim, no automatic cache-revision bump.

### K4.7 — Backend parity and reproducibility gate

- **Depends on:** K4.6.
- **Deliverable:** a differential gate proving identical accept/reject and
  diagnostics across the analyzed VM path and any other enforcing backend, plus
  determinism/reproducibility checks for specialized ordering and witnesses.
- **Acceptance:** the same annotated program yields the same decision and stable
  diagnostic ordering on every enforcing backend; non-enforcing backends reject;
  repeated fresh processes produce identical results; any future runtime guard
  has explicit parity tests.
- **Non-goals:** no premature Wasm/AOT feature work beyond pipeline reuse; no
  cross-machine reproducibility claim.

### K4.8 — Coverage-gap ledger and non-goal review

- **Depends on:** K4.7.
- **Deliverable:** an explicit ledger of known operations, known operations
  with dynamic resources and unknown operations within a parent capability; a
  review that no implied authorization, sandbox claim or `alloc`/`panic` budget
  sneaked in; resolution or deferral of the open questions in section 12.
- **Acceptance:** every taxonomy operation is marked enforceable or explicitly
  deferred with a reason; no advertised guarantee exceeds sections 6.4–6.6;
  deferred `alloc`/`panic` refinements and the deferred `spawn.*` scope
  parameter each have a stated feasibility gate.
- **Non-goals:** no expansion into a general policy language; no unsafe
  boundary work.

## 10. PROPOSED syntax summary (not implemented)

All examples in this document are PROPOSED. For quick reference:

```tempra
// PROPOSED — broad capability denial (unchanged, implemented today).
@deny(io)
fn a() { ... }

// PROPOSED — specialized denial of one filesystem destination.
@deny(fs.write(destination: "report.txt"))
fn b() { ... }

// PROPOSED — composition; the effective restriction is the union.
@pure
@deny(fs.remove(destination: "cache"))
fn c() { ... }
```

A program using only broad `@deny`/`@pure` must not change behavior.

## 11. Failure and diagnostic sketch (PROPOSED)

The existing broad diagnostic shape is preserved and extended with the
specialized clause. Exact wording is not fixed.

```text
error: function 'emit_report' denies specialized effect 'fs.write'
denied clause:
  fs.write(destination: "build/out.txt")
required operation:
  fs.write(destination: "build/out.txt")
introduced by:
  emit_report
call path:
  emit_report
  └── write_output
      └── std.fs.write_text
```

For a known operation with unknown resource, or an unknown operation within the
denied parent, the diagnostic must say that the decision failed closed because
the resource or operation could not be identified, rather than claiming a match.

## 12. Semantic decision ledger

### 12.1 Decisions incorporated by K4.1A

These are settled for v0.1. An implementation must not silently re-litigate
them.

1. **Operation identity and resource knowledge are separate.** Enforcement
   models (a) known operation, known resource, (b) known operation, unknown
   resource, and (c) unknown operation within a known parent capability
   (section 5.4).
2. **Known operation identity survives calls.** Resource knowledge may degrade
   to unknown; a known `fs.write` is never collapsed into generic `Io`
   (sections 5.3 and 5.4).
3. **Matching is conservative.** Different known operations never match; a
   known operation with unknown relevant resource cannot satisfy a restrictive
   resource clause; an unknown operation within the denied parent is never
   assumed safe (section 5.4).
4. **Exact literals and `*` only.** No filesystem prefix, glob or regex
   matching. `*` matches any resource value of an identified operation, not any
   operation in its parent capability (sections 4.1 and 5.4).
5. **Broad capability denial stays authoritative.** Specialized clauses only add
   restrictions; they never relax, replace or override broad `@deny`/`@pure`
   semantics (sections 5.1 and 8.1).
6. **Preferred specialized syntax is the colon clause form**
   `@deny(fs.write(destination: "/private"))`. The `=` form is rejected as a
   proposal. Grammar compatibility with the existing parser is still to be
   confirmed (section 12.2, item 1).
7. **`net.connect` / `net.listen` require `Io`.** `Nondeterminism`
   classification stays independent and must follow actual operation semantics
   (sections 3 and 6.5).
8. **`process.exit` stays under `Process`** and is explicitly outside
   enforceable coverage until a stable TIR mapping exists (sections 3 and 6.3).
9. **`alloc`/`panic` refinements and the `spawn.*` scope parameter are
   deferred** until their feasibility prerequisites exist (sections 3.1 and
   3.2).
10. **Generic specialization preserves declared contract intent.** Concrete
    specializations inherit the template contract unchanged; no resource-value
    substitution is supported (section 5.7).
11. **The broad diagnostic code `capability-denied` is retained.** The
    specialized diagnostic-code choice stays explicitly undecided (sections 7.2
    and 12.2).
12. **E6 JSON is extended only after implementation**, and **no `semantics-N`
    bump is pre-committed**. A bump is decided at implementation time only if
    enforcement changes acceptance (sections 7.2 and 8.3).
13. **The native registry explicitly classifies known operations**, and unknown
    externs fail closed. Classification is not inferred from names or types
    (section 6.6).

### 12.2 Remaining genuinely unresolved questions (approval required)

No implementation may pick a default that weakens the fail-closed invariant.

1. **Grammar compatibility.** Confirm the `name: value` clause form against the
   existing lexer and `DenyContract` argument grammar before freezing syntax.
   Are grouped/nested forms ever wanted, or is the flat clause form final?
2. **Prefix matching.** v0.1 is exact-literal plus `*` only. Is filesystem
   prefix matching ever required? If so, a normalization contract is a
   prerequisite and remains unspecified.
3. **Runtime guards.** Confirm the deferral for dynamic resources, or request a
   feasibility study for a fail-closed runtime guard with cross-backend parity.
4. **Specialized diagnostic code.** Decide whether to reuse `capability-denied`
   or introduce a distinct code such as `capability-specialized-denied` or
   `capability-unclassified`. Deliberately deferred.
5. **E6 JSON shape.** If and when specialized facts are surfaced, which
   current-tooling-schema keys are used.
6. **Cache revision.** Decide at implementation time whether specialized
   enforcement changes acceptance enough to require a `semantics-N` bump.
7. **Per-operation `Nondeterminism` classification.** The rule (follow actual
   operation semantics) is settled, but the concrete classification of each
   `net.*` operation still must be defined against the native registry.
8. **`spawn.*` scope semantics.** Deferred with milestone C; `scope` syntax
   cannot be frozen before its feasibility prerequisite exists.
9. **`alloc`/`panic` feasibility.** Deferred; requires VM/JIT/Wasm accounting
   and a stable failure taxonomy before any budget or category syntax.
10. **Native classification table completeness.** The registry must explicitly
    classify known operations; the actual operation-to-refinement table is
    implementation work (K4.3).

## 13. Relationship to other milestones

- **E1–E6:** reused unchanged. Specialized effects are a refinement layer over
  the existing analysis, not a replacement.
- **V (`@verified`):** specialized denial is not verification. `@deny(panic)`
  remains may-panic and specialized `panic` refinements are out of scope.
- **C (actors):** `spawn.*` taxonomy entries exist so concurrency work has a
  home, but their enforcement waits for the runtime and ownership model.
- **U (unsafe):** unaffected and separate. Specialized effects never authorize
  unsafe operations.

## 14. Summary of binding decisions

- Specialized effects are typed refinements of the seven existing capabilities,
  never independent permissions, and never new `CapabilitySet` bits.
- Operation identity and resource knowledge are separate axes: known operation /
  known resource, known operation / unknown resource, and unknown operation
  within a known parent capability. A known `fs.write` is never collapsed into
  generic `Io` because its destination is dynamic.
- Matching is conservative: different known operations never match; unknown
  resource knowledge cannot satisfy a restrictive resource clause; an unknown
  operation within the denied parent is never assumed safe. `*` matches any
  resource value of an identified operation, not any operation in its parent.
- `@deny`/`@pure` semantics, analysis, witnesses and cache policy are preserved,
  and broad capability denial remains authoritative.
- The taxonomy is closed; parameters are exact-literal plus `*` only, with no
  prefix/glob/regex matching and no runtime values.
- `net.connect`/`net.listen` require `Io`; `Nondeterminism` classification is
  independent. `process.exit` stays under `Process` and outside enforceable
  coverage until TIR support exists.
- Classification and enforcement fail closed for unknown operations, unknown
  resources and unclassified requirements.
- Filesystem and network specializations are source-spelling/intent checks with
  explicitly stated limitations, not sandboxes. No runtime guards are promised.
- The broad diagnostic code is retained; specialized diagnostic codes and E6
  JSON extension are deferred until implementation. No `semantics-N` bump is
  pre-committed.
- Backend parity and deterministic ordering are acceptance criteria, not
  aspirations.
- `alloc`/`panic` refinements and `spawn.*` scope are deferred until their
  feasibility prerequisites exist.
- No budgets, no general effect system, no unsafe boundary work, and no
  arbitrary native-code confinement claim.
