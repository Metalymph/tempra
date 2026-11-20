# Tempra — Compiler and Execution Pipeline

**Documentation revision:** v0.2 — 2026-11-16
**Scope:** compiler stage ordering, validation and execution boundaries
**Status:** observed GitHub `main` baseline plus approved VM-first migration; future gates are marked explicitly.

## 1. Reading this document

The [Architecture](ARCHITECTURE.md) defines component ownership. This document specifies **how a compilation request flows across those components**, which guarantees each boundary establishes, and which operations must fail closed. It is not a bytecode or source grammar specification.

References: [Memory & Ownership v0.4](../Tempra_Memory_Ownership_v0.4.md) (sole memory authority), [TIR](../TIR_SPEC.md), [TIR Lowering](../TIR_LOWERING.md), [Bytecode ISA](../BYTECODE_ISA.md), [Bytecode Cache](../BYTECODE_CACHE.md), [Comptime](../COMPTIME_SPEC.md), and [Capability Contracts](../specs/effects-v0.1.md) / [Specialized Effects v0.2](../specs/specialized-effects-v0.2.md).

The feature branch `feature/k5-compiler-timings` is not represented by the inspected remote `main` tree. All implementation statements below describe **that inspected baseline**, not an audit of unpushed changes. A contract labeled **TARGET** is not claimed to be implemented.

## 2. Stages and data products

| Stage | Input | Output | Guarantee or limitation |
| --- | --- | --- | --- |
| Lex/parse | Source bytes | Tokens, AST, spans | Syntax, not type/effect/memory safety |
| AST → HIR | AST, module context | HIR | Lowered source structure; some paths differ in coverage |
| Type checking | HIR, imports, generics | Typed HIR and type context | Type/specialization checks, not full E3/K4 enforcement on frontend-only paths |
| Comptime (current) | Eligible typed/partially typed HIR | Resolved values, selected HIR branches | K1 HIR evaluator, effect policy and fuel; **not** current VM-based comptime |
| TIR lowering | Supported concrete typed HIR | `TirModule` | Unsupported source forms fail with diagnostics |
| TIR verification | TIR | Verified typed TIR | Structural/types/numeric consistency; **not** full Memory v0.4 proof |
| Capability analysis | Verified TIR + extern classification | Direct/transitive summaries, call graph, witnesses | E1/E2 bounded TIR coverage |
| Contract enforcement | Summaries + concrete source contracts | Accept or violation | Broad `@deny`/`@pure` then specialized restrictions |
| Bytecode lowering | Verified TIR/types | Stack bytecode | Type-appropriate instructions and deterministic function map |
| Bytecode verification | Bytecode | Executable verified artifact | Stack/locals/jumps/calls; **not yet** all ownership obligations |
| VM | Verified bytecode | Value or controlled failure | Runtime subset only; future Tempra Script authority after parity |

An *analysed* or *verified* object applies only to the properties established by that stage. There is no universal, automatically acquired “safe for all future extensions” label.

## 3. Historical default path — HIR execution

```text
source
  ├─ import/module resolution, explicit stdlib root where applicable
  └─ lexer → parser → AST → HIR
                              ↓
                      type checking
                       ↳ K1 comptime facade
                              ↓
                       HIR interpreter
```

On the inspected `main` snapshot, this is the default `tempra run` path. It supports a broader HIR surface than the experimental VM but **rejects annotated execution** (source `@deny` or `@pure`) through a shared presence guard instead of silently ignoring contracts.

Historical other HIR-based paths include tests and REPL. They have documented unsupported/capability-contract boundaries and must not be assumed to enforce the same facts as a fresh VM compilation. Rejection belongs before REPL/session mutation where applicable.

This path is maintained only as long as needed for parity, reference tests and the **existing comptime kernel**; it is not a second general runtime in the approved Tempra Script target.

## 4. Current experimental VM fresh-compilation path

