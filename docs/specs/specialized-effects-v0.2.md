# Tempra Specialized Effect Families and Operation Semantics v0.2

> [!NOTE]
> **Status: normative specification only. Not implemented.**
>
> This document defines the proposed v0.2 semantics for specialized effect
> families, parameterless operation denials, cross-family read/write categories
> and the missing network taxonomy. It is the K4.8 semantic-consolidation
> specification.
>
> The current compiler still implements
> [Specialized Effects v0.1](specialized-effects-v0.1.md) and cache trust
> `semantics-6`. No production code, tests, parser, AST, taxonomy, matcher,
> native classifier, cache revision or runtime behavior is changed by this
> document. The requirements in section 11 are implementation work for later
> milestones, not descriptions of current behavior.
>
> The seven fundamental capabilities, static-only enforcement, the absence of
> runtime guards, the native/FFI trust model and all current runtime and cache
> limitations remain unchanged.

## 1. Purpose and scope

v0.2 extends the existing specialized-effect refinement without adding a new
capability system. It adds three things:

1. **Families.** `fs` and `net` are families under the existing `Io`
   capability. A family denial is a durable predicate over every operation in
   that family, including operations added in future taxonomy revisions.
2. **Cross-family categories.** `io.read` and `io.write` are semantic categories
   that span filesystem, network and generic I/O operations without changing any
   operation's parent capability.
3. **Complete operation target syntax.** Every concrete specialized operation can
   be denied without a parameter list, and two missing network operations,
   `net.send` and `net.receive`, are added to the taxonomy.

The normative hierarchy is:

```text
Capability
  └── Family (optional)
        └── Operation
              └── Resource
```

A **capability** is one of the seven existing fundamental capabilities. A
**family** is a compiler-owned grouping under `Io`. An **operation** is one
specialized effect identity. A **resource** is an operation-specific typed
parameter. A **category** is a named predicate over operations; it is not a
family and does not change an operation's capability identity.

This specification is static. It does not create runtime checks, endpoint
tracking, filesystem sandboxing, network isolation, actor execution or native
APIs.

## 2. Current implementation versus proposed semantics

The distinction is normative. A future implementation must not describe current
v0.1 behavior as if v0.2 were already implemented.

| Area | Current v0.1 implementation | Proposed v0.2 semantics |
|---|---|---|
| Capabilities | Seven analyzer capabilities; six public broad `@deny` spellings (`Unsafe` remains internal). | Unchanged. No capability bit or public permission is added. |
| Families | No family concept. `fs` and `net` are only operation-name prefixes. | `fs` and `net` are families under `Io`, but do not become capabilities. |
| Family syntax | `@deny(fs)` and `@deny(net)` are rejected as unknown public deny capabilities. | `@deny(fs)` and `@deny(net)` are valid family denials. |
| Operation taxonomy | 11 operations. `net.connect`, `net.listen`, `io.read`, `io.write` and the other listed operations exist as concrete siblings. | 13 operations: adds `net.send` and `net.receive`. Existing operation names remain. |
| Parameterless operation syntax | A bare operation name is accepted only when the operation declares no parameters (`process.exit`). Operations with declared parameters require parentheses, including `fs.write()`. | Every concrete operation may be written with an empty parameter list. A bare name denotes a parameterless operation except for `io.read`/`io.write`, where the bare name denotes the category and `()` selects the generic operation. See section 4.2. |
| `io.read` / `io.write` | Concrete operations under `Io`. A denial of `io.read` is disjoint from `fs.read`. Bare `@deny(io.read)` is a parse error because the operation has a resource parameter. | `io.read` and `io.write` are cross-family categories when written bare. Parenthesized `io.read(...)`/`io.write(...)` target the concrete generic operation. See section 4.2. |
| Cross-family membership | None. | `io.read` covers `fs.read`, `net.receive` and generic `io.read`; `io.write` covers `fs.write`, `net.send` and generic `io.write`. |
| Family future-proofing | Not applicable. | Family denials are stored as predicates, never as fixed expansions over currently known operations. |
| Matcher | Compares one concrete operation identity and its resource parameters. Unclassified operations are potential matches only for the operation's parent capability. | Adds family and category membership predicates with the same fail-closed rules. |
| Network operations | No production native bindings. `net.listen` is documented as accepting inbound connections. | `net.listen` is listener creation/binding. `net.connect` is connection establishment. `net.send` and `net.receive` are data transmission/reception. Endpoint/connection identity is explicitly deferred. |
| Semantic revision | `semantics-6`. | The future implementation must use a new semantic revision (expected `semantics-7`). No revision change is made by this document. |

The current implementation remains the authority for behavior until the
implementation milestones in section 16 are complete.

## 3. Formal hierarchy and taxonomy

### 3.1 Fundamental capabilities

The seven fundamental capabilities remain exactly:

- `Alloc`
- `Panic`
- `Io`
- `Process`
- `Spawn`
- `Nondeterminism`
- `Unsafe`

`Unsafe` remains internal. The public broad `@deny` spellings remain the
existing six (`alloc`, `panic`, `io`, `process`, `spawn`,
`nondeterminism`), with `@pure` unchanged. Families and categories add no
`CapabilitySet` bit and no new public permission.

