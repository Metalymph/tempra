# Tempra — Tempra Script Bytecode VM

**Documentation revision:** v0.2 — 16 November 2026
**Status:** Consolidated VM design and historical implementation baseline. VM parity, ARC NO-CYCLE and migration to the default runtime are **not certified by this document**.
**Scope:** Verified stack-bytecode execution, frames and values, calls, memory and effects, error handling, bytecode lowering, caching, profiling and transition gates.
**Source reviewed:** `Metalymph/tempra`, GitHub `main` (`b7e95a06aa5b28cc43d92c81174aa43fb7476c9f`). The local `feature/k5-compiler-timings` branch was unavailable remotely; recheck all implementation claims and CLI examples against its eventual commit before merging.

## 1. Authority and status

This document describes **how Tempra Script's stack VM executes Tempra**, not an independent language semantics.

Normative dependencies:

- [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md): **sole authority** for value semantics, copyability, borrowing, ownership transfers, resource destruction, NO-CYCLE, async boundaries and runtime/backend memory equivalence.
- [Surface Syntax v0.2](TEMPRA_SURFACE_SPEC_v0.2.md) and [Grammar](GRAMMAR.md): source-language spelling and currently documented/surface-open constructions.
- [Comptime Specification](COMPTIME_SPEC.md): compile-time evaluation, policy, determinism and the future VM evaluation gate.
- [Capability Contracts](specs/effects-v0.1.md) and [Specialized Effects v0.2](specs/specialized-effects-v0.2.md): `@deny`, `@pure`, capability propagation and operation-specific restrictions.
- [Bytecode ISA](BYTECODE_ISA.md), [Bytecode Cache](BYTECODE_CACHE.md), [TIR Specification](TIR_SPEC.md), [TIR to Bytecode](TIR_TO_BYTECODE.md): the corresponding encoding, verification and lowering boundaries.
- [Tempra Script release plan](roadmap/script.md) and [VM Coverage](VM_COVERAGE.md): sequencing and historically measured feature coverage, **not** authority to weaken semantics.

**Status labels used here:**

| Label | Meaning |
|---|---|
| **Normative contract** | Required behavior for accepted Tempra programs |
| **Documented `main` baseline** | Behavior recorded by the repository documentation at the cited GitHub revision; not newly certified here |
| **Target** | Approved architectural direction, awaiting implementation/parity evidence |
| **Deferred** | Details deliberately not settled by this specification |
| **Historical** | Description of an older implementation or experiment; not a future architectural requirement |

If the present code differs from a normative contract, that is a **conformance gap** to diagnose and fix, not permission for the VM to define different Tempra rules.

## 2. Architecture: actual baseline and target

### 2.1 Documented baseline on GitHub `main`

The repository's previous documentation describes two execution paths:

```text
Source -> Lexer -> Parser -> HIR -> Type checking -> HIR interpreter
                                                       (default, historical)

Source -> Lexer -> Parser -> HIR -> Type checking -> TIR lower
       -> TIR verifier -> Bytecode lower -> Bytecode verifier
       -> Tempra Script stack VM                                   (opt-in, historical)
```

At the reviewed baseline, `tempra run --experimental-vm file.tmpr` selected the latter path. That flag is documented for **historical/current-branch verification only**; it is not part of the final public CLI contract. The HIR evaluator and bytecode VM had different supported language subsets.

Neither the existence of a VM implementation nor passing selected differential tests establishes full language parity, correct ARC NO-CYCLE ownership, or a release-ready default interpreter.

### 2.2 Target execution architecture

```text
Source
  -> Lexer / Parser / module resolution
  -> HIR / type checking / specialization
  -> ownership, provenance, exclusivity, escape and capability facts
  -> TIR lowering -> TIR verification
  -> bytecode lowering / local optimization -> bytecode verification
  -> Tempra Script stack VM
```

The target has **one ordinary scripting execution engine: the bytecode VM**. The transition to default `tempra run` requires the gates in §17. Unsupported features must produce controlled diagnostics; a silent HIR fallback must not bypass ownership or capability enforcement.

The compiler may retain narrowly scoped evaluators or testing facilities during migration; these must not become an undocumented second general runtime.

### 2.3 No JIT or Cranelift in the target

