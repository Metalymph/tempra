# Tempra Roadmap

This document defines Tempra's implementation order, dependency structure and
acceptance gates. Phase-numbered prototype documents record completed
experiments, but they do not override this roadmap or the current language
specifications.

The normative language, comptime, capability/effect, TIR, cache and
specialized-effect specifications remain authoritative. This roadmap sequences
work and states what must be proven before a milestone is complete. It does not
define new source syntax and does not convert a scaffold into an implemented
feature.

## Scripting-first execution order (decision overlay, 2026-11-07)

This section governs **forward-looking product sequencing** while retaining
the historical E/K/R/M/C/A milestone records below. Historical milestones
remain referenced by their original canonical IDs. See
[milestone migration](roadmap/migration.md), [status](roadmap/status.md)
and [Tempra Script plan](roadmap/script.md).

**Product gates:** S1 **Tempra Script scripting achievement** (stack VM runtime,
language, stdlib, CLI, cache, distribution foundation) → S2 **Astro Tone
website/documentation** → S3 **formatter/linter/LSP/MCP and agent support**
→ S4 **Tempra Script v1.0-alpha/beta**. S2 and S3 may overlap S1, but S3 and the
[release gates](roadmap/release-gates.md) **must** pass before alpha.
Comparisons include **Python, JavaScript and TypeScript**, using pinned
CPython/Node.js/Bun versions and transparent TypeScript build accounting.

**After scripting maturity:** LLVM-backed Tempra Native AOT, then self-hosting.
The historical Cranelift JIT is designated for complete code removal; it is not part of either target distribution.
No embedded Wasm engine is part of the Tempra Script deliverable. The historical
default HIR interpreter remains the actual current path until stack VM
parity justifies promotion; future comptime authority changes also require
an explicit parity gate.

**Implementation closure:** R1–R3 Result/provenance changes were merged with a
user-reported green workspace gate for the supported scope.
This is **not** the closure of canonical R4/R5 or a formal verification
claim. The first draft obligation is R.PROOF. The phrase "K4 Result"
used in discussions maps to canonical R;
canonical K4 continues to denote the specialized-effects track.

**Proofs:** add meaningful `<canonical milestone>.PROOF` sub-milestones
with explicit properties, scopes, assumptions, proof artifacts and Rust
correspondence. See [proof policy](roadmap/formal-verification.md) and
`formal/manifest.toml`. Proofs progress alongside the product but do not
substitute for tooling, scripting or distribution gates.

**Immediate technical milestone:** canonical **K5 Phase Timings and Latency
Baseline**, followed by K6 regression budgets. K5 is empirical measurement,
not a mathematical proof; see [K5 protocol](roadmap/k5.md).
## PRE-AOT integration decision (2026-11-10)

The forward **single non-AOT stack VM** decision, Script 2.0, shared
REPL/Shell profile, TIR Graph and measured external-dependency audit are
specified in [the integrated PRE-AOT roadmap](roadmap/pre-aot.md).
Detailed gates: [VM/comptime migration](roadmap/unified-vm.md),
[Script 2.0](roadmap/script-2.md), [REPL/Shell](roadmap/repl-shell.md),
[TIR Graph](roadmap/tir-graph.md), and
[dependency reduction](roadmap/dependency-reduction.md).

**Identity clarification:** canonical K5.1 is the benchmark protocol and K5.2
is compiler phase timing; neither is Result Hardening (canonical **R**).
R4/R5 and `R.PROOF` remain independent open work. Preserve K5/K6 and M/M5
historical IDs. M5 gates actor-concurrency implementation; O is optional
optimization rather than a safety substitute.

**Forward target supersedes historical backend planning:** no JIT, LLVM as the
first Tempra Native native codegen, and LLVM/Wasm evaluation later. Historical
Cranelift AOT or dedicated Wasm proposals below remain recorded history,
not the active product decision. The old default HIR interpreter and TIR interpreter remain present and may only
be retired after documented VM parity, comptime-staging and regression gates.
The JIT is removed independently of those gates. `@deny`/`@pure`, Result,
ownership and deterministic diagnostics must never be bypassed during migration.

S1–S4 product gates still apply: formatter/linter/LSP/MCP must be usable
before alpha. Full PRE-AOT Tempra Script distribution additionally requires the
specified unified VM, `comptime` parity, Shell profile/job-control scope,
TIR Graph, stdlib, portability and measured startup/footprint. S2 Astro Tone
site and AI packs include reproducible Python/JavaScript/TypeScript examples.

## Definitive distribution decision (2026-11-16)

- **Tempra Script:** verified stack-bytecode VM, scripting-first toolchain,
  planned default after parity and release gates.
- **Tempra Native:** LLVM AOT, future native distribution after Script maturity.
- **One language:** identical source syntax, effects, Memory & Ownership v0.4,
  controlled errors and accepted behavior; no dialect split.
- **Legacy engines:** historical HIR remains a bounded parity/comptime reference;
  JIT and Cranelift are designated for full code removal, not backend promotion.

The phase logs below may retain explicit historical JIT names. They describe
experiments, **not** a supported future runtime or current safety certification.
Naming alone does not change the bytecode trust revision or source semantics.

## Current implementation baseline

Tempra has a working lexer, parser, AST-to-HIR lowering, typechecker, HIR
interpreter, module graph, native standard-library bridge, and language tooling.
TIR, TIR verification, bytecode lowering and the bytecode VM exist with a
smaller supported language subset. The historical Cranelift JIT is retired;
it is not a target runtime. MIR,
general-purpose optimization, native AOT, and dedicated Wasm emission remain
incomplete scaffolds.

## Status vocabulary

- **Implemented** — available on the documented supported surface, with its
  current limitations.
- **Experimental** — usable on a bounded subset; not a production guarantee.
- **In progress** — implementation is active and the milestone gate has not
  passed.