```text
source / multi-module input
  → parse → HIR → typecheck
       ↳ comptime evaluation and pruning
       ↳ concrete specialization requests
  → TIR lowering
  → TIR verifier
  → capability scan + call graph + fixed-point facts
  → source contract binding to concrete function IDs
  → broad and specialized contract enforcement
  → bytecode lowering + optimizations
  → bytecode verifier
  → execution in bytecode VM
```

**Critical ordering:** TIR is verified before E1 capability analysis; denying contracts are enforced **before** bytecode is eligible for execution or cache writing. The driver owns normalized contracts and concrete identity binding; frontend crates need not depend on `tempra-effects`.

The `VmCompilation` product can retain a verified TIR module, analysis facts, contracts and executable bytecode on fresh compilation. Capability facts are **not** implicitly generated on cache hits. A fresh compilation containing a called extern whose effect cannot be classified is rejected on an enforcing path; an unused declaration need not invalidate the module.

The VM's current source coverage is bounded. Source constructs lacking HIR → TIR or TIR → bytecode support must produce an explicit unsupported diagnostic: **there is no automatic retry in the HIR interpreter** after a VM failure, type error or contract violation.

### 4.1 Function and module identity

- `TirFunctionId` is a compilation-local semantic identity and **not** a raw index into the TIR function vector or bytecode table.
- Deterministic first-request specialization ordering and ordered module processing are K2 requirements.
- Bytecode emission maps concrete TIR IDs to bytecode function indices explicitly; physical storage position must not change a call target.
- Module/file compilation resolves the standard-library root as an explicit configuration input. There is no fallback to searching the current working directory for the stdlib.
- An unlowered generic template cannot be silently treated as an executable concrete specialization.

## 5. Cache-hit path and trust boundary

```text
source identity + compiler fingerprint + cache configuration
  → lookup .tempra/cache/ (documented single-source VM path)
     ├─ hit with valid header AND compatible semantic revision
     │    → decode → bytecode verify → VM execution
     └─ missing / stale / corrupt / invalid bytecode
          → fresh compilation (section 4)
              ├─ success → verify → safe cache replacement → execute
              └─ failure → diagnostic; existing artifact not promoted
```

A valid hit skips AST/HIR/TIR reconstruction and **does not run E1/E3/K4 again**. That is permitted only because the matching semantic compiler fingerprint attests to earlier successful enforcement for the supported cacheable source path.

In the inspected baseline, the cache uses magic `TMBC`, bytecode format `2` and semantic trust `semantics-8`; these are **historical version values**, not the declared versions of future Memory v0.4 enforcement. No new revision number is assigned by this document.

The non-cryptographic source hash/header is not a security authentication mechanism; the cache is considered local/compiler-owned. `decode_artifact` alone is not execution permission; bytecode must also be verified. Multi-module compilation historically has no equivalent persistent cache and must not be described as sharing the source-cache contract.

**TARGET:** ownership-aware verification and changed effects/copy behaviour require an explicit semantic version/trust migration. A historical compatible bytecode header cannot prove newly introduced loan/provenance/cleanup semantics by assumption.

## 6. Effect and specialized-effect boundary

```text
verified TIR
  → direct origins + called extern classifications
  → call graph, fixed-point propagation, witnesses
  → normalized effective contracts
       @pure = deny(io, process, spawn, nondeterminism)
       explicit @deny(...) is additive
       specialized fs/net/category/operation restrictions are additive
  → broad violation check
  → specialized violation check
  → no violation: eligible for bytecode lowering
```

- The six public broad denials are `alloc`, `panic`, `io`, `process`, `spawn`, `nondeterminism`; `Unsafe` is internal/reserved.
- Specialized restrictions do not become new capability bits and do not relax a broad denial.
- `@deny(alloc)` is conservative with respect to memory v0.4 value copies, buffer operations and backend differences.
- Semantic pruning in `comptime` is legitimate: unselected branches are erased before TIR analysis. **Merely expected** dead-code elimination, inlining, escape analysis or backend-specific optimization is not an effects proof.
- `check` and `emit-tir` on the documented baseline are **not** enforcement certificates. `tempra effects` uses a fresh enforcing compilation on supported surfaces.
- The native registry and classifier are trusted compiler metadata; a static `fs` denial is **not** runtime filesystem isolation, path canonicalization or a process sandbox.