**Decision:** Remove the experimental JIT and Cranelift, rather than merely freeze further JIT development. Neither is an optimization stage, dispatch tier, requirement or fallback of the target Tempra Script runtime.

The older Cranelift-JIT descriptions and test procedures are preserved in [`archive/jit/`](archive/jit/). Their availability does not make them normative. **Moving documentation does not remove code**: crates, Cargo features, options, CLI flags, tests, benches and CI references must be removed or migrated by a separately validated implementation change. Until then, references to those facilities describe *legacy code only*.

Tempra Native is a **separate planned LLVM AOT backend**, subject to the same memory/effect contracts; this document does not specify LLVM code generation.

## 3. Runtime execution model

### 3.1 Stack architecture

The VM is a stack-based interpreter. Functions have indexed local slots and a per-frame operand stack. The instruction loop executes verified bytecode; branch instructions update the instruction pointer, and `Call`/`Return` manage frame transitions.

The stack machine is an execution representation, **not** a source-level evaluation-order override. It must preserve the source language's applicable rules for expression evaluation, side effects, errors and cleanup.

### 3.2 Frames

The documented v0 `Frame` holds:

- **Function identifier** referring to the bytecode function (not an arbitrary array offset mistaken for a semantic ID).
- **Instruction pointer** indicating the next instruction.
- **Locals**, historically represented as `Option<VmValue>` slots: supplied arguments start initialized, other slots start unavailable.
- **Operand stack** containing intermediate `VmValue` values.

The historical implementation pre-allocates operand stack capacity using `max_stack` metadata and reserves a baseline call-stack capacity. It initializes callee locals from the caller's operand stack without constructing an intermediate argument vector in its documented fast path.

These are **implementation strategies**, not public guarantees of stack allocation, fixed address, zero copying or zero heap allocation. `max_stack` must be verified rather than blindly trusted.

### 3.3 Calls and returns

The existing bytecode convention evaluates/pushes arguments left to right. `Call` consumes the required argument count, initializes callee parameters, and transfers control. `Return` passes zero values for a `Unit` return or one value for a non-`Unit` return. The top-level result is delivered to the host/CLI adapter.

The **language-level** call contract is independent of physical stack operations:

| Source contract | Meaning | Lowering requirement |
|---|---|---|
| Ordinary `T` parameter | Value semantics for duplicable values; caller's value remains semantically valid | Materialize an independent value or perform an observationally equivalent last-use physical move |
| `ref` | Non-owning read access | Preserve provenance, lifetime and alias compatibility |
| `inout` | Exclusive mutable non-owning access | Preserve exclusivity and underlying-memory mutation |
| Explicit `consume` | Semantic ownership transfer; source invalidated | Mark the source consumed; preserve unique ownership and cleanup obligations |

`consume` is already established for closure captures; its **surface spelling on ordinary parameters and receivers remains subject to the Surface/Grammar alignment**. The VM must not invent an implicit consuming mode for a nonduplicable resource just because an optimizer detects its last use.

For script/application entry points, the documented CLI `main` adapter treats `Unit` and `Result::Ok` as success, uses an `Int` return value as a portable exit code in `0..=255`, and reports `Result::Err` as a controlled uncaught error. The exact active-branch adapter behavior must be regression-tested; it is not a property of the generic `Return` opcode.

## 4. Runtime values and ownership

### 4.1 Historical `VmValue` representation

The old `BYTECODE_VM.md` records these representations or categories:

| Value | Documented v0 representation / role |
|---|---|
| `Unit`, `Bool`, `Int64`, `UInt64`, `Float64` | Primitive runtime variants |
| `String` | UTF-8 string value |
| `Enum` | Enum name, variant name and payload list; used by `Option`/`Result` |
| `Tuple` | Positional sequence of values |
| `Array` | Historically `Rc<Vec<VmValue>>` |
| `Slice` | Parent plus start/end descriptor |
| `Struct` | Named fields and values |
| `Any` | Internal verification/runtime accommodation, **not** unrestricted public dynamic typing |

This table is an **implementation snapshot**, not a prescribed final layout. It does not establish the complete public numeric-type coverage or prove ARC NO-CYCLE. In particular, a Rust `Rc` field, a heap handle or an ordinary array clone cannot determine whether two Tempra values are allowed to alias or whether their mutations are independent.

### 4.2 Normative value semantics

Memory v0.4 requires:

1. **Duplicable owned values** are semantically independent after copy. A mutation through one value must not mutate another semantic copy.
2. **Nonduplicable owned resources** (for example a file owner) are not silently duplicated. A containing struct, enum, collection or closure inherits relevant copyability restrictions.
3. **Physical moves/reuse** may replace copies when the previous value cannot be observed; this does not imply source-language `consume`.
4. **Explicit `consume`** invalidates the source binding and transfers the relevant ownership, regardless of physical implementation.
5. **No strong owner cycles** are permitted in the safe-language ownership graph. The goal is ARC **NO-CYCLE** for Tempra Script; implementation and formal soundness are separate gates.
6. **No hidden ownership promotion**: the VM cannot copy a borrowed value into an owned buffer solely to make an invalid escape legal.

The runtime may choose copy-on-write, unique-buffer reuse, direct copying or other physical layouts **only when the observable behavior and applicable effect contracts are preserved**.

### 4.3 Borrowed access and slices

- `Slice(T)` is a **read-only, contiguous, non-owning view**; it never independently owns or keeps alive the source buffer.
- `inout Slice(T)` denotes exclusive mutable access to underlying elements, not merely reassignment of a slice descriptor.
- The compiler tracks conceptual provenance `Static`, `Borrowed(owner)` and `Temporary(scope)`; neither `let` nor `comptime` automatically implies `Static`.
- An eligible returned borrowed slice may depend on **one identifiable source parameter** under Memory v0.4; arbitrary alternate owners and escaping aggregates are not allowed.
- A `Range` is an independent, half-open `[start, end)` value. It describes an interval, does not keep a buffer alive and is bounds-checked when applied.
- Borrow liveness follows last reachable use across branches, loops and calls, subject to conservatively checked joins and alias conflicts.

**Verifier requirement:** executing a structurally well-formed `Slice` opcode is not evidence that the slice may escape, be stored, or survive its owner. Ownership/provenance constraints must be proven before execution and retained in verifiable intermediate contracts.

### 4.4 Resource release and cleanup

Owned resources must be released **once**, and never while referenced by a still-active runtime operation. Cleanup obligations apply to normal returns, error propagation and **completed** cancellations. A transfer removes the previous owner's right to release the resource. An automatic non-failing release is possible; a failure that is part of the public `close()` contract must not silently disappear.

If cleanup requires I/O, process or another capability, its effects must be included in the applicable requirements. Bytecode lowering cannot hide such effects from the checker.

**No promise:** cleanup following an uncatchable process kill, host crash or unsupported cancellation path.

## 5. Bytecode verification and execution authority

### 5.1 Existing structural verifier boundary

The historical v0 verifier checks, or is designed to check, at least:

- opcode and operand validity;
- constant, function and local-slot bounds;
- jump targets at valid instruction boundaries;
- consistent stack shape/height across paths, under `max_stack`;
- instruction operand types and return shapes;
- local initialization before load;
- call arity/signature compatibility.

**Verifier-first is mandatory:** decode bytes, verify the module, then execute. Deserializing a cache artifact alone is not authorization to execute it. Malformed or incompatible artifacts must never silently become trusted input.

### 5.2 Ownership-aware verification: target

Memory v0.4 defines additional **conceptual invariants** for checked TIR/bytecode boundaries:

| Concept | Invariant to preserve |
|---|---|
| Value copy | Only valid for duplicable values; semantic independence maintained |
| Transfer/consume | Source may not be used after semantic transfer; owner released once |
| Read borrow | Owner/provenance valid until last use |
| Exclusive borrow | No simultaneous incompatible access |
| Borrow end | CFG-compatible termination; no illegal escape |
| Closure environment | Explicit captures and no owner cycle/illegal borrowed escape |
| Suspend/resume | No dynamic borrow alive across suspension |
| Drop/close | Correct ownership, pending-I/O synchronization and capability accounting |
| Call summaries | Borrowed results, callback escapes, ownership and effects preserved across modules |

These names are **semantic categories, not new VM opcodes**. Whether a particular fact is encoded in TIR, metadata or verified lower-level instructions is an implementation decision. The ordinary stack verifier must not claim these checks are already complete because it validates stack depth.

A verifier should check normalized facts and local CFG invariants, rather than reimplement the full source-language borrow checker. A change in the trusted semantics must invalidate caches that lack those guarantees.