- **Planned** — designed direction or sequence, not implemented.
- **Research** — investigation with no committed language or runtime guarantee.
- **Deferred / conditional** — not active until an explicit trigger exists.

Scaffold crates, taxonomy entries and syntax-only operation names do not count
as implementation evidence.

## Dependency model

Tempra separates semantic prerequisites from optimization, backend and research
work. The graph is need-based: a workstream may begin when its actual
prerequisites exist, without waiting for unrelated milestones to finish.

M5 is the mandatory safety gate before actor concurrency implementation.
Other milestones follow their explicitly documented technical prerequisites.
Actor concurrency may not claim safe task, channel or actor boundaries
until M5 has passed. Specification and design work for C may proceed earlier.

```text
Semantic prerequisites
E (implemented for the current surface)
  → R
  → M
  → M5 memory stabilization gate
  → C
  → V

Advanced memory optimization
M ownership facts
  → O
  O1/O2 preparatory work may begin during M
  O is not a prerequisite for C or A

Native AOT
K/TIR stability
  → A1 native ABI
M ownership facts
  → A2 ownership-aware code generation
A1 + A2
  → A3 LLVM-based AOT (Tempra Native, after Script maturity)
  → A4 linking
  → A5 standalone executables
  → A6 backend parity
  → A7 performance and binary size

Compiler performance
K3 (in progress), K5 and K6 (planned)
  → P
P may overlap A and O once its measurements exist.

Research
M/O analyses
  → NR standalone RC-free research
NR always retains fallback ARC and never blocks baseline native AOT.

Conditional
U
  → only after a concrete source-level unsafe primitive exists
```

The following constraints are explicit:

- R precedes M because ownership rules depend on Result/value-use semantics.
- M establishes memory correctness and stable ownership semantics.
- C implementation depends on M5; C does not depend on O, NR, A or P.
- O is advanced optimization, not a prerequisite for correctness, C or A.
- A may progress incrementally. A1 can begin once K/TIR semantics are stable
  enough for its ABI, and A2 waits only for the M ownership facts it consumes.
- A does not wait for O, NR, P or V to finish. V gates only claims that AOT
  provides verified execution.
- NR and A are independent. RC-free research must never delay baseline AOT.
- P does not gate A and does not change language semantics.
- Wasm/AOT parity is a future backend obligation. It is not a current K3 claim
  beyond the executable HIR, TIR, VM and JIT surfaces.

## Milestone status at a glance

| Identity | Milestone | Status | Type |
| --- | --- | --- | --- |
| E1–E6 | Capability/effect semantic core and tooling | Implemented for the current surface | Semantic/tooling history |
| K1, K2 | Comptime consistency and deterministic compilation | Implemented | Compiler history |
| K3 | Runtime parity | **IN PROGRESS** | Compiler |
| K4 | Specialized-effect track | Implemented | Compiler history |
| K5 | Phase timings and latency baseline | Planned | Compiler responsiveness |
| K6 | Regression budgets and responsiveness gate | Planned | Compiler responsiveness |
| R | Result / value-use hardening | R1–R3 bounded gate reported; R4/R5 open | Semantic prerequisite |
| M | Memory model, acyclic ownership and stabilization | v0.4 specified; implementation/M5 gate open | Semantic prerequisite |
| C | Pluggable actor concurrency | Planned; blocked by M5 | Semantic prerequisite |
| V | Verified functions (`@verified`) | Planned | Semantic proof |
| A | Native AOT | Planned; existing scaffold only | First-class backend milestone |
| O | Advanced memory optimization | Planned | Optimization |
| NR | RC-free research | Research | Standalone research track |
| P | Compiler performance | Planned | Optimization |
| U | Unsafe boundary | Deferred / conditional | Conditional safety boundary |

## Completed and active compiler history

### E — Capability / Effect Semantic Core (E1–E4 DONE)

The E1–E4 semantic core and E6 tooling projection are complete for the currently
meaningful surface. The compiler computes stable per-function semantic summaries
over verified TIR, propagates requirements transitively through calls, and
exposes those facts without a second analysis. This does not close the existing
TIR coverage gaps.

The public contract surface remains deliberately small:

- `@deny(...)`
- `@pure`

The internal initial vocabulary is `Alloc`, `Panic`, `IO`, `Process`, `Spawn`,
`Nondeterminism`, and `Unsafe`. Public denial names exclude `unsafe`. `Unsafe` is
reserved with no current E1 origins; the future `@unsafe` boundary belongs to
conditional milestone U, not the immediate Phase E sequence.

Phase E is subdivided as follows:

1. **E1 — Internal capability model and TIR analysis (implemented).**
   `tempra-effects` provides pure analysis over `TirModule`, direct capability
   discovery, a reusable call graph, fixed-point propagation, deterministic
   provenance, and hand-built TIR fixture tests. It adds no public syntax.
2. **E2 — Minimal pipeline integration (implemented).** Fresh source and
   multi-module VM compilations verify TIR, analyze it once, and retain TIR,
   capability facts and verified bytecode in `VmCompilation`. Cache-hit
   execution, TIR emission, TIR interpretation and HIR paths remain
   analysis-independent. E2 itself added no enforcement, UI, attributes, CLI
   output or source-span expansion; E3 below defines the current contract
   behavior.
3. **E3 — `@deny(...)` (implemented).** E3.1 acquires nonempty typed contracts,
   stable compilation-local TIR function binding and declaration/attribute
   provenance. E3.2 enforces denied transitive requirements, retains structured
   violations/witnesses, rejects annotated execution on non-capability-aware
   paths and invalidates pre-enforcement cache artifacts through the semantic
   fingerprint.
4. **E4 — `@pure` (implemented).** The single convenience contract normalizes to
   denying `io`, `process`, `spawn`, and `nondeterminism` in the driver. Explicit
   deny is unioned; pure intent retains its own source span. E3 enforcement and
   cache semantic revision are unchanged; Alloc and Panic remain allowed by pure
   alone.