See [E1 Capability Analysis](../E1_CAPABILITY_ANALYSIS.md), [Specialized Effects v0.2](../specs/specialized-effects-v0.2.md) and [K4 Gate](../K4_SPECIALIZED_EFFECTS_GATE.md).

## 7. Comptime: current path and future VM evaluator

### Current authoritative K1 path

```text
type checking / specialization
  → canonical tempra-interpreter comptime facade
       → module-aware HIR lookup (supported file/module paths)
       → shared nested evaluation session + deterministic fuel
       → restricted effects, controlled errors and call-depth
  → materialized known value or chosen branch in HIR
  → runtime TIR lowering (selected path only)
```

Currently the typechecker requests compile-time evaluation *before* it can always emit a complete runtime TIR module. `if comptime` and `comptime match` prune non-selected branches during semantic processing; rejected extern/host I/O is not an execution option simply because the runtime has a native dispatcher.

### TARGET — comptime on the same VM semantics

```text
typechecking request with type/generic environment
  → prepare eligible compile-time execution unit (design pending)
  → verify accepted comptime-only operations/effects/ownership
  → Tempra Script VM evaluator with bounded deterministic session
  → typed value / type / compiler diagnostic
  → HIR pruning and specialization continuation
```

This is a target, **not** a claim that `tempra-vm` already hosts K1 comptime. Equivalence must cover function/type specialization, `compile_error`, error ordering, restrictions on runtime values, host effects, nested fuel, call-depth and deterministic diagnostics. Instruction count is not automatically equal to K1's evaluator-progress ticks. `comptime` does not bypass `ref`/`inout` or resource nonduplicability.

The current HIR comptime implementation must not be removed until an independently tested VM replacement is ready. See [Comptime Specification](../COMPTIME_SPEC.md).

## 8. Ownership-aware pipeline — mandatory TARGET

Memory v0.4 is the **single normative model**; the exact pass placement and IR encoding remain implementation design choices. A valid implementation must make the following facts available to the correct checker/verifier boundaries:

```text
typed HIR
  → value-use and copy/consume analysis
  → owner/provenance and borrow-flow checking
  → verified ownership-state transitions [representation OPEN]
  → ownership-preserving TIR/bytecode lowering
  → runtime cleanup, including error/cancellation paths
```

| Operation | Required check / observable contract |
| --- | --- |
| Value argument/assignment | Duplicable values remain independent; last-use physical move cannot change source semantics |
| Resource transfer | Nonduplicable values require explicit semantic `consume`; no implicit unique duplication |
| `ref` | Read-only loan with a valid owner and no escaping provenance |
| `inout` | Exclusive mutable loan; no alias conflicts or hidden parallel accesses |
| `Slice(T)` | Non-owning read-only; owner stays valid for every use |
| `inout Slice(T)` | Exclusive mutation of elements; no synthetic owning copy as a safety bypass |
| `Range` | Half-open `[start,end)` applied to a concrete collection with bounds checks |
| Closure | Explicit capture modes, nonescaping borrows and safe `fn[self]` recursion |
| Early return/`Result` | Correct cleanup, source invalidation and no retained invalid borrow |
| `.await()` / actors | No dynamic borrow across suspension; isolated ownership; actor handles deferred |

A structural TIR verifier or bytecode stack checker **does not alone implement these contracts**. They may need additional semantic facts, verification protocols or representation changes. No concrete new TIR/VM opcode name is mandated here.

## 9. TIR interpretation and IR inspection

