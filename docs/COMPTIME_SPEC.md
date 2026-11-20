# Tempra — Comptime Specification

**Revision:** v0.2 (documentation consolidation, 15 November 2026)
**Status:** Consolidated documentation for review. Established source contracts and K1 history preserved; Memory v0.4 applied; VM migration **planned, not certified**.
**Scope:** Compile-time evaluation, type-directed specialization, determinism, resource limits, capability policy, memory safety and the transition to Tempra Script bytecode VM.
**Normative dependencies:** [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md), [Surface Syntax v0.2](TEMPRA_SURFACE_SPEC_v0.2.md), [Grammar](GRAMMAR.md), [Capability Contracts](specs/effects-v0.1.md), [Specialized Effects v0.2](specs/specialized-effects-v0.2.md).

> **Authority and status:** This document defines `comptime` semantics, not an independent memory manager or effect system. Memory & Ownership v0.4 is the sole authority for ownership, copying, borrowing, resource cleanup and backend equivalence. The capability specifications govern user-visible `@deny`/`@pure` contracts. Where this document describes K1–K4 or a historical HIR evaluation path, it records an implementation baseline, **not** evidence of completed VM migration or proof of memory safety. Any discrepancy in current implementation must be reported, not silently accepted as a new language rule.

## 1. Design contract

`comptime` is an explicit, typed compile-time evaluation facility. It supports simple deterministic computation, specialization and selection of language constructs **before executable runtime code is emitted**.

The core guarantees are:

1. **One language.** Compile-time and runtime Tempra obey the same type, value, ownership and borrowing rules. `comptime` grants no implicit copying privilege, lifetime extension, resource duplication or unsafe authority.
2. **Restricted execution.** Compile-time operations must be deterministic within the supported evaluation model. Arbitrary I/O, host state observation, process execution and unsafe foreign calls are not permitted simply because they appear in `comptime`.
3. **Explicit phase boundary.** Known results and selected branches are materialized or resolved before the ordinary runtime emission path. Unselected compile-time branches do not contribute runtime instructions or capability requirements.
4. **Bounded progress.** An evaluation has reproducible progress and call-depth limits. Failure produces a controlled diagnostic, not a Rust panic, silent partial result, or implicit fallback to host execution.
5. **Backend-independent meaning.** Tempra Script's current/historical HIR kernel and future bytecode VM comptime kernel must produce equivalent accepted values, diagnostics, branch decisions, effects and ownership outcomes for the supported common subset. Tempra Native never invents different `comptime` semantics.
6. **No implementation claim from a proposal.** New VM execution, nonprimitive result materialization, advanced specialization and general FFI remain subject to separate implementation and conformance gates.

`comptime` is an evaluation **phase**, not an effect, a public lifetime annotation, an ownership mode, a compile-time escape hatch or a separate scripting dialect.

## 2. Status vocabulary and architecture

| Label | Meaning |
|---|---|
| **Language contract** | Semantic rule that accepted Tempra code must satisfy |
| **Documented K1 implementation** | Behavior reported by the original K1 specification on the HIR interpreter |
| **Migration target** | Desired VM-backed behavior; not certified until tests and integration prove it |
| **Deferred** | No settled public syntax or implementation commitment |

### 2.1 Documented historical execution path (K1)

```text
Source → AST → HIR/typechecking
                  └─ comptime facade → restricted HIR evaluator
                          ├─ values / controlled diagnostics
                          └─ selected branches + materialized HIR constants
               → typed HIR → verified TIR → backend-specific emission
```

- K1.1 installed one canonical compiler-facing evaluation facade around the HIR interpreter. It owns comptime mode, evaluation fuel, result/error/control-flow normalization, and restoration of the previous evaluator state.
- K1.2 supplied module-aware lookup for ordinary multi-module/file typechecking, retaining separate module HIR identities rather than flattening imports.
- Nested `comptime` nodes share an evaluation session and its remaining resource budget.
- The typechecker owns the conversion from supported evaluated values to HIR constants. The runtime TIR lowerer and runtime backends do **not** reevaluate the expression.
- In the original baseline, analysis/LSP and REPL integration and full specialization were separate follow-ups.

The reference to the HIR interpreter is a **historical implementation fact**. It is not an architectural requirement to retain that interpreter after a tested VM migration.

### 2.2 Target execution path (Tempra Script VM)