5. **E6 — Structured capability diagnostics and inspect/query surface
   (implemented).** `tempra effects [--json] <path>` compiles fresh and projects
   `CapabilityAnalysis`, contracts, violations, witnesses, TIR identities and
   current provenance for humans and tools. `TirFunctionId` remains
   authoritative; names are display-only. The current JSON schema is
   deterministic but not promised as a permanent protocol. E6 does not implement
   LSP integration or the future TIR Graph.

E1 is TIR-only. HIR-only operations are coverage gaps for later integration, not
justification for a second analyzer.

The implemented E1–E6 design and boundaries are recorded in
[E1 Capability Analysis](E1_CAPABILITY_ANALYSIS.md). The historical E5 audit
found no intentional source-level primitive requiring unsafe authorization. It
recorded narrow correctness debts in native signature integrity, comptime extern
handling, and internal JIT pointer/ABI/lifetime assumptions, not new semantic
phases or demonstrated exploits; K1.4 has since resolved the bodyless-dispatch
portion of the comptime debt. `check` remains frontend/typechecking-only, not
capability certification.

### K — Comptime Consistency & Compiler Responsiveness

K is the active compiler-consistency and responsiveness track. It must establish
one canonical, backend-independent comptime semantics: VM, JIT, Wasm and future
AOT must observe the same comptime result or error. The K1 audit/spec selected
the existing HIR interpreter as the semantic kernel. K1.1 adds one canonical
facade that owns comptime mode/fuel setup, value/error/control-flow
normalization and state restoration. K1.2 extends the same authority to ordinary
multi-module and file-based typechecking through a module-aware lookup adapter
over separate HIR files. K1.3 makes supported integer arithmetic in the shared
HIR kernel panic-free and profile-independent without changing float semantics.
The K1.4 shared call dispatch rejects functions without executable HIR bodies
rather than producing synthetic `Unit`; comptime still does not execute
VM-native externs. K1.5 makes the HIR interpreter the sole effect-policy
authority for currently executable comptime operations: local `cmd`
construction and allocation remain allowed, while process execution,
output/input, filesystem, environment, host-OS access, `dbg` and `exit` are
rejected at their effect boundaries. The typechecker remains the owner of
primitive HIR materialization, and TIR/backends consume resolved code. K1.6 adds
deterministic evaluator-progress fuel ticks and a conservative compiler-defined
v0.1 call-depth safety limit of 8 active Tempra frames. The limit is
revisionable as the evaluator evolves; nested comptime shares the active session
without resetting its budget.

Current status:

```text
K1 audit/spec       DONE
K1.1 facade         DONE
K1.2 multi-module   DONE
K1.3 arithmetic     DONE
K1.4 extern/bodyless DONE
K1.5 effect policy   DONE
K1.6 termination     DONE
K1.7 cache revision  DONE
K1 implementation   DONE
K2.1 determinism audit DONE
K2.2 specialization ordering DONE
K2.3 stdlib discovery input DONE
K2.4 semantic/diagnostic iteration DONE
K2.5 cross-process reproducibility gate DONE
K2 implementation   DONE
K3.1 runtime parity audit DONE
K3.2 Int64 parity    DONE
K3.3 numeric backend parity DONE
K3 implementation   IN PROGRESS

K4 specialized-effect track
K4.1 specification                    DONE
K4.2 taxonomy/parameters              DONE
K4.3 TIR classification               DONE
K4.4 propagation/witnesses            DONE
K4.5A syntax/provenance               DONE
K4.5B enforcement/semantics-6         DONE
K4.6A boundary audit                  DONE
K4.6B.1 native boundary integrity     DONE
K4.6B.2 compilation boundary tests    DONE
K4.6C guarantee documentation         DONE
K4.6 specialized-effect guarantees    DONE
K4.7A differential coverage audit     DONE
K4.7B focused differential tests      DONE
K4.7C final specialized parity gate   PASS
K4.7 backend parity/reproducibility   DONE
K4.8 family/operation semantics spec  DONE (specification only)
K4.9A taxonomy/parser/AST targets     DONE
K4.9B canonical contracts/driver      DONE
K4.9C analysis/classifier metadata    DONE
K4.9D family/category enforcement     DONE (semantics-7)
K4.9E cache/diagnostic review         DONE
K4.9 family/operation implementation  DONE
K4.10 final effects gate              DONE (bounded PASS)
K4 specialized-effect track           DONE
```

K1.7 raises the cache trust fingerprint from `semantics-1` to `semantics-2`, so
artifacts predating K1.3–K1.6 cannot bypass current comptime semantics. K2.1's
determinism audit is complete. K2.2 gives specialization requests deterministic
first-request order, emits bytecode functions in ascending `TirFunctionId`, and
uses an explicit TIR-to-bytecode function map instead of treating identity as a
vector position. It raises current cache trust to `semantics-3` because revision-2
artifacts may contain the former specialization miscompilation. Bytecode format
and semantic revision remain independent. Native signature integrity remains
separate from the now-explicit HIR bodyless rejection. K2.3 removes cwd from
implicit standard-library discovery in the compiler-authoritative file/module
paths: an explicit resolved `std/` root wins, otherwise the default is discovered
only from the compiler executable's ancestor layout. A relative root supplied
explicitly is resolved once at the API boundary; cwd never selects std
implicitly. Together with K2.2 this establishes Level 1 (deterministic semantic
result/errors) for the audited compiler paths. K2.4 makes module/source
declaration order authoritative wherever interface or field iteration affects
semantics or diagnostics; hash containers remain lookup-only at those
boundaries. It raises cache trust to `semantics-4` because revision-3 artifacts
may encode the former unordered interface selection. Multiple valid interfaces
for one associated type remain a separate type-system semantics debt: K2.4
preserves the existing first-match rule but makes it deterministic. This
establishes Level 2 for the audited compiler-authoritative paths. K2.5 proves
identical runtime results, diagnostics and raw artifacts across fresh compiler
processes, equivalent absolute roots and cwd changes after declared paths are
resolved. This establishes Level 3 for the supported same-build, same-host/target
bytecode surface; it is not a cross-machine Level 4 claim. K2 is complete.
VM/JIT runtime parity continues in K3 for currently executable surfaces; Wasm
and future AOT join runtime parity when those backends exist. Phase timings,
latency baselines and regression budgets continue in K5–K6. Analysis/LSP/REPL
comptime integration remains tooling work, while full comptime specialization is
separate compiler feature/debt. Compile latency remains monitored throughout
development; no deep optimization is implied by K1.