`@deny(io)` continues to deny the entire `Io` capability, including every
filesystem, network and generic I/O operation. Family and category denials are
subsets of that capability; they never relax, replace or discharge a broad
`Io` denial.

### 3.2 Families

v0.2 defines exactly two families:

| Family | Parent capability | Membership requirement |
|---|---|---|
| `fs` | `Io` | An operation on filesystem namespace or content. |
| `net` | `Io` | An operation on a network connection, listener or data transfer. |

A family is **not** a capability and is **not** serialized as a `CapabilitySet`
bit. `@deny(fs)` is a specialized family restriction, not a broad `Io` denial.
It must not be implemented as `@deny(io)` and must not deny unrelated `Io`
operations such as generic `io.write`.

Families are closed at any one specification revision but extensible across
revisions. A family denial is a predicate over the family's operation
membership. It is not a fixed union of the operations known at the time the
clause is parsed.

### 3.3 Operations and resources

The proposed operation taxonomy is:

| Family | Operation | Meaning | Resource parameters |
|---|---|---|---|
| `fs` | `fs.read` | Read filesystem content. | `source` |
| `fs` | `fs.write` | Create or modify filesystem content. | `destination` |
| `fs` | `fs.remove` | Delete filesystem content. | `destination` |
| `net` | `net.connect` | Establish an outbound network connection. | `host`, `port`, `protocol` |
| `net` | `net.listen` | Create/bind a network listener for inbound connections. | `host`, `port`, `protocol` |
| `net` | `net.send` | Transmit data over an established connection or association. | none in v0.2; see section 8 |
| `net` | `net.receive` | Receive data from an established connection or association. | none in v0.2; see section 8 |
| none (direct `Io`) | `io.read` | Read data from a console or generic stream. | `stream` |
| none (direct `Io`) | `io.write` | Write data to a console or generic stream. | `stream` |
| none (direct `Process`) | `process.exec` | Execute an external process. | `executable` |
| none (direct `Process`) | `process.exit` | Explicitly terminate the current process. | none |
| none (direct `Spawn`) | `spawn.task` | Create structured concurrent work. | `scope` (reserved) |
| none (direct `Spawn`) | `spawn.actor` | Create an actor. | `scope` (reserved) |

The table order is the canonical taxonomy order; `net.send` and `net.receive`
follow `net.listen` and precede generic `io.read`/`io.write`.

The direct `Io` operations have no family. They are still under the `Io`
capability. `Process` and `Spawn` have no families in v0.2; their broad
capability denials and concrete operation denials are sufficient.

Every operation has exactly one parent capability. Adding a family or category
must never change that parent. In particular:

- `fs.*` and `net.*` remain `Io` operations;
- `io.read` and `io.write` remain `Io` operations;
- `process.*` remain `Process` operations;
- `spawn.*` remain `Spawn` operations.

### 3.4 Cross-family categories

v0.2 defines two categories:

| Category | Membership | Meaning |
|---|---|---|
| `io.read` | `fs.read`, `net.receive`, generic `io.read` | Data or content entering the program from an external source. |
| `io.write` | `fs.write`, `net.send`, generic `io.write` | Data or content leaving the program to an external sink. |

Membership is semantic, not an accidental list of current operations. Future
operations classified as ingress of external data belong to `io.read`; future
operations classified as egress of external data belong to `io.write`. The
classification of every new `Io` operation must be complete before the operation
is accepted into the production taxonomy.

The following are deliberately **not** members:

- `fs.remove` is not `io.write`. Deletion is a filesystem namespace mutation,
  not data transmission to an external sink.
- `net.connect` is not `io.read` or `io.write`. Connection establishment is a
  prerequisite for transfer, not transfer itself.
- `net.listen` is not `io.read` or `io.write`. Listener creation is not data
  reception.
- `process.exec`, `process.exit`, `spawn.task` and `spawn.actor` are not
  members. Their parent capabilities are `Process` and `Spawn`.

A category is a denial-side predicate. It does not change the operation's
identity, parent capability, family, resource parameters or runtime behavior.

## 4. Denial target syntax and resolution

### 4.1 Grammar

The proposed `@deny` target grammar is:

```text
deny_target      ::= capability
                   | family
                   | category
                   | operation [ "(" [ parameter_list ] ")" ]

capability       ::= "alloc" | "panic" | "io" | "process"
                   | "spawn" | "nondeterminism"
family           ::= "fs" | "net"
category         ::= "io.read" | "io.write"
operation        ::= "fs.read" | "fs.write" | "fs.remove"
                   | "net.connect" | "net.listen"
                   | "net.send" | "net.receive"
                   | "io.read" | "io.write"
                   | "process.exec" | "process.exit"
                   | "spawn.task" | "spawn.actor"

parameter_list   ::= parameter ( "," parameter )*
parameter        ::= parameter_name ":" parameter_value
```

The grammar is intentionally ambiguous between bare category names and
parameterless generic operations for `io.read`/`io.write`. Section 4.2 resolves
that ambiguity normatively. Parentheses are semantically significant only for
that resolution.