```text
Source → frontend/typechecking + specialization context
       → restricted comptime compilation/evaluation on Tempra Script VM
       → controlled value / diagnostics / branch selection
       → materialization in compiler-owned typed representation
       → verified runtime TIR → bytecode VM or LLVM AOT
```

This diagram specifies responsibilities rather than a fixed crate-level implementation or an approved intermediate-representation layout. The migration must account for type-dependent evaluation occurring **during** checking: the compiler cannot assume that an ordinary fully checked runtime TIR module already exists at every comptime evaluation point. It may use a restricted evaluation IR, staged lowering, or another verified adapter without introducing different language semantics.

**Promotion condition:** The VM becomes the canonical `comptime` execution engine only after the gates in §17 pass. There must be no hidden fallback to the old HIR evaluator for code claimed to be VM-backed. If a feature is unavailable, report a controlled unsupported/compilation error or postpone promotion of that feature.

### 2.3 Backend scope

Tempra Script is the scripting-first stack VM. Tempra Native is the planned LLVM AOT compiler. `comptime` is resolved at compilation and must not depend on JIT or Cranelift. Neither a JIT feature nor Cranelift-based native code generation is part of the forward-looking `comptime` architecture. Their historical tests do not confer new `comptime` guarantees.

## 3. Supported surface syntax

The forms below are established by the Surface Specification and Grammar; runtime/typechecker support must still be audited on the active development branch.

### 3.1 Expression and block

```tempra
let n = comptime 1 + 2

let total = comptime {
    let a = 1
    let b = 2
    a + b
}
```

A `comptime` block follows ordinary expression-block semantics: its final expression supplies the value. It does not create a separate ownership or scope system. The historically implemented K1 materialization subset comprises `Int`, `Float`, `Bool`, `String` and `Unit`; this document does not assume that arrays, user structs or arbitrary resources can already be embedded as compile-time results.

### 3.2 Named function `comptime` parameters

```tempra
fn add_n(comptime n: Int, x: Int) -> Int {
    x + n
}

let answer = add_n(3, 4)
let another = add_n(comptime 1 + 2, 4)
```

A `comptime` value argument must be known at the call site: a supported literal, explicit `comptime` expression or an available `comptime` parameter. A normal runtime binding is **not** implicitly reclassified as comptime just because its initializer looks constant.

`comptime` parameters are immutable and part of specialization identity. The documented Phase 2 syntax applies to **named functions**, not to closure parameter lists.

### 3.3 Compile-time conditional

```tempra
fn default_value(type T) -> T {
    if comptime T == Int {
        0
    } else if comptime T == Bool {
        false
    } else {
        compile_error("unsupported type")
    }
}
```

The condition must resolve to a compile-time `Bool`. Unselected alternatives must parse, but are not typechecked as runtime branches. Selection may be deferred when dependent on a generic type until its specialization is known.

### 3.4 Compile-time match

```tempra
fn default_value(type T) -> T {
    comptime match T {
        Int => 0
        Bool => false
        String => ""
        else => compile_error("unsupported type")
    }
}
```

The scrutinee must be compile-time known (or depend on a deferred generic/comptime parameter). Patterns are tested in source order; currently documented K1 support covers literals and type-name patterns. The chosen arm alone is semantically retained; unmatched alternatives are pruned. At specialization, a selected arm or a valid fallback is mandatory. General constructor/tuple/struct patterns are **not** claimed implemented by this document.

### 3.5 Type values and equality

```tempra
fn is_int(type T) -> Bool {
    T == Int
}

let result = is_int[Int]()
```

Generic type parameters and primitive type identifiers can be evaluated as type values. `==` and `!=` compare compatible type values; this does not imply arbitrary reflection, runtime dynamic interface dispatch or new source coercions.

## 4. Compile-time-known values and scope

A value is eligible for evaluation only when the comptime environment can establish it under the supported rules. An ordinary `let` declaration is immutable **at runtime** but is not implicitly a comptime constant. Neither `let` nor `comptime` grants `Static` memory provenance.

```tempra
fn example(x: Int) {
    let a = 3
    let one = comptime 2 + 1
    // comptime (x + 1) : error, runtime parameter
    // comptime (a + 1) : error, runtime local, even though initialized by literal
}
```