| Entry | Current purpose | Restrictions |
| --- | --- | --- |
| `emit-tir` | Typed source to verified, readable TIR | Does not run runtime/effect enforcement by itself |
| `interpret_tir_source` | Compile supported source to TIR and interpret a narrow subset | Rejects annotated source on documented baseline; not full VM parity |
| Raw `tempra-tir-interp` | `TirModule` → verifier → micro-step interpreter | No full stdlib, closure/heap coverage, general scripting runtime or ownership certification |

These paths are useful for localizing a discrepancy: syntax/typecheck vs TIR lowering vs bytecode lowering vs runtime. They are *not* alternative general user runtimes in the target architecture.

## 10. Target runtime and backend topology

```text
verified source semantics / TIR + semantic side tables
  ├─ Tempra Script stack bytecode → verified bytecode → VM  [scripting runtime target]
  ├─ Tempra Native LLVM AOT → native executable           [future]
  └─ dedicated Wasm/WASI emission                   [separate future track]
```

Tempra Script's intended memory management is ARC **NO-CYCLE**, with safe-language acyclicity and checked borrows. Tempra Native aims for LLVM-generated native values without GC/ARC where provable; **zero reference counting is a goal, not an established guarantee**. Dedicated Wasm code generation is distinct from Wasm hosting and from Tempra Native.

**No JIT/Cranelift execution or AOT path is part of the target.** The inspected `main` still contains legacy crates, features and tests. Code removal must be verified in a separate implementation change. Historical documents reside in `docs/archive/`.

## 11. Error, diagnostic and unsupported-feature policy

- **Parsing/typechecking:** reject invalid source before invoking runtime.
- **TIR lowering:** reject unsupported source constructs, missing required return, invalid local/call type facts and impossible CFG rather than inventing values.
- **TIR verification:** reject invalid blocks, terminators, types and instructions.
- **Effect enforcement:** reject violations before caching/execution; include retained witness/provenance where available.
- **Bytecode lowering/verifier:** reject impossible opcode typing, invalid jumps, stack depth, call arity or invalid local reads.
- **Runtime:** report checked arithmetic/bounds errors and explicit `Result` paths, not host-language panics.
- **Comptime:** deterministic bounded evaluation and controlled diagnostics, with restricted host effects.
- **Ownership (TARGET):** reject illegal borrow, nonduplicable copy, escaping view, unsound closure/actor transfer and double cleanup.

When source spans are unavailable from TIR, a future lowering-owned `TirLocation → {module, Span}` side table is the appropriate bridge. Debug names alone are not a reliable source map. Deterministic error ordering remains important to the compiler trust contract.

## 12. Promotion, removal and release verification

The HIR-default → Tempra Script-VM-default promotion must be gated by reproducible, commit-scoped evidence:

1. **Surface:** test the declared language subset including Result, patterns, generics, closure capture, modules, stdlib and CLI entry contracts.
2. **Failures:** differential controlled errors, including overflow, division by zero, invalid bounds and diagnostics.
3. **Effects:** broad and specialized policies are enforced on fresh and cache-backed runs, with explicit rejection of unsupported cases.
4. **Memory:** enforce the complete approved v0.4 ownership matrix; do not cite the existing Rust `Rc` representation as proof.
5. **Cache:** old trust revisions miss safely; cold, valid-hit and corrupted-hit behavior converges under current semantics.
6. **Comptime:** independently certify a future VM evaluator before retiring the HIR comptime kernel.
7. **Determinism:** preserve K2's supported Level 3 reproducibility or document a deliberately versioned boundary change.
8. **Removal:** verify absent legacy JIT/Cranelift crate dependencies, feature flags, command-line switches, CI jobs and benchmark claims in the separate code cleanup.
9. **Release:** satisfy [Tempra Script gates](../roadmap/release-gates.md) and record actual commit, commands, hardware and inputs for K5/K6 measurements.

**Pipeline invariant:** every executable artifact must be derived from checked semantics under a trusted compiler revision; unsupported or unsafe programs fail closed.