### 4.2 Target resolution

Targets resolve as follows:

1. A bare capability spelling is a broad capability denial and contributes to
   `explicit_denied`/`denied`. It never becomes a specialized clause.
2. `fs` and `net` are family denials. They are specialized clauses, never broad
   capability denials.
3. A bare `io.read` or `io.write` is a **category denial**, because those
   spellings are category names. It covers all members of the category.
4. A parenthesized `io.read(...)` or `io.write(...)` is a **concrete generic
   operation denial**. An empty parameter list, `io.read()` or `io.write()`, is
   a parameterless denial of only the generic operation.
5. Any other dotted operation spelling is a concrete operation denial. It may be
   bare or parenthesized. `@deny(fs.write)` and `@deny(fs.write())` are
   semantically identical; both have an empty canonical parameter list.
6. Family and category denials are parameterless. `@deny(fs(...))`,
   `@deny(net(...))`, `@deny(io.read(...))` with parameters, and
   `@deny(io.write(...))` with parameters are invalid. Resource restrictions
   require a concrete operation target.
7. `@deny(fs.read)`, `@deny(fs.write)`, `@deny(fs.remove)`,
   `@deny(net.connect)`, `@deny(net.listen)`, `@deny(net.send)`,
   `@deny(net.receive)`, `@deny(io.read)`, `@deny(io.write)`,
   `@deny(process.exec)`, `@deny(process.exit)`, `@deny(spawn.task)` and
   `@deny(spawn.actor)` are valid parameterless denials. For `io.read` and
   `io.write`, the bare form resolves to the category per rule 3; the explicit
   generic operation denial is `io.read()`/`io.write()`.

Rule 3 is a deliberate resolution of the name collision: a bare category name
means the broad category, while parentheses mean the concrete generic
operation. Implementations and diagnostics must preserve this distinction in
canonical contracts.

### 4.3 Examples

| Clause | Target | Effect |
|---|---|---|
| `@deny(io)` | Capability `Io` | Denies the entire `Io` capability. |
| `@deny(fs)` | Family `fs` | Denies every filesystem operation, including future `fs` operations. |
| `@deny(net)` | Family `net` | Denies every network operation, including future `net` operations. |
| `@deny(io.read)` | Category `io.read` | Denies `fs.read`, `net.receive` and generic `io.read`. |
| `@deny(io.write)` | Category `io.write` | Denies `fs.write`, `net.send` and generic `io.write`. |
| `@deny(io.read())` | Operation `io.read` | Denies only generic `io.read`, with no resource restriction. |
| `@deny(fs.write)` | Operation `fs.write` | Denies every `fs.write` occurrence. |
| `@deny(fs.write(destination: "x"))` | Operation `fs.write` with resource | Denies `fs.write` only when the destination matches `"x"`; unknown destinations fail closed as potential. |
| `@deny(fs.write(destination: *))` | Operation `fs.write` with wildcard | Denies every `fs.write` occurrence, including unknown destinations. |
| `@deny(net.connect)` | Operation `net.connect` | Denies every connection-establishment occurrence; it does not deny send/receive. |
| `@deny(net.send)` | Operation `net.send` | Denies data transmission; it does not deny connection establishment or reception. |
| `@deny(process.exit)` | Operation `process.exit` | Denies explicit termination; ordinary returns are unaffected. |
| `@deny(spawn.task)` | Operation `spawn.task` | Denies task creation; existing task execution is unaffected. |

## 5. Canonical contract representation

### 5.1 Broad contract fields stay separate

`FunctionCapabilityContract` continues to distinguish:

- `explicit_denied`: broad capabilities written explicitly;
- `denied`: broad capabilities after `@pure` union;
- `specialized_denied`: family, category and operation clauses.

Family and category denials MUST NOT contribute to `explicit_denied` or
`denied`. They are specialized restrictions and are enforced through the
specialized path. Broad enforcement remains authoritative and is returned
first if a broad violation exists.

### 5.2 Specialized target representation

The proposed canonical representation replaces the clause's bare
`EffectOperation` field with an explicit target:

```rust
pub enum EffectFamily {
    Fs,
    Net,
}

pub enum EffectCategory {
    IoRead,
    IoWrite,
}

pub enum SpecializedDenyTarget {
    Family(EffectFamily),
    Category(EffectCategory),
    Operation(EffectOperation),
}

pub struct SpecializedDenyClause {
    pub target: SpecializedDenyTarget,
    pub parameters: Vec<SpecializedDenyParameter>,
    pub span: Span,
}
```

`EffectOperation` gains `NetSend` and `NetReceive`. Family and category targets
are not `EffectOperation` variants and are not `Capability` variants.

Operation metadata must expose at least:

- parent capability;
- optional family (`Fs`, `Net`, or none);
- category membership set (possibly empty);
- canonical name;
- accepted resource parameters;
- canonical taxonomy order.

### 5.3 Canonical ordering and deduplication

Canonical clauses are ordered deterministically:

1. Family targets: `fs`, then `net`.
2. Category targets: `io.read`, then `io.write`.
3. Operation targets: canonical operation order, including `net.send` and
   `net.receive` at their declared positions.