A named `comptime` parameter is available as a compile-time binding in the function's allowed specialization context. Type values introduced by generic `type` parameters are available for type-directed branching. An unresolved parameter cannot be read as a concrete host value before the required specialization is established.

Compile-time evaluation must respect lexical scoping, module qualification and shadowing exactly as typechecked Tempra would. It must not search uncontrolled host locations, fabricate missing function bodies or silently resolve imports differently from the compiler's explicit module graph.

## 5. K1 phases: implementation baseline

The table records what the pre-migration documentation reports. It is **not** a new CI attestation for the active `feature/k5-compiler-timings` working tree.

| Workstream | Documented K1 outcome | Exact scope |
|---|---|---|
| K1 audit | Complete | Chose one restricted HIR semantic kernel |
| K1.1 | Complete | Facade, session state, boundary normalization, shared fuel |
| K1.2 | Complete | Multi-module/file-aware lookup; no flattened HIR |
| K1.3 | Complete | Controlled integer errors in the shared evaluator |
| K1.4 | Complete | Reject bodyless/extern calls during comptime |
| K1.5 | Complete | Single deterministic compile-time operation policy |
| K1.6 | Complete | Deterministic fuel ticks and call-depth guard |
| K1.7 | Complete | Cache semantic revision 2 for K1 trust |
| K2.1–K2.5 | Documented complete | Reproducible specialization order, stdlib resolution and bounded cross-process gates |
| K3 | In progress in historical snapshot | Runtime arithmetic/effect parity, not a blanket all-backend certification |
| K4 | Documented complete for bounded effect surface | Specialized capability contract and taxonomy enforcement |
| K5/K6 | Follow-up / branch-specific | Measurement, responsiveness, and budgets; verify live branch status |

The historical status must not be rewritten as “not implemented” merely because the VM is now the chosen target. Conversely, a working HIR implementation does not prove VM parity or Memory v0.4 implementation.

## 6. Arithmetic, typing and controlled errors

K1.3's shared HIR evaluator reports controlled arithmetic errors for supported operations:

| Operation | Documented behavior |
|---|---|
| `Int64` addition/subtraction/multiplication | Checked signed overflow |
| `Int64` division | Controlled division-by-zero and `MIN / -1` overflow |
| Unary signed negation | Checked signed overflow |
| `UInt8` division | Controlled division-by-zero |
| `UInt8` add/subtract/multiply | Historical wrapping behavior retained by K1.3; this **does not** establish an independent normative unsigned-arithmetic rule |
| Floating-point arithmetic | Historical floating behavior unchanged by K1.3, including IEEE division by zero |
| Remainder/modulo | Not supported source syntax in the K1 baseline |
| Right shift | Historically parsed but rejected by the typechecker in the K1 boundary |

The Surface Specification and verified current runtime parity govern the supported arithmetic contract, including `Panic` effects. A backend's accidental native overflow, Rust panic, trap, or wrapping behavior is not a substitute for a Tempra diagnostic.

`compile_error("message")` causes an intentional compilation diagnostic **only in a permitted comptime context** or a selected comptime branch. It is not a runtime exception and is not valid as an ordinary runtime call.

```tempra
fn required_type(type T) -> T {
    if comptime T == Int {
        0
    } else {
        compile_error("Int required")
    }
}
```

Errors in an unselected branch must not invalidate a valid specialization. Errors in the selected branch must not be silently swallowed by dead-code optimization, a cached artifact or backend fallback.

## 7. Effect restrictions at comptime

### 7.1 Compile-time admission is not `@pure`

The compile-time evaluator has a **restricted execution policy**, distinct from a public `@pure` or `@deny(...)` annotation. `@pure` permits allocation and possible `Panic`, whereas `comptime` additionally disallows many host-observable or nondeterministic operations. A function being `@pure` does not automatically make every call comptime-evaluable; a function without `@pure` is not automatically denied if its actually evaluated operations are permitted by the comptime policy.

The public capability vocabulary remains defined in the linked effect specifications; there is no public `@comptime_pure`, `@allow`, new effect namespace or implicit unsafe authorization.

### 7.2 K1.5 operation classes