## 6. Errors and controlled failure

The VM reports controlled `VmError` outcomes rather than relying on Rust panics for valid input. The documented historical categories include:

- checked signed/unsigned arithmetic failures, including overflow/underflow and division by zero;
- `StackUnderflow`, `TypeMismatch` and similar conditions indicating invalid bytecode or broken invariants;
- `UnreachableExecuted` when an invalid/reachable trap instruction is executed;
- call/entry, native boundary, capability and other structured failures as required by the current supported operations.

### 6.1 Numeric semantics preserved across paths

- `Int64` addition, subtraction, multiplication and division use checked semantics. Signed overflow includes `Int64::MIN / -1`; division by zero is a typed controlled failure.
- The documented `UInt64` arithmetic kernel checks overflow, underflow and division by zero on its supported operations.
- `Float64` follows the documented IEEE-754 behavior for NaN, infinities, signed zero and ordered comparisons; it must not be accidentally lowered through signed integer operations.
- Where arithmetic may fail conservatively, `@deny(panic)` must be enforced according to the capability contract. An optimizer may not silently change or remove observable errors.

Source-language `Result` and `Option` remain ordinary ADT values. The `?` operator is lowered with explicit control flow; it is not an exception mechanism and must retain correct cleanup on early exit.

## 7. Source feature coverage

The following is a **historical coverage summary** drawn from `BYTECODE_VM.md` and `VM_COVERAGE.md` on GitHub `main`. It must be checked again on the actual post-parity branch before release.

| Area | Documented historical status | Required follow-up |
|---|---|---|
| Core arithmetic, logical operations, functions, `if` | Exercised on VM | Recheck complete numeric contracts and diagnostics |
| Arrays, slices, tuple construction, basic enum/struct operations | Implemented over bounded subsets | Enforce Memory v0.4 provenance, copyability, exclusivity and cleanup |
| `for` and `for` expressions | Recorded as differential-tested | Check all iteration and borrowed-state cases |
| `Result`, `Option`, `?`, `is` | Recorded as supported over relevant cases | Check propagation cleanup, pattern coverage and borrowed returns |
| `match` | Simple literal/wildcard/variant arms documented | Guards, nesting and full parity require revalidation |
| `extend` methods / `self` | Basic forms recorded as supported | Check receiver ownership semantics, including `inout self` |
| Closures and captures | Complex/recursive cases recorded as incomplete or unstable | Implement explicit `ref`/`inout`/`consume`, escapes and acyclic `fn[self]` |
| String iteration | Documented gap | Implement and parity-test if included in declared release surface |
| Generic methods / specializations | Documented limited subset | Verify concrete specialization identity and ownership summaries |
| CLI `main` arguments | Documented gap | Validate entry behavior and diagnostics |
| Effects/specialized denials | Static enforcement exists for a documented bounded VM surface | Verify coverage against actual emitted operations and cached hits |
| ARC NO-CYCLE and nonduplicable resources | Memory v0.4 **design**, not proven by old coverage table | Separate M/M5 implementation, differential and safety gates |
| Async tasks, actors, cancellation | Memory semantics approved; execution specifics incomplete | Do not claim operational support or actor-handle policy |

**Do not** reuse old counts such as “19 differential tests” or a fixed opcode count as a present-day certification without rerunning the exact test suite on a pinned commit.

## 8. Bytecode lowering and local optimization

The path from verified typed TIR to bytecode should prioritize deterministic and correct lowering. The documented v0 optimizer includes:

1. **Stack/local placement:** allocate local slots and compute `max_stack` for each function.
2. **Peephole optimization:** identify sequences such as `StoreLocal(temp)` followed by `LoadLocal(temp)`.
3. **Temporary-only elimination:** do not casually remove user locals/parameters; preserve control-flow, diagnostics and debug mapping.
4. **Jump fix-ups:** rewrite jump targets correctly after removing/replacing instructions.
5. **Reverification:** re-verify optimized functions; use an unoptimized verified path if an optimization is inapplicable or its output fails verification, without accepting malformed program semantics.
6. **Superinstructions:** fuse hot instruction patterns only when side effects, error ordering, value independence and verifier guarantees remain equivalent.

Optimization is **not** a source-level waiver: an optional copy-elimination, reuse or superinstruction pass cannot be the sole reason an otherwise potentially allocating operation satisfies `@deny(alloc)`.