4. Within an operation target, parameters in canonical parameter order
   (`source`, `destination`, `host`, `port`, `protocol`, `executable`,
   `stream`, `scope`).
5. Identical target-plus-parameter clauses are deduplicated, retaining the
   first source span.
6. Family/category clauses are never expanded into operation clauses. A family
   denial and an explicit member-operation denial remain distinct canonical
   entries.

The source span of the original family, category or operation clause is
retained. Diagnostic projection may summarize the target, but canonical
enforcement must not lose the target kind.

## 6. Conservative matching semantics

### 6.1 Broad-first invariant

Broad capability enforcement is evaluated before specialized matching and is
unchanged. If a broad denial is violated, the existing broad
`CapabilityContractViolations` result is returned before any specialized
violation. A family or category denial never masks, replaces or weakens a broad
violation.

### 6.2 Target membership

For a **known operation fact**:

| Denial target | Definite match | Disjoint |
|---|---|---|
| Family `fs` | Operation family is `fs`. | Operation family is `net` or none. |
| Family `net` | Operation family is `net`. | Operation family is `fs` or none. |
| Category `io.read` | Operation is `fs.read`, `net.receive` or generic `io.read`. | Any other operation. |
| Category `io.write` | Operation is `fs.write`, `net.send` or generic `io.write`. | Any other operation. |
| Operation `O` | Fact operation identity is exactly `O`. | Any other operation identity. |

Resource parameters are evaluated only for operation targets:

- no parameters: every occurrence of the operation matches definitely;
- wildcard parameter: matches known, unknown or missing resource values;
- exact parameter: equal known value is definite, unequal known value is
  disjoint, and unknown/missing value is potential;
- duplicate or wrong-kind observed metadata is potential, never disjoint.

Family and category denials are parameterless. They match a known member
definitely regardless of resource knowledge.

### 6.3 Unclassified and missing metadata

An unclassified operation fact has a known broad capability but no concrete
operation identity. It is never treated as safe merely because no concrete
operation clause exists.

| Fact state | Family/category denial | Operation denial |
|---|---|---|
| Parent capability matches; family/category membership unknown | Potential. | Potential. |
| Parent capability differs | Disjoint. | Disjoint. |
| Parent capability matches; family is known and non-member | Disjoint for that family. | Potential for an operation under that parent until identity is known. |
| Parent capability matches; category membership is known and non-member | Disjoint for that category. | Potential for an operation under that parent until identity is known. |
| Parent capability matches; category membership is known and member | Definite. | Potential until concrete operation identity is known. |

A known operation with missing family or category metadata is a compiler
invariant error, not `Disjoint`. A compiler must not silently treat incomplete
operation metadata as evidence of safety. An unclassified fact may optionally
carry a known family and category membership; if either is absent, the
conservative potential rules above apply.

Missing operation metadata is therefore fail-closed: it can cause a potential
violation, but it can never make a denial conclusively disjoint or turn unknown
metadata into evidence of safety.

### 6.4 Future operations

If a future taxonomy revision adds an operation to `fs` or `net`, an existing
`@deny(fs)` or `@deny(net)` clause must match it without the clause being
rewritten. This follows from family membership being stored as a predicate.

If a future operation is classified as an `io.read` or `io.write` category
member, the corresponding category denial must match it. Every future `Io`
operation must declare its family and category membership (including explicit
empty membership) before production use. Missing membership is an invariant
error, not a bypass.

Family/category clauses MUST NOT be lowered to the currently known operation
set at parse time, canonicalization time, cache-write time or analysis time.

### 6.5 Multiple matching clauses

Clauses are independent. A single fact may match a family clause and an explicit
operation clause; both are retained and may produce separate violations. The
existing broad-first ordering, per-clause fact ordering, classification
tie-break and duplicate-removal rules remain the basis for deterministic output.

## 7. Matching matrix

The following table is normative for known operation facts. `D` means definite,
`P` potential, `X` disjoint.

| Observed operation | `@deny(fs)` | `@deny(net)` | `@deny(io.read)` | `@deny(io.write)` | `@deny(fs.write)` | `@deny(io.read())` |
|---|---|---|---|---|---|---|
| `fs.read` | D | X | D | X | X | X |
| `fs.write` | D | X | X | D | D | X |
| `fs.remove` | D | X | X | X | X | X |
| `net.connect` | X | D | X | X | X | X |
| `net.listen` | X | D | X | X | X | X |
| `net.send` | X | D | X | D | X | X |
| `net.receive` | X | D | D | X | X | X |
| generic `io.read` | X | X | D | X | X | D |
| generic `io.write` | X | X | X | D | X | X |
| `process.exec` | X | X | X | X | X | X |
| `process.exit` | X | X | X | X | X | X |
| `spawn.task` | X | X | X | X | X | X |
| `spawn.actor` | X | X | X | X | X | X |
| unclassified `Io` | P | P | P | P | P | P |
| unclassified `Process` | X | X | X | X | X | X |
| unclassified `Spawn` | X | X | X | X | X | X |