K3.1 audited the executable runtime surfaces without selecting arithmetic rules
by taste or by borrowing future `@verified` semantics. K3.2 establishes the
first shared runtime contract: `Int64` add/sub/mul/neg are checked, division
distinguishes zero from signed overflow, and HIR, TIR, VM and actually-native JIT
execution return controlled failures rather than Rust panics, traps or accidental
wrapping. Native pure and heap-aware call chains propagate `{ value, status }`
without falling back after a semantic failure. E1 treats checked `Int64`
add/sub/mul/div as conservative `Panic` origins. K4.5B raises cache trust to
`semantics-6` while adding static specialized-effect enforcement; the bytecode
format is unchanged. K3 remains in progress.

K3.3 establishes numeric backend parity for `Int64`, `UInt64`, `Float64` and the
existing `UInt8` surface without changing the bytecode format. The TIR verifier
records operand/result types and rejects arithmetic on unsupported numeric
types, so invalid TIR cannot reach bytecode. HIR-to-TIR and TIR-to-bytecode
lowering select `AddI64`/`AddU64`/`AddF64` and the matching subtraction,
multiplication, division and ordering opcodes from verified types; `UInt64` and
`Float64` operations are never silently lowered to `Int64`. `UInt64`
add/sub/mul/div use the shared checked runtime kernel (overflow, underflow and
division by zero are controlled failures). `Float64` keeps IEEE 754 behavior,
including division by signed zero, NaN, infinities, signed zero, ordered
comparisons and lowered unary negation. The TIR interpreter, bytecode VM and
source-level HIR/TIR/VM differential tests agree for the reachable numeric
surface. `UInt64` remains source-unreachable because integer literals are
`Int64` and no numeric conversion constructor is implemented, so its checked
contract is established at the TIR/bytecode/VM boundaries rather than claimed
as source-level parity. The native JIT intentionally
still supports only eligible `Int64` functions: `UInt64` and `Float64`
signatures are rejected as unsupported types and execute interpreted, with no
native execution and no semantic fallback after native failure. Effect analysis
adds checked `UInt64` add/sub/mul to the existing conservative `Panic` origins;
every `Div` remains conservatively `Panic` per E1. K3.3 raises cache trust to
`semantics-8` because revision-7 artifacts may contain untyped numeric lowering
or predate Float64 literal support. K3 implementation remains in progress for
runtime surfaces outside this numeric scope.

K4.6 is complete. K4.6A found no contract-enforcement bypass while auditing the
compilation, runtime and FFI boundaries. K4.6B.1 added native
registry/classifier/signature integrity regressions; K4.6B.2 added fresh
source/file, module-graph, VM/profiling/JIT, cache, unsupported-path,
unknown-extern and inspection boundary regressions. K4.6C records the actual
specialized-effect guarantees and trust limits in
[Specialized Effects v0.1](specs/specialized-effects-v0.1.md). The implemented
guarantee is compile-time only: broad `@deny`/`@pure` remains authoritative,
specialized matching is fail-closed for unknown resources/operations, unsupported
interpreters reject annotated input, and no runtime guard, filesystem sandbox,
network isolation or native-code sandbox exists. K4.6C is documentation-only and
does not change source, runtime behavior, cache format or `semantics-6`.