| Class | Meaning | Examples documented in K1 |
|---|---|---|
| **A** | Permitted deterministic local evaluation | Arithmetic, local strings/regex/collections, deterministic local allocation, loops, constructing a `Command` value via `cmd(...)` |
| **B** | Forbidden host-observable operation | Output, input, filesystem, environment, host OS, process execution, `exit`, `dbg` |
| **C** | Not executable / bodyless, rejected | `std.io.print`, `std.io.println` as extern declarations without executable HIR bodies |
| **D** | No supported source/evaluator origin in the historical snapshot | Spawn, random, time, unsafe operations |
| **E** | Unresolved policy defect | **Must be zero** for a completed audit |

This is the documented K1 audit taxonomy. New stdlib or runtime primitives require reclassification rather than automatic admission. Host-observable or nondeterministic operations may not be allowed simply because they have no mapped historical HIR instruction.

`cmd("git", ["status"])` constructs a local value and was allowed; **executing** the command remains forbidden. Similarly, an extern declaration encountered in a comptime call must produce a controlled unsupported/bodyless error, not a synthetic `Unit` or an accidental native dispatch.

### 7.3 Runtime effect analysis and pruning

Compile-time selection happens before ordinary runtime TIR capability analysis. The executable branch alone contributes to runtime effects for that specialization. This is semantic pruning, not reliance on optimizer dead-code elimination.

The compile-time evaluator must independently enforce its own execution restrictions at the operation boundary, including calls to helper routines. Existing runtime `@deny`/`@pure` obligations are **not waived** by comptime evaluation; the compiler must not reuse a cached result that bypasses currently required checks.

### 7.4 Allocation, panic and cleanup

Under Memory v0.4:

- `@deny(alloc)` uses a conservative backend-independent classification. A heap allocation hidden inside a VM helper, result materialization or cleanup cannot be ignored if the relevant contract covers it.
- `@pure` does not prohibit `Alloc` or `Panic` unless a separate denial does so.
- `@deny(panic)` cannot be discharged merely because a particular execution happened not to overflow or access out of bounds; established type/effect contracts determine admissibility.
- Non-fallible automatic cleanup may occur only where ownership permits. Any observable cleanup requiring I/O/process or other forbidden capability must not be hidden inside an otherwise comptime-admitted operation.

The exact accounting boundary between compiler-internal evaluation storage and program-attributable `Alloc` still requires conformance tests. This specification does not claim that the current HIR kernel or future VM is already certified against Memory v0.4's full `@deny(alloc)` contract.

## 8. Deterministic progress limits (K1.6)

The documented v0.1 restricted evaluator uses a **shared one-million-tick default budget** for comptime evaluation. A tick is a semantic evaluation-progress unit, not a time measurement or proportional CPU/memory budget.

It charges one tick for each:

1. Expression entry.
2. Statement entry.
3. Tempra call-frame entry.
4. Actually executed `for` body, **after** `next()` returns an item.

A budget of `N` allows exactly `N` successful ticks; the next attempted tick at zero reports `FuelExhausted`. The documented default call-depth safety bound is **8 simultaneously active Tempra frames**; frame 9 reports `CallDepthExceeded`. Both limits are compiler-versioned policy parameters, not immutable source-language constants.

Rules:

- A nested `comptime` node uses the **same session**, remaining fuel and call-depth state; it does not reset either counter.
- The historical evaluator checks exhausted fuel before call-depth errors and before the operation's own error at the same boundary.
- Blocks and branch/match selection are bounded by their enclosing expression and statement ticks; no `while` construct existed in the historical HIR surface.
- Builtin, regex, string and collection helper invocations had **constant semantic cost per invocation** in this baseline, not proportional-per-byte fuel.
- There is no documented wall-clock timeout, CPU-time limit, memory quota or byte-processing budget. One million ticks does **not** imply bounded CPU or heap use in the general case.

### 8.1 VM migration: preserve semantic budgets, not instruction counts

A future VM will use different instruction granularity. Raw bytecode instruction count is **not** interchangeable with the K1.6 expression/statement/call/iteration tick contract. The implementation must either carry semantic progress checkpoints into bytecode or define and approve a deliberately versioned new budget contract.

The migration tests must compare nested-session behavior, exhaustion precedence, call depth, loop boundaries and diagnostics. It must not reset fuel at an internal call or treat “no timeout” as license to perform arbitrary unbounded native helper work.

## 9. Determinism, module resolution and reproducibility

Determinism applies to accepted values, selected branches, controlled errors, specialization ordering and compiler artifact trust within an explicit supported envelope.

The K2 documented work includes:

- **K2.1:** audit of nondeterminism sources.
- **K2.2:** deterministic first-request specialization ordering and separation of semantic function identity from physical bytecode storage position.
- **K2.3:** explicit standard-library root discovery; no hidden current-working-directory search for compiler-authoritative module paths.
- **K2.4:** stable source/module declaration order for interface selection, requirements and struct-field diagnostics.
- **K2.5:** cross-process, byte-for-byte reproducibility gates for the supported emitted bytecode surface under identical source/module order, compiler build, semantic revision, format, options, resolved stdlib contents, host and target.

**Bounded guarantee:** K2.5 does not establish identical bytecode across architectures, compiler builds, targets or arbitrary environments. Interface selection and invalid-interface diagnostics were tested at the frontend boundary where the corresponding source was not yet bytecode-lowerable.

The future VM evaluator must not introduce nondeterministic iteration, scheduler-dependent result selection, host-dependent resource reads, unstable function IDs or undocumented changes to diagnostic priority.

## 10. Semantic revision and cache trust

`semantics-N` is a compiler-trust revision separate from the binary bytecode format version. The historical cache format is version 2 and the latest revision documented on GitHub `main` is **`semantics-8`**; this is a **snapshot**, not a verified claim about the user's unpushed local branch.

| Revision | Documented reason |
|---|---|
| `semantics-2` (K1.7) | Canonical comptime arithmetic/bodyless/effect/fuel behavior |
| `semantics-3` (K2.2) | Stable specialization request / function identity mapping |
| `semantics-4` (K2.4) | Deterministic interface and diagnostic iteration |
| `semantics-5` (K3.2) | Checked `Int64` runtime/`Panic` facts |
| `semantics-6` (K4.5B) | Specialized-denial enforcement integrity |
| `semantics-7` (K4.9D) | Specialized v0.2 family/category matching |
| `semantics-8` (K3.3) | Numeric backend parity |

Revision-1 artifacts and earlier semantic revisions that fail current trust requirements cannot be reused as valid hits, even if their bytes can still be decoded. A trusted hit still requires version, source/module identity, explicit stdlib and other active compatibility inputs to match; decoding alone is not verification.

### 10.1 Future VM and ownership transition

The transition to canonical VM `comptime`, and any implementation change that affects accepted programs, ownership verification, effect enforcement, diagnostic behavior or specialization correctness, **must invalidate untrustworthy artifacts** through the compiler's normal semantic revision/fingerprint policy. No new `semantics-N` number is assigned by this document; that is an implementation decision tied to the actual change.

Comptime/ownership facts must not be fabricated as already serialized in the cache if the current format only stores verified bytecode. If necessary, recompute compiler-authoritative facts from trusted sources rather than guessing from a cache hit.

## 11. Memory & Ownership v0.4: universal applicability

Memory & Ownership v0.4 applies equally during comptime evaluation and to values derived from it. **Phase separation does not create stronger ownership privileges.**

| Case | Rule |
|---|---|
| `let` or comptime-generated local | Binding immutability does **not** mean `Static` provenance |
| Passing duplicable `T` by value | Independent value semantics; backend may elide a physical copy at proven last use |
| Explicit `ref` | Nonowning read access, valid only while underlying owner is valid |
| Explicit `inout` | Exclusive mutable access with no incompatible live aliases |
| Closure capture `fn[consume x]` | Semantic transfer; source binding invalidated |
| Ordinary by-value argument | Not semantic `consume` merely because optimizer sees a last use |
| Nonduplicable resource | Cannot be implicitly copied; must use a legal explicit ownership/borrow contract |
| `Slice(T)` | Read-only nonowning contiguous view; not a detached copy |
| `inout Slice(T)` | Exclusive mutable access to underlying storage |
| `Range` | Independent half-open `[start, end)` descriptor, no owner lifetime extension |
| Borrowed aggregate | Temporary/local only under v0.4 restrictions; cannot escape as an independent persistent owner |
| Borrow through `await` | No dynamic live borrow over suspension in the v1 contract |

The syntax of `ref` in arbitrary named-parameter position, `consume` on named parameters/receivers, and the declaration mechanism for nonduplicable resource types remain **SURFACE OPEN** under Surface v0.2 / Grammar. This document does **not** add those spellings to implemented syntax by implication.

### 11.1 Temporary allocation is not static provenance