`@deny(fs.write(destination: "x"))` follows the operation row for `fs.write`,
then applies the resource rules: exact equal known value is `D`, unequal known
value is `X`, and unknown or missing destination is `P`. `@deny(fs.write)` has
no resource restriction and is `D` for every `fs.write` fact.

The unclassified row deliberately over-approximates under `Io`: an unknown
`Io` operation might be a member of any `Io` family or category. This is
fail-closed, not a claim that the unknown operation actually belongs there.

## 8. Network semantics

### 8.1 Distinct network operations

v0.2 distinguishes four network concerns:

| Operation | Concern | Not included |
|---|---|---|
| `net.connect` | Establish an outbound connection to an endpoint. | Sending or receiving application data. |
| `net.listen` | Create/bind a listener for inbound connections. | Accepting a connection, reading data, or sending data. |
| `net.send` | Transmit data over an already-established connection or association. | Establishing the connection or receiving data. |
| `net.receive` | Receive data from an already-established connection or association. | Establishing the connection or sending data. |

`net.connect` and `net.listen` are not members of `io.read` or `io.write`.
`net.send` is a member of `io.write`. `net.receive` is a member of `io.read`.

The current v0.1 wording that `net.listen` "accepts inbound connections" is
narrowed by v0.2: listener creation is `net.listen`; acceptance and
per-connection lifecycle are not silently folded into it. There is currently no
dedicated operation for accepting an inbound connection. That remains a
coverage gap for a later taxonomy revision.

### 8.2 Endpoint and connection identity

`net.connect` and `net.listen` retain the existing `host`, `port` and
`protocol` resource parameters. These describe the endpoint named by the
operation; they do not establish a general connection identity.

`net.send` and `net.receive` have **no resource parameters in v0.2**. In
particular, they do not accept `host`, `port`, `protocol` or `stream` as a
substitute for a connection identity. Only parameterless denials, written
`@deny(net.send)` or `@deny(net.send())` and `@deny(net.receive)` or
`@deny(net.receive())`, are valid in v0.2.

This is deliberate. Endpoint-specific restrictions on already-open connections
require an explicit endpoint/connection identity model. v0.2 does not define
that model and makes no claim that:

- denying `net.connect` for one endpoint denies send/receive on an already-open
  connection to that endpoint;
- denying `net.send(host: ...)` or `net.receive(host: ...)` is valid;
- a host/port pair is a unique connection identity;
- connection reuse, accepted sockets, proxying, NAT, TLS sessions or aliases are
  tracked;
- a listener denial covers data operations on connections accepted by it.

A future endpoint/connection model must define connection handle identity,
creation and destruction, aliasing, accept semantics, listener association,
proxying and data-transfer attribution before resource-aware send/receive
clauses can be specified.

### 8.3 Production availability

No production native networking APIs are added by this specification.
`net.connect`, `net.listen`, `net.send` and `net.receive` remain taxonomy and
contract vocabulary until a later native-binding milestone. A clause for an
operation with no reachable production fact records a restriction but has
nothing to match. It is not authorization and is not proof that the operation
is unavailable or safe.

## 9. Process and spawn semantics

### 9.1 Process

`process` remains a fundamental capability. Its two operations are distinct:

- `process.exec` concerns execution of an external process. It is not a generic
  marker for all `Process` effects.
- `process.exit` concerns explicit process termination. It is not generated by
  an ordinary function return, including a return from the entry point.

`@deny(process)` denies both. Parameterless `@deny(process.exec)` and
`@deny(process.exit)` deny every occurrence of the identified operation. The
existing implementation limitation that `process.exit` is HIR-only and has no
stable TIR mapping remains in force until a later implementation milestone.

### 9.2 Spawn

`spawn` remains a fundamental capability. Its two operations are distinct:

- `spawn.task` concerns creation of structured concurrent work.
- `spawn.actor` concerns creation of an actor.

Scheduling, awaiting, executing or completing an existing task does not
automatically constitute `spawn.task`. Sending or receiving an actor message
does not automatically constitute `spawn.actor`. Only the creation primitive
produces the corresponding operation fact.

The `scope` resource parameter remains reserved. It may be parsed for source
compatibility where the current grammar permits it, but it has no normative
matching semantics until the actor model defines scope. An exact scope clause
must not be treated as an enforced broad guarantee while the model is absent.

## 10. Enforcement and trust boundaries

v0.2 changes only compile-time vocabulary and matching. It preserves all current
limits:

- no runtime specialized-effect guard;
- no filesystem sandbox, path canonicalization, symlink/hard-link policy or TOCTOU
  guarantee;
- no network isolation, connection tracking or packet attribution;
- no native-code sandbox or resolver authentication;
- no FFI redesign;
- no new native networking APIs;
- no actor implementation;
- no optimization;
- no change to the seven capabilities or their bits;
- no change to the trusted local bytecode-cache model.

Family and category denials are not authority. They are static restrictions over
compiler-classified facts. A program that passes enforcement still executes
under the host process's operating-system permissions and the trusted native
implementation's actual behavior.

Effect inspection and structured diagnostics retain the current v0.1
limitation: specialized violations surface as generic compiler errors, not a
specialized E6 schema. v0.2 does not require an E6 schema change; a future
diagnostic projection must not silently invent one.