### 8.1 Copy versus last-use physical move

The checker decides whether a copy is permitted and whether a semantic `consume` occurs. The lowerer may exploit a proven last use to reuse storage or move a buffer physically, but must preserve:

- observable independence for ordinary duplicable by-value values;
- mutation exclusivity and borrowed-view validity;
- semantic invalidation after explicit `consume`;
- single release of a nonduplicable resource;
- the same required capabilities (including potential `Alloc` and cleanup) on all supported backends.

No public size threshold dictates copying versus moving. Small aggregates may be cheaper to pass in registers; large buffers may benefit from reuse. These are ABI/optimizer choices.

## 9. Bytecode cache and reproducibility

The v0 bytecode cache is a **compiler-private artifact**, not a stable public ABI. See [BYTECODE_CACHE.md](BYTECODE_CACHE.md) for its exact format and validation rules.

The reviewed historical documentation records:

- `.tempra/cache/` as the CLI cache location;
- format version `2`, distinct from the semantic trust revision;
- `semantics-8` as the then-current compiler fingerprint revision on `main`;
- cache decoding **followed by verification** before execution;
- cache misses, version mismatches or corruption triggering fresh compilation rather than execution of untrusted bytes;
- deterministic bytecode/cache behavior under the bounded K2.5 same-compiler/same-host/target contract for its supported fixtures.

**Do not invent a new current fingerprint:** verify the semantic revision on the active branch before changing this document or fixtures. Changes that add or alter trusted ownership/borrow/resource checks **must** invalidate older cache artifacts unless their guarantees can be soundly re-established. A bytecode-format bump is a separate decision from a semantic-trust bump.

A cache hit must not evade `@deny` enforcement, ownership verification, specialized-effect contracts or stale-module checks. A failed fresh compile must not be turned into a successful execution via an old cache artifact.

## 10. Capability enforcement and `@deny(alloc)`

Capability contracts remain defined by the capability specifications. The target VM must enforce direct and transitive requirements, including specialized operation/family/category restrictions, on the executable surface and at its compiler-owned trust boundary.

Memory v0.4 strengthens the allocation rule:

- A copy of an inline scalar/aggregate need not require dynamic allocation when its admitted lowering does not allocate.
- An independent copy of a dynamic buffer, a new owned compact payload, or a heap-resident closure environment can potentially require `Alloc`.
- A pure transfer of ownership via `consume` does not require `Alloc` *solely because of transfer*, though the surrounding operation may allocate.
- A function cannot satisfy `@deny(alloc)` only because one backend or an optional optimization sometimes elides an allocation.
- VM heap/runtime helpers and cleanup cannot hide operation-level alloc/IO/process effects that the source contract prohibits.

`@pure` is not allocation-free: it excludes IO, Process, Spawn and NonDeterminism, but may allocate or fail conservatively with Panic.

**Status:** the reviewed effect implementation claims apply only to its documented supported TIR/VM subset. This document does not declare complete VM-wide `@deny(alloc)` certification or retroactively certify an older runtime representation.

## 11. Closure execution

The language syntax already defines capture lists of the forms:

```tempra
let offset = 10
let add = fn[ref offset](x: Int) -> Int { offset + x }

let data = load_data()
let take = fn[consume data]() { process(data) }

let factorial = fn[self](n: Int) -> Int {
    if n <= 1 { 1 } else { n * self(n - 1) }
}
```

The examples illustrate **source contracts, not current VM parity**.

- **No capture default:** all external variables must be named with a mode (`ref`, `inout` or `consume`). A closure without captures uses `fn(...)`, not `fn[]`.
- `ref`/`inout` dynamic captures may be used only while the borrowed access is valid and the closure is proven non-escaping; a static immutable reference has its separately proven exception.
- `consume` transfers ownership into the environment and invalidates its source binding. Escaping owned environments are legal only when their components satisfy ownership/copyability/NO-CYCLE constraints.
- `fn[self]` is explicit recursion, **not** a fourth capture mode; its implementation must not establish a strong self-ownership cycle.
- The environment's layout, stack/heap placement and calling convention are VM implementation details, not new language semantics.
- Imported callback escape summaries must be enforced; unknown escape is treated conservatively.

## 12. Async, tasks and actors — approved safety constraints, deferred runtime details