A comptime evaluation may create temporary arrays, strings, slices or other values to calculate an answer. The allocation's occurrence during compilation does **not** prove that its storage survives as a static runtime object.

When producing a runtime value, the compiler must **materialize an independent legal value** in the supported representation. A `Slice(T)` from temporary comptime storage cannot silently escape into the emitted program. Valid solutions may use an owned materialization when explicitly supported or a separately proven `Static` object; neither is implicit borrow repair.

### 11.2 Borrow liveness and provenance

The source-to-comptime boundary obeys the same NLL-like, CFG-sensitive rules for loans, including branches, loops, joins, returns and callback captures. An evaluated temporary must not be destroyed while a legal borrow remains in use, and no reference to evaluator stack/heap storage may escape into runtime artifacts.

The compiler must not add a hidden owning copy merely to convert an invalid borrowed escape into a valid program. An expression that cannot be materialized with the required provenance is rejected with a controlled diagnostic.

### 11.3 Unique resources and cleanup

The comptime evaluator cannot synthesize a second owner for a nonduplicable resource. Owned resources must be released at their valid ownership boundary, including on controlled error or fuel exhaustion; there must be no double free or use-after-free.

Host-backed resources, foreign handles, I/O-bearing drops, untrusted FFI pointers and pending async operations cannot be made comptime-safe by generic ownership machinery alone. These operations remain disallowed or explicitly **DEFERRED** until a restricted, auditable contract exists. Cleanup must not smuggle forbidden host effects into otherwise deterministic comptime execution.

### 11.4 Recursive closure `fn[self]`

Recursive closure identity is an existing language feature, not permission for a strong ownership cycle. A comptime evaluator supporting recursive closures must preserve the same NO-CYCLE invariant and explicit capture modes (`ref`, `inout`, `consume`) as runtime Tempra. It may not fall back to a cyclic self-owned environment or RC merely because the value is evaluated during compilation.

Whether a particular closure or recursive specialization is currently comptime-evaluable depends on actual evaluator/lowering coverage; this document does not claim complete closure support in the historical K1 evaluator or the future VM.

## 12. Generics, specialization and phase ordering

### 12.1 Generic type values

`type T` is a compile-time type input. It may be compared with other supported type values, used to choose `if comptime` branches and matched in `comptime match`. A generic type projection `T.Item` is a **type-position** feature per Surface Syntax; this document does not promote it to a runtime or comptime expression automatically.

### 12.2 Specialized function identity

A specialization identity must distinguish the target function, concrete type arguments and required compile-time value arguments.

```text
SpecializationIdentity (conceptual)
    function_identity
    type_arguments
    comptime_arguments
```

The older Phase 2 specification illustrated a `SpecializationKey` with a temporary **function name string** and warned that stable `FunctionId` was future work. K2.2 subsequently documented a repair separating semantic function identity from physical bytecode storage index. Consequently, the older sample struct is a **historical bootstrap illustration**, not a current canonical ABI or claim that name strings are sufficient.

### 12.3 Value parameter erasure

The Phase 2 implementation accepted comptime parameters but explicitly deferred complete generated-HIR specialization, constant substitution and removal of comptime parameters from runtime signatures. Full erasure must be validated independently of call-site comptime validation and specialization bookkeeping. Do not label it implemented solely because a `SpecializationKey` exists.

### 12.4 Deferred branches are not unchecked code execution

When a generic condition or scrutinee is unresolved, defer selection until the actual specialization is known. The parser must still accept all branches; the compiler only typechecks and emits the selected branch once the compile-time decision is available. It must not execute an unselected branch, collect its errors as though reachable or silently skip semantic checks on the selected one.

## 13. Modules and compiler authority

The compiler evaluates comptime expressions with the same explicitly resolved module graph and stdlib root as the normal compilation. Module-qualified calls must retain their declaring module identity and must not silently dispatch to an unrelated function with the same name.

Imports are not an authorization to inspect arbitrary host files or the environment during comptime execution. Loading source modules through the compiler's explicit resolver is distinct from allowing user code to perform filesystem I/O in the comptime evaluator.

If an imported function has no supported executable body, the evaluator reports a controlled error. No automatic native/FFI fallback is permitted.

## 14. Diagnostics, failures and cancellation of evaluation

Comptime diagnostics should be stable, typed and useful to compilers, CLI, LSP and agents. At minimum distinguish:

| Failure | Required diagnostic class |
|---|---|
| Runtime local or parameter used in comptime | Not compile-time known |
| Comptime condition not `Bool` | Wrong compile-time condition type |
| Unsupported compile-time value materialization | Unsupported value / materialization |
| `compile_error(message)` reached | Explicit compiler diagnostic with message |
| Executed forbidden host operation | Comptime capability/policy violation |
| Bodyless/extern function reached | Nonexecutable body / extern boundary |
| Fuel exhausted | `FuelExhausted` |
| Call-depth bound exceeded | `CallDepthExceeded` |
| Arithmetic error | Controlled overflow / division-by-zero diagnostic |
| Illegal borrowed escape | Owner/provenance/escape violation |
| Illegal implicit copy of unique resource | Copyability/ownership violation |
| Failed or inconsistent specialization | Concrete specialization identity and location |

Source spans, module identity, specialization inputs and reachability context should be retained when known. A compilation failure must not install a partial or misleading materialized constant, leave owned resources live beyond their valid scope, or publish an artifact as trusted.

The exact diagnostic wording remains implementation-specific; the semantic distinctions and deterministic precedence within the versioned supported subset must remain testable.

## 15. Non-goals and deferred features

This revision does **not** introduce or certify:

- Arbitrary compile-time filesystem, network, environment, process, random, time or host-OS access.
- Unsafe foreign calls, raw pointers or FFI-backed resource ownership at compile time.
- Arbitrary runtime-to-comptime promotion of immutable locals.
- Persistent borrowed slices or dynamic borrowed aggregates embedded in generated runtime values.
- Nonprimitive materialization beyond the historical primitive baseline; general arrays/structs/enums remain an expansion and test task.
- Full generated specialization/erasure of comptime parameters as a documented completed feature.
- General constructor/tuple/struct patterns in `comptime match` beyond supported and verified coverage.
- Explicit lifetime syntax, implicit `consume`, a new memory manager or a new effects mechanism.
- General actor/task execution or suspending dynamic borrows in comptime.
- `@unsafe` or `@verified` source syntax as implemented.
- A completed VM-backed comptime migration, Tempra Native LLVM implementation or formal proof of RC-free memory management.

Actor-handle lifetime and representation remain **DEFERRED POST NO-CYCLE** in the Memory v0.4 program and are not a comptime requirement.

## 16. Conformance matrix

The expected outcome in this table is a **language/specification contract** unless the row explicitly states a K1 historical implementation. It is not proof that a current VM already passes the case.

| Case | Expected behavior |
|---|---|
| `comptime 1 + 2` | Evaluates to a compile-time constant |
| `comptime { let a = 1; a + 2 }` | Evaluates the block result |
| Runtime local within comptime | Rejected, even if bound with `let` |
| Named function comptime parameter with literal | Accepted under supported specialization contract |
| Named function comptime parameter with runtime local | Rejected |
| Closure declared with `comptime` parameter | Rejected under documented v0.1 surface |
| `if comptime` selected branch valid | Accepted; only selected branch typechecked |
| Unselected `compile_error` | Does not fail the specialization |
| Selected `compile_error` | Controlled compiler error |
| `comptime match` type pattern | Selects the matching arm when known |
| Unmatched compile-time match with no fallback | Rejected |
| `cmd(...)` without execution | Allowed in historical restricted policy |
| `cmd(...)` execution or host I/O | Rejected |
| Bodyless extern reached | Rejected; never synthetic `Unit` |
| Nested comptime fuel | Same shared budget |
| Tick after fuel reaches zero | `FuelExhausted` |
| Ninth active Tempra frame, historical default | `CallDepthExceeded` if fuel not already exhausted |
| Checked `Int64` overflow | Controlled error |
| Borrowed slice escaping evaluator memory | Rejected |
| Copy of unique owned resource without explicit transfer | Rejected |
| `fn[self]` | Cannot create a strong ownership cycle |
| Materialization of unsupported complex value | Controlled unsupported result; no dangling storage |
| Semantic cache revision invalid | Recompile under current contracts |
| Tempra Script VM and historical HIR shared corpus | Equivalent values, diagnostics and side-effect rejection before VM promotion |

## 17. Tempra Script VM migration acceptance gates

These gates are **future work** and are not presumed complete simply because VM parity is being developed elsewhere.