## 11. Implementation delta

The following changes are required in later implementation milestones. None is
made by this specification.

### 11.1 Parser

- Recognize `fs` and `net` as family denial targets.
- Recognize bare operation targets for every operation, not only operations with
  no declared parameters.
- Recognize `net.send` and `net.receive`.
- Resolve bare `io.read`/`io.write` to category targets and parenthesized forms
  to generic operation targets.
- Reject parameters on family and category targets.
- Reject resource parameters on `net.send`/`net.receive`.
- Preserve the existing `@pure` handling and broad capability grammar.
- Retain source spans for family, category and operation clauses.

### 11.2 AST

- Add a specialized denial target distinct from `DenyCapability`.
- Add family and category syntax nodes or target kinds.
- Add `NetSend` and `NetReceive` operation spellings.
- Preserve the distinction between bare category and parenthesized generic
  operation forms in AST/canonical lowering.
- Keep AST free of analyzer matching policy and capability bits.

### 11.3 Driver and canonical contracts

- Replace the clause's bare operation field with a canonical target enum.
- Keep broad `explicit_denied` and `denied` fields unchanged.
- Implement canonical target ordering, parameter ordering, deduplication and
  span retention.
- Never expand family/category clauses into concrete operation clauses.
- Preserve family/category clauses in `FunctionCapabilityContract` for tooling
  and enforcement.
- Update contract equality/binding collision behavior without weakening
  duplicate-binding detection.

### 11.4 Effects taxonomy and analysis

- Add `EffectFamily` and `EffectCategory`.
- Add `NetSend` and `NetReceive` to `EffectOperation::ALL` with defined canonical
  order, parent capability, family, category membership and parameter keys.
- Add family/category metadata to every `Io` operation, including explicit empty
  membership for direct `io.read`/`io.write`.
- Keep `CapabilitySet` and the seven capabilities unchanged.
- Extend unclassified facts only if needed to carry optional known family or
  category membership. Missing metadata must remain fail-closed.
- Preserve direct/transitive propagation, deterministic call graphs, witnesses,
  resource knowledge and fact origin.
- Add tests for future-family membership without fixed expansion.

### 11.5 Matcher

- Implement target membership predicates for family, category and operation.
- Preserve broad-first enforcement.
- Preserve exact/wildcard/unknown/missing resource matching.
- Add conservative potential matching for unclassified `Io` facts against
  family/category targets.
- Treat missing family/category metadata on a known operation as a compiler
  invariant error, never `Disjoint`.
- Preserve deterministic violation ordering and duplicate removal.
- Add a regression matrix covering every row in section 7.

### 11.6 Native classifier

- Add taxonomy metadata for `net.send` and `net.receive`.
- Do not add native networking APIs or FFI bindings.
- Keep the native registry and classifier as separate compiler-owned tables.
- Ensure any future `std.net.*` binding declares its operation, family and
  category membership before it becomes reachable.
- Preserve exact native registry/classifier/signature consistency tests.

### 11.7 Diagnostics and tooling

- Render family and category targets without pretending they are capabilities.
- Preserve the generic specialized compiler error until a later structured
  diagnostic milestone.
- Do not add E6 schema fields in the initial implementation.
- Update effect inspection tests only when the normative target representation
  is implemented.

## 12. Semantic revision and cache compatibility

The current compiler uses `semantics-6`. This specification does not change
`SEMANTIC_REVISION`, the cache fingerprint or any cache artifact.

The future implementation MUST bump the semantic revision before release
(expected `semantics-7`). The reason is not merely new syntax:

- the taxonomy adds two operations;
- denial targets gain family and category meanings;
- `io.read`/`io.write` gain category semantics for bare target forms;
- the matcher gains new membership predicates and future-operation behavior;
- an older compiler's enforcement result must not be trusted for the new
  vocabulary.

The conservative cache policy treats a compatible hit as evidence of a prior
successful enforced compilation by the same build and semantics. A
`semantics-6` artifact cannot certify v0.2 family/category enforcement even if
the particular source form happened to be rejected by the older parser. The
implementation must therefore:

- raise the cache fingerprint to the new semantic revision;
- treat all older artifacts as incompatible misses;
- require a fresh compile and fresh enforcement before writing a new artifact;
- leave the bytecode format independent unless a separate format change is
  approved;
- never serialize family/category metadata into the cache;
- never reconstruct specialized facts from a cache hit;
- preserve the rule that a failed compilation does not create or replace a
  valid artifact.

This is a deliberate compatibility change for the later implementation. It is
not made by this document.

## 13. Acceptance criteria

An implementation is not complete until all of the following hold.

### 13.1 Capability and representation invariants

- The seven fundamental capabilities and `CapabilitySet` bits are unchanged.
- `@deny(fs)`/`@deny(net)` do not set the `Io` bit and do not behave like
  `@deny(io)`.
- Family/category clauses are represented distinctly from capabilities and
  concrete operations.
- Canonicalization never expands a family/category clause into current
  operations.
- Canonical ordering, deduplication and source-span retention are deterministic.

### 13.2 Syntax and resolution