The Memory v0.4 contract applies to eventual VM async instructions and continuations even though their concrete bytecode representation is **not specified here**:

- A live **dynamic** `ref`, `inout`, `Slice(T)` or closure capturing such access cannot cross `.await()`.
- An owned value, `Range` or truly static immutable view may cross suspension under the usual constraints.
- Spawned tasks receive autonomous owned values or permitted static immutable access, **not** captured dynamic external borrows — including under structured concurrency in v1.
- Actor mutable state is isolated; actor messages convey independent values, not borrowed dynamic slices.
- A continuation requiring a `Reader`, `Writer` or buffer during suspension must own the necessary state. A last-use physical move does **not** authorize implicit semantic consumption of a unique external resource.
- Cancellation requests do not authorize release of buffers still used by pending OS I/O. Release occurs after completion, effective cancellation or safe ownership handoff.
- **Actor-handle lifetime and identity are DEFERRED POST NO-CYCLE.** Do not choose a strong-reference or identifier policy here.

No `Spawn`, `Suspend`, `Drop` or ownership-specific opcode naming is settled by this document.

## 13. Profiling, tracing and performance

The historical VM provides optional call, instruction and loop-backedge counters; opcode histograms and superinstruction counts are documented for its then-current opcode set. Such telemetry remains useful **without any JIT** for:

- interpreter dispatch and instruction-mix analysis;
- cold-start, throughput and peak-RSS baselines;
- identifying frequent bytecode operations and validating superinstructions;
- detecting regressions across compiler versions;
- informing later *VM-only* optimizations.

The historic `hot` label or thresholds may remain a profiling display field if the implementation retains them, but **must not trigger native compilation** in the target. Remove JIT compilation counters, JIT rejection/fallback statistics and feature-switch documentation from the living VM specification after the implementation cleanup. Preserve their history in the archive.

K5/K6 performance gates require pinned commit, compiler/build profile, OS/CPU, workload, measurement method and raw results. This document asserts no benchmark numbers or performance superiority over Python, JavaScript or native code.

## 14. Stdlib/native boundary

The supported VM can dispatch selected standard-library/native functions through an explicit registry. A native function is **not** automatically safe or capability-free simply because its implementation is Rust.

- Validate signature and arity at the boundary.
- Attribute effects to their actual operation (I/O, process, alloc, panic, nondeterminism, etc.).
- Do not expose raw foreign pointers as valid Tempra borrows without an authorized safe contract.
- Follow nonduplicable resource ownership, deterministic cleanup and pending-I/O pinning/stability requirements.
- Preserve explicit errors; a bodyless or unsupported call must not silently manufacture `Unit`.
- Foreign libraries may use their own memory managers, but the safe wrapper must preserve the language's external invariants.

The `@unsafe` source boundary and raw FFI ABI remain separately deferred. No wrapper in this document grants general unsafe authority.

## 15. `comptime` on the VM — planned migration

The historical K1 canonical kernel is the restricted HIR interpreter. The [revised Comptime Spec](COMPTIME_SPEC.md) defines the target of executing compile-time computations through the **same Tempra Script VM semantics**, with a restricted execution profile.

Migration requires verified parity for:

1. Supported compile-time values, type-directed selection and specialization.
2. Deterministic policy rejection of filesystem/host I/O, process, environment, nondeterminism and unsupported foreign calls.
3. Fuel/progress and call-depth limits: counting bytecode instructions instead of historical semantic progress is **not automatically equivalent**.
4. Nested sessions, module-aware lookup, controlled errors and materialization before ordinary runtime emission.
5. Memory v0.4 ownership, reference provenance, cleanup, effect accounting and cache invalidation.

Evaluation occurs **during** checking and specialization, so the design cannot assume an already fully checked runtime TIR module for every comptime expression. A restricted lowering adapter or staged VM input may be necessary. It remains an implementation decision.

Do not claim VM-backed comptime is complete until its conformance gates pass; do not silently fall back to the HIR evaluator on code advertised as VM-backed.

## 16. Conformance and regression matrix