1. **Compilation topology:** Demonstrate that all currently supported `comptime` forms can be evaluated at the correct typechecking/specialization stage without relying on an unverified fully materialized runtime TIR module.
2. **Coverage parity:** Run the historical K1 evaluation corpus and focused multi-module/generic/comptime-branch tests on both kernels for the actually shared subset; classify unsupported features explicitly.
3. **Arithmetic and error parity:** Check all controlled integer failures, float boundary behavior, bodyless extern rejection, `compile_error`, and selected/unselected branch semantics.
4. **Policy parity:** Exercise A/B/C/D operation classes, including `dbg`, command construction vs execution, native extern attempts and transitive calls. No host-execution bypass.
5. **Budget parity:** Preserve documented semantic tick contract, nested session accounting, exhaustion precedence and call-depth behavior, or approve a separately versioned deliberate change.
6. **Ownership parity:** Apply Memory v0.4 to copyability, `ref`/`inout` aliasing, dynamic borrowed escapes, closure environments, unique resource cleanup and valid result materialization.
7. **Capability parity:** Preserve `@deny`/`@pure` and specialized-effect restrictions across runtime and compile-time boundaries, including any alloc/cleanup attributable to lowered operations.
8. **Determinism:** Confirm canonical specialization/diagnostic iteration and K2-style cross-process byte-for-byte artifacts under the declared same-build/host/target envelope.
9. **Cache safety:** Reevaluate semantic trust requirements, bump the revision when needed, reject old hits that can bypass new guarantees, and verify corrupted/obsolete cache fallback.
10. **No fallback ambiguity:** Explicitly promote the VM kernel as the single canonical comptime authority after tests pass. Retire or clearly mark the old HIR path as historical/test-only; no silent mixed execution.
11. **Lifecycle/error cleanup:** Prove no dangling temporary evaluator values, double release or host-side resource leak on normal return, type error, explicit compiler error, fuel exhaustion and cancellation of compilation.
12. **Performance:** Measure compile latency and peak memory with the K5/K6 methodology. Preserve Tempra Script's scripting-first cold-start goal without replacing semantic correctness with a fragile optimization.

Each gate needs repository evidence: fixture names, test commands, commit SHA, supported subset, observed result and known gaps. The gate is not closed by this document's publication.

## 18. Implementation checklist (not part of the public language)

The compiler/VM integration should record at least the following test families:

```text
comptime/primitive_eval
comptime/blocks_and_scopes
comptime/runtime_local_rejection
comptime/parameter_specialization
comptime/type_values
comptime/if_pruning
comptime/match_pruning
comptime/compile_error
comptime/arithmetic_errors
comptime/host_operation_policy
comptime/bodyless_extern_rejection
comptime/nested_fuel_and_depth
comptime/multi_module_identity
comptime/deterministic_specialization
comptime/memory_borrow_escape
comptime/resource_copy_and_cleanup
comptime/runtime_capability_contracts
comptime/cache_revision_and_reproducibility
```

Names are illustrative fixture groups, **not** a claim that tests or files already exist. Add differential execution tests only for operations both kernels actually support; preserve unsupported-case tests to prevent accidental host behavior.

## 19. Historical notes and migration log

- The original `COMPTIME_SPEC.md` recorded K1's canonical HIR interpreter, deterministic evaluator budget, controlled arithmetic, bodyless extern rejection and effect policy.
- K2 established deterministic specialization, module ordering, explicit stdlib input and bounded cross-process reproducibility in the documented bytecode scope.
- K3/K4 improved arithmetic parity, cache trust and specialized effect enforcement without making the HIR kernel an eternal language requirement.
- The Memory & Ownership v0.4 decision adds one normative memory model for all phases, including comptime, with conservative effects and unique resources.
- The proposed removal of JIT and Cranelift is a separate implementation operation; this document removes them from the forward execution plan but does not assert repository code removal.
- The VM-first migration remains pending certification; its rollout must preserve the semantic contracts above.

## 20. Final principle

> **Comptime changes *when* a computation is evaluated, not *what* values, ownership, borrowing, effects or resource behavior Tempra permits.**
>
> Compiler evaluation, Tempra Script VM and future LLVM AOT must share one source-level language semantics. A performance shortcut, backend representation or cache hit can never authorize behavior forbidden by the checked Tempra program.