- `@deny(fs)`, `@deny(net)`, `@deny(io.read)` and `@deny(io.write)` parse as
  family/category targets.
- Bare operation forms for all non-category-named operations parse and
  canonicalize to parameterless operation clauses.
- `@deny(fs.write)` and `@deny(fs.write())` canonicalize equivalently.
- `@deny(io.read)`/`@deny(io.write)` resolve to the categories, while
  `@deny(io.read())`/`@deny(io.write())` resolve to the generic operations.
- Parameters on family/category targets are rejected.
- Resource parameters on `net.send`/`net.receive` are rejected in v0.2.
- `@deny(net.send)` and `@deny(net.receive)` parse and enforce their operation
  identity.

### 13.3 Matching

- The complete section 7 matrix passes for known operations.
- `@deny(fs)` matches `fs.read`, `fs.write`, `fs.remove` and future `fs`
  operations.
- `@deny(net)` matches all four network operations and future `net` operations.
- `@deny(io.read)` matches `fs.read`, `net.receive` and generic `io.read`, but
  not `fs.remove`, `net.connect`, `net.listen` or `net.send`.
- `@deny(io.write)` matches `fs.write`, `net.send` and generic `io.write`, but
  not `fs.remove`, `net.connect`, `net.listen` or `net.receive`.
- Exact, wildcard, unknown and missing resource behavior is unchanged for
  operation clauses.
- Unclassified `Io` facts are potential matches for all `Io` family/category
  clauses.
- Missing family/category metadata cannot produce `Disjoint`.
- Broad violations still take precedence over specialized violations.
- Violation ordering and duplicate removal remain deterministic.

### 13.4 Process, spawn and network boundaries

- `process.exit` is not generated by ordinary returns.
- `process.exec` remains distinct from `process.exit`.
- `spawn.task`/`spawn.actor` are generated only by creation primitives, not task
  execution or actor messaging.
- `net.connect`, `net.listen`, `net.send` and `net.receive` remain distinct.
- No claim is made that connect/listen endpoint restrictions cover send/receive
  on existing connections.
- `scope` remains reserved and does not become an enforced actor resource.

### 13.5 Revision and trust

- `SEMANTIC_REVISION` is bumped and old artifacts are incompatible misses.
- Cache hits remain bytecode-only and do not reconstruct facts.
- No runtime guard, native sandbox, FFI redesign, new native networking API,
  actor implementation or optimization is introduced.
- Existing v0.1 specialized guarantees and limits remain documented and tested.

## 14. Regression matrix for implementation

The future implementation must include at least the following cases. The
"Expected" column is the enforcement result in addition to the current v0.1
baseline.

| Case | Clause | Observed fact | Expected |
|---|---|---|---|
| Family direct read | `@deny(fs)` | `fs.read` | Definite violation |
| Family write | `@deny(fs)` | `fs.write` | Definite violation |
| Family remove | `@deny(fs)` | `fs.remove` | Definite violation |
| Family excludes net | `@deny(fs)` | `net.connect` | Disjoint |
| Family future operation | `@deny(fs)` | synthetic future `fs` operation | Definite violation |
| Net family | `@deny(net)` | `net.send` | Definite violation |
| Net family excludes fs | `@deny(net)` | `fs.write` | Disjoint |
| Read category fs | `@deny(io.read)` | `fs.read` | Definite violation |
| Read category net | `@deny(io.read)` | `net.receive` | Definite violation |
| Read category generic | `@deny(io.read)` | generic `io.read` | Definite violation |
| Read category excludes remove | `@deny(io.read)` | `fs.remove` | Disjoint |
| Read category excludes connect | `@deny(io.read)` | `net.connect` | Disjoint |
| Read category excludes listen | `@deny(io.read)` | `net.listen` | Disjoint |
| Write category fs | `@deny(io.write)` | `fs.write` | Definite violation |
| Write category net | `@deny(io.write)` | `net.send` | Definite violation |
| Write category generic | `@deny(io.write)` | generic `io.write` | Definite violation |
| Write category excludes remove | `@deny(io.write)` | `fs.remove` | Disjoint |
| Generic-only read | `@deny(io.read())` | `fs.read` | Disjoint |
| Generic-only read | `@deny(io.read())` | generic `io.read` | Definite violation |
| Generic-only write | `@deny(io.write())` | `net.send` | Disjoint |
| Operation parameterless | `@deny(fs.write)` | `fs.write` unknown destination | Definite violation |
| Operation exact | `@deny(fs.write(destination: "x"))` | `fs.write` exact `"x"` | Definite violation |
| Operation exact mismatch | `@deny(fs.write(destination: "x"))` | `fs.write` exact `"y"` | Disjoint |
| Operation exact unknown | `@deny(fs.write(destination: "x"))` | `fs.write` unknown destination | Potential violation |
| Operation wildcard | `@deny(fs.write(destination: *))` | `fs.write` unknown destination | Definite violation |
| Unclassified Io family | `@deny(fs)` | unclassified `Io` | Potential violation |
| Unclassified Io read category | `@deny(io.read)` | unclassified `Io` | Potential violation |
| Unclassified process | `@deny(fs)` | unclassified `Process` | Disjoint |
| Missing family metadata | `@deny(fs)` | known `Io` operation with missing family | Compiler invariant error |
| Broad precedence | `@deny(io, fs.write)` | `Io` requirement | Broad violation first |
| Process exit | `@deny(process.exit)` | explicit exit | Definite violation |
| Return is not exit | `@deny(process.exit)` | ordinary return | Disjoint |
| Spawn creation | `@deny(spawn.task)` | task creation | Definite violation |
| Task execution not spawn | `@deny(spawn.task)` | await/execute existing task | Disjoint |
| Actor messaging not spawn | `@deny(spawn.actor)` | actor message | Disjoint |
| Network separation | `@deny(net.connect)` | `net.send`/`net.receive` | Disjoint |
| Send/receive resource syntax | `@deny(net.send(host: "x"))` | n/a | Parse error in v0.2 |
| Cache revision | new syntax | old `semantics-6` artifact | Incompatible miss |