K4.7 is complete. K4.7A audited existing differential coverage; K4.7B added
focused source/file, explicit-stdlib-root, enforcement-outcome, deterministic
violation, broad-invariance, inspection and VM/JIT regressions. K4.7C adds
independent-process canonical specialized-violation evidence and records the
native `std.*` JIT limitation: JIT v1 rejects entries that call these natives and
the interpreter fallback executes them, with no specialized-enforcement bypass.
The gate verdict is **PASS** for the supported same-build, same-host/target
surface. Runtime guards, Wasm/AOT enforcement, unbound taxonomy operations,
structured E6 specialized diagnostics and cross-machine parity remain explicitly
unsupported or deferred in
[Specialized Effects v0.1](specs/specialized-effects-v0.1.md#104-final-gate-verdict).
K4.7C changes tests and documentation only; cache format and `semantics-6` are
unchanged.

K4.8 is documentation-only. It records the normative v0.2 extension for `fs`
and `net` families, parameterless operation denials, `io.read`/`io.write`
cross-family categories, `net.send`/`net.receive`, and conservative
future-operation matching in
[Specialized Effects v0.2](specs/specialized-effects-v0.2.md). It does not
modify the parser, AST, taxonomy, contracts, matcher, native classifier,
runtime or cache. At that checkpoint the compiler remained v0.1 on `semantics-6`.

K4.9 implements that specification in sequence: K4.9A taxonomy and parser/AST
targets; K4.9B canonical contracts and driver binding; K4.9C analysis and
classifier metadata; K4.9D family/category matching and enforcement; K4.9E
cache and diagnostic compatibility review. K4.9A–E are implemented: contracts
retain targets without expansion, matching uses total canonical operation
metadata and unclassified facts remain conservative. Broad enforcement stays
first. K4.9D atomically raises trust to `semantics-7`, with no cache migration,
format change, runtime guards or native network APIs. Module/file extend methods
with contracts are explicitly rejected because that frontend does not lower
them; source-compiler method binding remains supported. K4.9E confirms v0.1
resource-qualified compatibility and `semantics-7` cache trust without another
semantic change. K3.3 later raises current trust to `semantics-8` for numeric
backend parity; the historical `semantics-7` guarantee remains as described. Typed specialized violations retain target kinds, spans,
classifications and witnesses, but CLI E6 still renders only a generic error
count and omits specialized intent from successful inspection. This is a
documented tooling limitation, not a complete specialized JSON/query surface.
K4.10 completes the bounded final gate and publishes the full per-operation
[coverage ledger](K4_SPECIALIZED_EFFECTS_GATE.md). The verdict is PASS for the
supported static compiler surface: three production-classified operation
identities (four bindings), no additional TIR-only producer and ten taxonomy-only
operation identities. Synthetic matching coverage is not executable native
coverage. The gate also closes a malformed-cache verifier panic for parameters
without local slots using an existing controlled error. Rich specialized E6
projection, module/file extend lowering, missing native APIs, connection identity,
runtime guards, path confinement, broader FFI enforcement and actor integration
remain explicit limits/follow-ups. This completes the K4 specialized-effect
track, not all runtime/backend work or a sandbox guarantee.

K5 and K6 are planned responsiveness work. They are separate from the completed
K4 specialized-effect track.

- **K5 — Phase timings and latency baseline (planned).** Record cold compilation,
  cache miss/hit, VM and JIT execution, and edit-to-compile baselines on named
  reference environments. Measurement only; no semantic change.
- **K6 — Regression budgets and responsiveness gate (planned).** Convert the K5
  baseline into accepted regression budgets and a repeatable release/CI gate.
  P consumes these measurements; K6 does not gate language milestones.

## Future language milestones

### R — Result / Value-Use Hardening (R1–R3 BOUNDED GATE; R4/R5 OPEN)

R follows E semantically. R1–R3 have a reported implementation gate on a
bounded tested surface; R4 error compatibility, R5 parity and R.PROOF remain
open. K5/K6 responsiveness does not block R. R is not a capability or an effect. Result-returning values must not be silently discarded:
handling uses `?`, `match`, return/propagation, or another meaningful use. The
future treatment of `let _ = result_returning_call()` must not create an
accidental escape hatch; this does not add wildcard-binding syntax today. An
explicit opt-out such as `@discardable` may be evaluated later and is not
committed syntax.

R is scoped to **Result**. `Option` handling remains outside R unless a separate
decision expands the scope. R must not introduce generalized affine/linear type
semantics. Storing a Result in a variable or aggregate transfers the handling
obligation to that storage; it does not discharge it. Indirect discards through
bindings, aggregates, destructuring, branches, loops or closure capture must
remain visible to the same obligation tracking. The design should use existing
value-use and control-flow structure rather than a new type-system feature.

1. **R1 — Direct discards.** Reject expression statements that silently drop a
   Result. A tail-position Result is valid when it is the function result.
2. **R2 — Bindings and transfer.** `let`/`var`, destructuring and aggregate
   construction may store a Result, but the stored obligation remains live.
   Unused Result bindings do not satisfy handling. `let _ = ...` must not become
   an escape hatch.
3. **R3 — Control flow and aggregates.** Cover `if`, `match`, `for`, block
   expressions, tuples, structs, enums, arrays and returned aggregates. Storing a
   Result as data is a use; dropping the containing value without handling it is
   still an indirect discard.
4. **R4 — Propagation and error-type compatibility.** Define when `?` is valid
   from the enclosing function result and error type. Preserve the no-implicit-
   coercion rule; do not invent error conversion rules.
5. **R5 — Parity and acceptance gate.** HIR, TIR, VM, JIT, `check`, diagnostics
   and future backends must agree. No cache or execution path may bypass the
   obligation.

Acceptance gate: no silent direct or indirect Result discard across the listed
constructs; stored obligations transfer rather than discharge; no new syntax; no
generalized affine semantics; diagnostics and execution-path parity; existing
programs that already handle Results remain valid.

### M — Memory Model (v0.4 SPECIFIED; IMPLEMENTATION AND M5 OPEN)

Memory correctness is distinct from memory optimization. The intended invariant
is:

> Safe Tempra must not permit construction of strong ownership cycles that
> prevent ARC from reclaiming otherwise unreachable values.

[Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) now defines
this invariant across values, `let`/`var`, `ref`/`inout`, explicit `consume`,
closure captures and `self`, aggregates, borrowed views, nonduplicable resources
and escaping values. The historical interpreter representation does **not**
certify that the approved semantics are already implemented.

Phase M must keep the source language substantially simpler than Rust lifetimes
or Pony reference capabilities. Formal acyclic ownership rules follow R in the
semantic roadmap.

1. **M1 — Model and invariant specification.** Define ownership categories,
   legal edges, escape rules, value semantics and the acyclic invariant without
   adding source syntax.
2. **M2 — Acyclic ownership and ARC correctness.** Cover structs, enums, arrays,
   slices, closures, explicit captures, recursive closure `self`, and escaping
   values. Strong ownership graphs must remain acyclic.
3. **M3 — `ref` / `inout` / `consume` and nonowning references.** Define
   borrow-like rules for the existing capture modes, slice/view behavior,
   use-after-move, exclusivity and escape. `ref` remains the existing read-only
   capture mode unless a later specification explicitly says otherwise; no
   lifetime syntax is introduced.
4. **M4 — Ownership-aware TIR, cleanup and backend parity.** Retain ownership,
   alias and escape facts in TIR; verify them; define deterministic cleanup and
   release order; keep HIR, TIR, VM and JIT behavior aligned. Future A and Wasm
   backends must consume the same facts before claiming parity.
5. **M5 — Mandatory memory stabilization gate.** This is the hard gate before C
   implementation. It requires: no constructible strong ownership cycles in safe
   Tempra; reclamation of unreachable acyclic values; no use-after-free, double
   release or nondeterministic cleanup; sound `ref`/`inout`/`consume` and capture
   behavior; ownership-aware TIR verification; and cross-backend parity on the
   currently supported execution paths. Stress, leak and ownership-regression
   evidence must be retained.

C implementation may not claim task, channel or actor safety before M5 passes.
C design and specification may proceed earlier. Ownership analysis and
preparatory optimization may begin during M; they do not make M complete.

### C — Pluggable Actor Concurrency (PLANNED)

Pluggable actor concurrency depends on both capability summaries and the acyclic
ownership model. Implementation is blocked by M5. No runtime/plugin interface is
specified here. The retained direction is:

- `Task[T]`, `spawn`, and `.await()`;
- structured `TaskGroup`;
- `actor`;
- `Channel[T]` and `select`.

Normal Tempra will not use `async fn`, locks, raw threads as its primary model,
or user-facing atomics. `ref` and `inout` must not cross task, actor, or channel
boundaries. Owned values, immutable share-safe values, and concurrency-safe
handles will form the communication basis; their exact formal rules belong to
Phase C after M.

1. **C1 — Task, task group, channel and selection semantics.** Define structured
   task lifetimes, task results and errors, channel transfer, and `select`
   behavior.
2. **C2 — Actor isolation, structured concurrency and cancellation.** Define
   actor mailboxes, isolation, cancellation propagation, group completion and
   cleanup.
3. **C3 — Pluggable execution domains.** Define the runtime/plugin boundary for
   schedulers and execution domains without exposing locks, raw threads or
   atomics as the primary language model.
4. **C4 — Parity and acceptance gate.** No data races; deterministic cancellation
   and cleanup; `ref`/`inout` never cross boundaries; owned/share-safe/handle
   rules are enforced; `spawn.task`/`spawn.actor` capability facts stay consistent
   with K4; HIR/TIR/VM/JIT and future A parity.

`spawn.task` and `spawn.actor` remain syntax-only taxonomy entries with no
production runtime today. The `scope` parameter remains reserved until C defines
its normative semantics.

### V — Verified Functions (PLANNED)

`@verified` will request stronger static proof that runtime failures are
impossible or explicitly represented. It is not an alias for capability denial
and does not replace ordinary safety. V remains separate from effects and from
the default checked language.

Future checks include panic/crash paths, unsafe operations, overflow and
underflow, division by zero, bounds accesses, numeric narrowing, fallible
allocation, partial operations, and transitive calls. The initial design has no
`requires`, `ensures`, `invariant`, separate proof language, or global
`--verified` mode. Normal Tempra syntax, types, control-flow narrowing, `Result`,
`Option`, and checked APIs express proof conditions.

1. **V1 — Proof-obligation model.** Define the obligations above and how they
   consume E, R, M and C facts without a second analysis.
2. **V2 — `@verified` enforcement.** Enforce obligations for annotated functions
   while preserving `@deny`/`@pure` authority and ordinary checked behavior.
3. **V3 — Parity and acceptance gate.** Discharge or explicitly represent every
   obligation across supported backends; no new proof language; no weakening of
   existing safety or capability contracts.

V gates only claims that a backend provides verified execution. It does not gate
baseline native AOT output.

## Backend and optimization milestones

### A — Tempra Native LLVM AOT (PLANNED, AFTER SCRIPT MATURITY)

Tempra Native is a planned LLVM ahead-of-time distribution of the **same**
language as Tempra Script. It is a core product goal after the scripting
runtime and its required semantic gates have matured. Historical Cranelift
backend crates are scaffolds designated for removal, **not** a future AOT
implementation route. No JIT or Cranelift fallback is a release strategy.

A1–A7 retain their canonical identities. Their implementation status does not
change merely because the chosen backend has changed.

1. **A1 — Native ABI and runtime contract.** Define scalar/aggregate layout,
   calling conventions, `Result`/`Option`, string/array/slice/struct/enum/tuple
   representation, closure conventions, error status, FFI/native integration and
   startup. Gate: documented ABI plus executable conformance tests.
2. **A2 — Ownership-aware code generation.** Consume the normative
   [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) semantics and
   verified TIR ownership/effect summaries. Preserve semantic copies,
   `consume`, `ref`, `inout`, no-cycle/cleanup and nonduplicable resources.
   An RC-free native implementation is an **unproven performance goal**, not a
   prerequisite that may weaken correctness. Gate: no invalid ownership or
   cleanup behavior on the declared surface.
3. **A3 — LLVM AOT backend.** Lower verified typed IR to LLVM ahead of time.
   Emit native object code or a linkable artifact without VM, JIT, or interpreter
   execution. Gate: representative native code executes correctly on named targets.
4. **A4 — Linking.** Produce complete native artifacts with startup/runtime,
   platform linkers and controlled failures. Gate: repeatable links on the
   supported target matrix.
5. **A5 — Standalone executables.** Programs run without a Tempra VM, source
   checkout or JIT. Define arguments, exit codes, standard I/O and distribution
   support. Gate: standalone smoke and functional tests.
6. **A6 — Backend parity.** Tempra Script and Tempra Native must agree on
   observable values, checked errors, effects, ownership and cleanup for a
   **declared identical supported subset**. Unsupported annotations/features
   must be rejected, not bypassed. Gate: reproducible differential tests.
7. **A7 — Performance and binary size.** Record native runtime performance,
   compile/link time, startup, peak RSS and artifact size under K5/K6-style
   named environments. No blanket faster-than-VM claim is implied.

**Schedule:** Native can have preliminary design/prototype work, but product
release and parity commitments follow Script maturity and memory safety gates.
Advanced O/NR optimizations are optional; they never replace the baseline
correctness or runtime fallback policy.

### O — Advanced Memory Optimization (PLANNED)

Advanced optimization that aggressively reduces ARC overhead lands after memory
correctness is established. Preparatory O1/O2 and O4 analyses may begin during M.
O reduces ARC overhead through:

- uniqueness analysis;
- escape analysis;
- stack promotion;
- scalar replacement;
- Perceus-inspired reuse;
- retain/release elimination.

These are performance techniques, not prerequisites for memory correctness,
concurrency or native AOT.

1. **O1 — Uniqueness analysis.**
2. **O2 — Escape analysis, stack promotion and scalar replacement.**
3. **O3 — Perceus-style reuse and retain/release elimination.**
4. **O4 — Preparatory analyses for NR.** Ownership inference, escape analysis and
   region preparation may begin during M. O4 does not make NR part of O and does
   not commit RC-free behavior.

Acceptance gate: semantics and M guarantees unchanged; no new cycles or
miscompilation; measured improvement; HIR/TIR/VM/JIT and A parity on the supported
surface. O does not gate C or A. The `tempra-opt` crate remains a scaffold.

### NR — RC-Free Research (STANDALONE RESEARCH TRACK)

NR is experimental research, not a committed language guarantee and not an O
sub-milestone. It must never block baseline native AOT. O4 may prepare required
analyses, but NR owns the investigation.

NR explicitly investigates RC-free functions, modules and native programs:

1. **NR1 — Ownership inference and escape analysis.** Infer ownership and escape
   behavior for RC-free candidates, reusing M/O facts where sound.
2. **NR2 — Regions.** Investigate region-based lifetime and allocation reasoning
   without adding source-level lifetime annotations.
3. **NR3 — Direct cleanup for functions, modules and native programs.** Explore
   direct cleanup and whole-program ownership where proof succeeds.
4. **NR4 — Fallback ARC.** Define boundaries where proof fails; baseline ARC
   remains available, correct and interoperable with RC-free regions.
5. **NR5 — Benchmark-driven feasibility gate.** Measure runtime, compile time,
   binary size and complexity. Adoption requires explicit evidence; negative
   results are valid. No NR result may gate A.

NR introduces no new syntax and makes no language guarantee. Its findings may
inform O or A later but are not prerequisites for either.

### P — Compiler Performance (PLANNED)

P is the deep optimization milestone for compilation responsiveness. It may
address cold-compile latency, incremental compilation, module-level
invalidation/cache, specialization deduplication and control, reuse of stable
facts, and interning/arena/data-structure optimization. Parallelization follows
profiling rather than preceding it. Agent and IDE edit-to-compile latency is a
product requirement. E6 introduces no incremental cache, daemon or persistent
capability facts.

1. **P1 — Cold compilation and baseline budgets.** Measure and record the initial
   baseline; define reference environments.
2. **P2 — Incremental compilation and cache invalidation.** Module/dependency
   invalidation, stable facts and correct reuse.
3. **P3 — Specialization control and deduplication.** Deterministic
   specialization identity, code-size control and cache compatibility.
4. **P4 — Memory footprint, artifact size and data structures.** Interning,
   arenas, serialization and allocation reduction.
5. **P5 — Profiling-driven optimization and latency gate.** Optimize after
   evidence; publish accepted latency budgets and regressions.

Acceptance gate: cold, incremental and edit-to-compile budgets are recorded
against named reference environments; cache invalidation and reproducibility
remain correct; specialization does not blow up artifact size; memory and
artifact budgets are met; no semantic change; parallelization follows profiling.
K5/K6 supply the initial measurements. P may overlap A and O and does not gate A.

### U — Unsafe Boundary (DEFERRED / CONDITIONAL)

U is a small standalone conditional milestone shown after P but activated by a
concrete trigger, not by list position and not as a prerequisite for completing
E: the first real source-level operation capable of violating Tempra memory/type
invariants unless explicitly authorized. No such intentional primitive exists
today, so `@unsafe` is not implemented or accepted.

The intended direction is local authorization within small audited implementation
islands, without automatically requiring annotations throughout the caller graph.
Safe wrapper interfaces should remain callable from ordinary safe Tempra and must
preserve safety for their admitted inputs. This does not settle unsafe-to-call
rules or introduce hypothetical primitives. `@unsafe` must never disable `@deny`
or `@pure`; `@deny(unsafe)` remains invalid and `Capability::Unsafe` remains
internal/reserved.

Activation gate: a concrete source-level primitive must exist; authorization must
stay local; safe wrappers must preserve admitted-input safety; deny/pure must
remain authoritative; no speculative unsafe syntax or primitives may be added
ahead of the trigger.

## Parallel engineering tracks

Tempra Native LLVM AOT is milestone A above, not merely a parallel-track entry. The remaining
engineering tracks are explicit and do not weaken the semantic dependency chain.

| Track | Status | Current position and gate |
| --- | --- | --- |
| Tempra Native | Milestone A (LLVM AOT) | Future distribution after Script maturity; source tree contains legacy scaffolds, not a working LLVM backend. |
| Wasm / WASI | Planned | Dedicated Wasm emission and host crates remain scaffolds; Wasm does not depend on the discarded Cranelift backend. Gate: verified output, hosting, contracts and memory parity. |
| Tempra Script | Experimental VM path; future default | Bytecode VM with a narrower documented subset and cache; K3 parity and default-promotion gate remain open. |
| Legacy JIT removal | Retirement change | Crates, features, dependencies, CLI paths and CI jobs removed; validate workspace and cross-platform CI. Historical notes remain archived. |
| Type-system hardening | Planned / ongoing | Multiple valid interfaces for one associated type still use deterministic first-match (K2.4 debt); module/file `extend` lowering is limited. Gate: specified selection rules, diagnostics and tests. |
| Comptime specialization | Planned | Full HIR specialization and runtime erasure are not implemented; `SpecializationKey` names remain temporary. Gate: deterministic specialization, runtime erasure, code-size control. |
| Standard library | Implemented MVP, expanding | `std.fs`, `std.path`, `std.strings`, `std.io`; coverage differs between HIR and VM. Gate: stable APIs, backend parity, tests. |
| REPL / LSP / tooling | Implemented with gaps | LSP, REPL, CLI, diagnostics and effect inspection exist. Comptime integration and rich specialized E6 projection remain deferred. Gate: integration without weakening current frontend boundaries. |
| TIR Graph | Planned later | May combine calls, CFG, capabilities, allocations, ownership edges, escapes, actor boundaries, specializations and provenance. Requires M/C and later facts; not promised by current inspection. |
| Package management | Planned | A safe CLI package manager remains separate. Build/install scaffolds are not production package management. Gate: manifest/lockfile, integrity, reproducibility. |
| Release hardening | Ongoing / planned | Cross-platform CI exists. Hostile cache artifacts, native signature integrity and `is_extern` serialization remain separate debts. Gate: release checklist, hardened cache and boundary validation. |

## Acceptance gate summary

| Gate | Required outcome |
| --- | --- |
| R | No silent direct or indirect Result discard; obligations transfer through storage; no new syntax or generalized affine semantics; backend/diagnostic parity. |
| M5 | Acyclic strong ownership, reclamation, deterministic cleanup, sound `ref`/`inout`/`consume` and capture behavior, ownership-aware TIR, cross-backend parity. Blocks C implementation. |
| C | Structured task/group/actor/channel/select semantics, isolation, cancellation, no `ref`/`inout` crossing, no data races, pluggable domains and backend parity. |
| V | Proof obligations discharged or explicitly represented; no proof language; separate from effects; backend parity. |
| A | Native ABI, ownership-aware LLVM AOT, linking, standalone executables, Script/Native semantic parity, independent performance/size baselines. |
| O | Semantics and M guarantees preserved; measured optimization; no miscompilation; RC-free not implied. |
| NR | Evidence-based feasibility with fallback ARC; no language guarantee; no gate on A. |
| P | Cold/incremental/edit-to-compile budgets, cache correctness, specialization control, memory/artifact budgets, no semantic change. |
| U | Activated only by a concrete unsafe primitive; local audited authorization; safe wrappers; deny/pure preserved. |

## Cross-cutting constraints

- No milestone may add source syntax through this roadmap. Syntax changes require
  their own specification.
- This consolidation changes documentation only. It does not alter compiler
  behavior, bytecode, cache format or normative specifications.
- Implemented, experimental, planned and research status must remain explicit.
  A scaffold or taxonomy entry is not completed functionality.
- Existing guarantees and limitations remain in force: `check` is frontend-only;
  HIR/REPL/test execution reject annotated programs; cache hits are
  verified-bytecode-only; specialized enforcement is compile-time only; no
  runtime sandbox exists; TIR/Wasm/AOT coverage remains bounded.
- Native AOT remains a core product goal.
- No artificial global sequencing: workstreams may overlap when their actual
  prerequisites exist. M5 is the mandatory safety gate before actor concurrency implementation.
- Other milestones follow their explicitly documented technical prerequisites.
- RC-Free research must never block baseline native AOT.
- Unsafe remains deferred and conditional; no speculative unsafe primitives.

## Terminology

- **checked**: normal Tempra semantics; safe and implicit, with no `--checked`
  flag;
- **verified**: stronger local static guarantees requested by future `@verified`;
- **unsafe**: an explicit boundary for low-level operations unavailable to normal
  safe Tempra.

The former "Spark mode" and global checked/verified mode proposals are not part
of the current design.

## Open architectural decisions

The following decisions remain open and are not committed by this roadmap:

- R enforcement stage (typechecker, HIR or TIR), diagnostic severity, and the
  exact treatment of stored Result obligations.
- Whether an explicit discard opt-out is ever proposed; `@discardable` is not
  committed.
- M ownership representation, nonowning-reference boundaries, and cleanup
  strategy.
- C scheduler, execution-domain API, cancellation propagation, `select` fairness
  and actor mailbox ordering.
- C transfer rules for owned values, share-safe values and concurrency handles.
- A target matrix, ABI details, linking model, runtime startup strategy, and
  whether A supports C concurrency in its first release.
- LLVM target matrix, lowering/ABI choices and backend compilation strategy; the LLVM backend decision itself is finalized.
- NR region model, fallback ARC boundaries, adoption granularity and feasibility
  thresholds.
- O4/NR analysis ownership and the boundary between preparatory work and research.
- P reference budgets, incremental-cache granularity, and parallelization
  threshold.
- V proof-obligation boundary and interaction with `Panic` and fallible allocation.
- K5/K6 exact deliverables and whether they gate P only.
- U activation criteria and safe-wrapper rules.

## Historical references

The richer **TIR Graph** is a later tooling milestone. It may eventually combine
calls, CFG, capabilities, allocations, ownership edges, escapes, actor boundaries,
specializations and source provenance. Some facts require M, C and later compiler
work and cannot be promised by current capability inspection.

The Phase 15–18 JIT documents are historical implementation records for an
opt-in experiment. They are not the normative language roadmap. The same applies
to phase-numbered lowering, VM and performance records: they document their
named prototypes and do not override current status here.

## Script ecosystem priority (2026-11-26)

After ownership conformance, final VM parity, ARC NO-CYCLE, VM-only comptime
and VM-only transition, the following milestones precede Native implementation:

1. **Shell REPL** — interactive scripting environment.
2. **StdLib rework** — coherent ownership-aware standard library.
3. **Tooling rework: LSP, fmt, lint, graph** — shared compiler infrastructure.
4. **Tempra MCP** — agent-facing language and tooling integration.

The Native AOT direction (custom backend + LLVM) remains planned, but the
complete Script environment takes implementation priority. This ordering
supersedes earlier sequencing where it conflicts, without changing the
normative Memory & Ownership v0.4 contracts.