| Case | Required result / gate |
|---|---|
| Decode malformed bytecode | Controlled rejection; never execute unverified content |
| Different CFG predecessors with incompatible stack states | Verifier rejection |
| Load uninitialized local | Verification or controlled runtime rejection |
| Call with wrong arity or bad return stack | Verifier rejection |
| `Int64`/`UInt64` checked arithmetic | Controlled typed errors, no accidental wrapping or host panic |
| `Float64` NaN, signed zero, infinities | IEEE-compatible results on supported operations |
| By-value duplicate and subsequent mutation | Original semantic value unchanged |
| Last-use optimization | Same result and effects as unoptimized compilation |
| `consume` on unique owner | Source invalidated, exactly one owner/release |
| Borrow after owner destruction | Static checker/verifier rejection |
| Two incompatible `inout` aliases | Static checker/verifier rejection |
| Borrowed dynamic slice escaping into persistent aggregate | Rejection; no implicit owning copy |
| Dynamic borrow live over suspend | Rejection |
| Closure `fn[self]` | Correct recursion without a strong self-cycle |
| Failure/early return | Correct resource cleanup, no double release |
| Completed task cancellation | No release while I/O still uses storage |
| `@deny(alloc)` on potentially allocating copy | Conservative denial unless a sound guarantee exists |
| `@pure` and specialized denials | Capability and operation checks preserved |
| Cold/warm cache and stale semantic revision | Same accepted program behavior, invalid cache rejected |
| HIR-to-VM parity transition | Same supported result/error/diagnostic semantics; no hidden fallback |
| VM-backed comptime promotion | Preserve deterministic budget, effect policy and memory invariants |
| Removed JIT/Cranelift code | Workspace, CLI, docs and CI free of active JIT paths; historical archive retained |

The table expresses **required tests**, not a statement that every test already exists or passes.

## 17. Promotion and cleanup gates

Before this document can describe Tempra Script VM as the current authoritative scripting runtime:

1. **Pin the implementation commit.** Record actual branch/SHA and working-tree state; this document's GitHub `main` baseline must not be mistaken for the local K5 branch.
2. **Establish VM language parity** on the declared release surface, including closures, generics, pattern guards, string iteration, entry arguments, native calls and controlled failures where promised.
3. **Enforce Memory v0.4** through the frontend, IR contracts, bytecode verifier/runtime representations and destruction paths; record which guarantees remain future gates.
4. **Enforce capability contracts** on every supported execution and cache path, without accidentally bypassing `@deny` or specialized restrictions.
5. **Verify cache and compiler identity**, including semantic fingerprint bumps for any newly trusted memory/effect guarantee and deterministic bytecode output.
6. **Separate comptime promotion.** Preserve HIR comptime authority until the restricted VM kernel passes its own parity/termination/effect gates.
7. **Remove legacy JIT and Cranelift implementation** in a separate code change. Remove or replace feature flags, crates, hotness-driven promotion, CLI knobs, examples, tests, dependency locks and CI hooks; keep archived documents only.
8. **Measure K5/K6 performance** on reproducible workloads, including startup, cache hit/miss, execution cost and RSS.
9. **Update supporting docs** (`BYTECODE_ISA.md`, `TIR_SPEC.md`, `TIR_LOWERING.md`, `VM_COVERAGE.md`, `VM_PERF_BASELINE.md`, architecture, roadmap and developer guides) based on the resulting exact commit.
10. **Promote the default VM only after gates pass.** Where behavior is unsupported, fail explicitly rather than silently run through a less-checked execution path.

## 18. Deliberate non-goals

This document does **not**:

- introduce a second ownership, copying, lifetime or exception model for the VM;
- guarantee RC-free execution in Tempra Script (its planned model is ARC NO-CYCLE);
- declare ARC NO-CYCLE, lifetime proofs, fully sound async borrowing or all capabilities implemented today;
- promise that an optimization, COW path or implementation-specific `Rc` clone is allocation-free;
- provide a public stable bytecode ABI or permit arbitrary untrusted bytecode execution;
- specify the exact IR/opcodes for future borrowed values, closures, tasks, actors or resource drops;
- decide the actor-handle lifetime model (**DEFERRED POST NO-CYCLE**);
- reintroduce JIT or Cranelift as a VM component or as an AOT backend choice;
- claim an LLVM AOT backend already exists or that its RC-free research goal is proven;
- mark VM-backed `comptime` as complete without a separate measured parity gate.

**Bottom line:** One Tempra memory model, one target Tempra Script scripting VM, one checked compiler pipeline. Optimizations may change physical representation, never source semantics.