The future test suite should also prove deterministic ordering across family,
category and operation clauses, and that no family/category clause is expanded
into a fixed operation list.

## 15. Remaining semantic ambiguities

These points are recorded so implementation does not silently choose an
unstated rule.

1. **`io.read`/`io.write` name overload.** v0.2 normatively resolves bare names
   as categories and parenthesized names as generic operations. This is usable
   but inherently subtle. If the language later gains an explicit category
   spelling or a distinct generic-operation spelling, this rule should be
   revisited before stabilization.
2. **Send/receive resources.** No connection identity exists in v0.2, so
   `net.send`/`net.receive` have no resource parameters. A future connection
   model must decide whether resource restrictions are per connection, per
   endpoint, per listener or per data-flow, and must define aliasing and
   accepted-socket identity.
3. **Accepting inbound connections.** `net.listen` covers listener creation, not
   accept. There is no dedicated `net.accept` operation. Whether acceptance is a
   distinct effect or part of receive remains open.
4. **Unclassified `Io` over-approximation.** A family denial treats an
   unclassified `Io` fact as potential even when the actual operation may be
   outside the family. This is intentionally conservative. A future classifier
   may attach family/category hints to unclassified facts to reduce false
   positives without changing the fail-closed default.
5. **Family/category resource restrictions.** v0.2 allows resource parameters
   only on concrete operations. Family-wide restrictions such as
   `fs(destination: "...")` or `net(host: "...")` are not specified. If needed,
   they require a separate resource-predicate design and endpoint model.
6. **Scope.** `spawn.task(scope: ...)` and `spawn.actor(scope: ...)` remain
   syntactically reserved but semantically undefined until the actor model
   specifies scope.
7. **Diagnostics.** Family/category targets need canonical human and JSON
   representation. This specification preserves the current generic compiler
   error and does not define E6 schema fields.
8. **Future operation completeness.** The future implementation must enforce
   complete family/category metadata for every known `Io` operation. The
   process by which a new operation is reviewed and admitted to a family or
   category must be part of that milestone's acceptance criteria.
9. **Family/category interaction with broad `@pure`.** `@pure` continues to
   imply the existing broad restrictions. A family or category clause does not
   add a capability bit and cannot be used to selectively relax `@pure`.
10. **Cache revision policy.** Section 12 requires bumping the semantic revision
    for the whole v0.2 implementation rather than trying to prove that every new
    source form was previously rejected. K4.9E must implement that conservative
    choice and document the resulting incompatibility.

## 16. Implementation sequence

The roadmap sequence is:

```text
K4.8   family/operation semantics specification       DONE (this document)
K4.9A  taxonomy metadata and parser/AST targets       PLANNED
K4.9B  canonical contracts and driver binding         PLANNED
K4.9C  effects analysis and classifier metadata       PLANNED
K4.9D  family/category matcher and enforcement        PLANNED
K4.9E  semantic revision, cache migration, diagnostics PLANNED
K4.9   family/operation semantics implementation      PLANNED
K4.10  final effects gate and coverage ledger         PLANNED
```

Ordering constraints:

- K4.9A must define the complete taxonomy and syntax before canonical contracts
  can be stable.
- K4.9B must preserve family/category targets without expansion before the
  matcher can be tested.
- K4.9C must provide total operation metadata before family/category matching can
  claim future-operation safety.
- K4.9D must implement the section 7 matrix and conservative unknown rules.
- K4.9E must bump the semantic revision before any artifact from the new compiler
  is trusted.
- K4.10 must publish the full per-operation enforceable/deferred coverage ledger
  and audit the final effects gate. It must not count unimplemented network
  APIs, actor semantics, endpoint tracking or runtime guards as passing.

## 17. References

- [Specialized Effects v0.1](specialized-effects-v0.1.md) — current implemented
  behavior and trust limits.
- [E1 Capability Analysis](../E1_CAPABILITY_ANALYSIS.md) — existing analysis and
  enforcement architecture.
- [Architecture](../architecture/ARCHITECTURE.md) — pipeline and backend
  boundaries.
- [Bytecode cache](../BYTECODE_CACHE.md) — trusted-cache model.
- [Roadmap](../roadmap.md) — K4 milestone sequence.
