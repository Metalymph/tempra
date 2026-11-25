# Tempra — Compiler & Runtime Architecture

**Documentation revision:** v0.2 — 2026-11-16
**Scope:** compiler layers, runtime paths, semantic authorities, ownership/effects, and planned backends
**Status:** consolidated target architecture with a separately identified implementation baseline. This document is not an implementation or release certificate.

## 1. Purpose and authority

Tempra is a statically checked, scripting-first systems language with value semantics. **Tempra Script** is its intended stack-VM scripting runtime; **Tempra Native** is the planned LLVM-based native AOT compiler. Both must implement the same source-language semantics.

Architectural descriptions cannot override normative language rules:

| Concern | Authority |
| --- | --- |
| Value semantics, ownership, borrowing, `consume`, slices, closure captures, cleanup, actor safety | [Memory & Ownership v0.4](../Tempra_Memory_Ownership_v0.4.md) — **sole memory-model authority** |
| Source language | [Surface Syntax v0.2](../TEMPRA_SURFACE_SPEC_v0.2.md) and [Grammar](../GRAMMAR.md), with explicitly open constructs excluded from current claims |
| Comptime | [Comptime Specification](../COMPTIME_SPEC.md), including the HIR-to-VM migration gate |
| Broad and specialized effects | [Capability Contracts](../specs/effects-v0.1.md), [Specialized Effects v0.2](../specs/specialized-effects-v0.2.md) |
| TIR and execution | [TIR Specification](../TIR_SPEC.md), [Bytecode VM](../BYTECODE_VM.md), [Bytecode ISA](../BYTECODE_ISA.md) |
| Cache and reproducibility | [Bytecode Cache](../BYTECODE_CACHE.md) |
| Delivery milestones, proof gates and releases | [Roadmap](../roadmap.md) and [roadmap subdocuments](../roadmap/) |

**Status distinction:** *approved* means designed; *implemented* means present on the stated bounded source path; *verified* requires reproducible test evidence at a specific commit; *formally proved* requires a separate proof artifact. None implies another.

### Baseline being described

The current runtime still defaults to HIR execution, with an experimental bytecode VM. The legacy JIT/Cranelift code has been removed by the retirement change; VM-first execution remains separately gated.

## 2. Design principles

1. **One source semantics:** neither the interpreter representation nor a native code-generation choice defines what a legal Tempra program means.
2. **Script-first:** fast startup, small runtime and predictable errors matter before sophisticated optimizations or native compilation.
3. **Typed intermediate facts:** validated TIR, function identities, call graphs, effects and future ownership facts are shared by consumers, not independently re-inferred in each backend.
4. **Verifier boundaries:** malformed IR, stale artifacts and unsupported source constructs fail with controlled diagnostics; they do not trigger silent semantic fallback.
5. **Deterministic compilation:** inputs, module order, specialization ordering and trusted cache revisions must not introduce incidental nondeterminism on the supported surface.
6. **Explicit effects and ownership:** minimal source syntax, strong semantic enforcement; `@pure` and `@deny(...)` do not silently relax memory safety.
7. **Distinct compiler and runtime goals:** Tempra Script may use acyclic reference counting; Tempra Native's RC-free LLVM goal is not already proven by that choice.
8. **Historical separation:** the JIT/Cranelift prototypes are retained as archival records only, not future runtime/compiler dependencies.

## 3. Current execution paths on the inspected baseline

### 3.1 Default HIR execution — current historical state

```text
source / modules
  → lexer → parser → AST → HIR
  → module resolution and type checking
       ↳ K1 comptime facade when required
  → HIR interpreter
```

The existing HIR execution path has broader language coverage than the experimental VM. It **rejects programs with capability contracts** (`@deny` and `@pure`) rather than running them without enforcement. Historically, `tempra check` performs frontend and type-checking work only; it does **not** certify broad/specialized capability enforcement or the future ownership proof. REPL and source TIR interpretation have separate unsupported-path guards. These are implementation limitations, not desired Tempra Script behavior.

### 3.2 Experimental bytecode VM — currently documented path

```text
source / module inputs
  → parse, HIR, typecheck, comptime pruning
  → specialized concrete functions → TIR lowering
  → TIR structural/type verification
  → E1/E2 capability and specialized-effect facts
  → E3/K4 contract enforcement
  → TIR → bytecode lowering
  → bytecode verification
  → Tempra Script stack VM (experimental)
```

A fresh VM compilation can retain verified TIR, capability facts and bytecode in the internal `VmCompilation` product. Capability analysis is compiler-owned: untrusted annotations cannot be silently ignored by a runtime dispatch decision. The current source path may fail for syntax/features not lowered to executable TIR/bytecode. A failure is reported rather than silently rerouting to HIR execution.

**Cache hit:** matching source identity and compiler/semantic fingerprint, followed by bytecode verification, may execute cached bytecode *without reconstructing TIR or capability facts*. That is acceptable only within the historical compiler-trust contract for the cache revision; introducing the new v0.4 ownership enforcement needs a new, explicitly validated trust boundary. See [Bytecode Cache](../BYTECODE_CACHE.md).

### 3.3 Diagnostic and inspection paths

| Path | Role | What it does **not** certify |
| --- | --- | --- |
| `tempra check` (historical baseline) | Frontend/module/type checks | Complete effects/ownership enforcement |
| `tempra emit-tir` | TIR lowering, verification, readable IR | Executability of the full source language or effect-contract satisfaction |
| `tempra effects` | Fresh capability-aware compilation and inspection on its supported subset | Unavailable HIR-only operations, future ownership proof or sandboxing |
| Raw/source TIR interpreter | Narrow TIR validation/regression | Full VM, stdlib, native ABI, actor or ownership behavior |
| REPL/LSP | Interaction and diagnostics over their documented frontend scope | Automatic certification through the VM pipeline |

The precise CLI features supported in the user's branch must be checked before presenting new release guidance.

## 4. Approved target: Tempra Script VM-first runtime

```text
source
  → lexer / parser
  → typed HIR + module resolution + generic specialization
  → comptime resolution / branch pruning
  → ownership/value-use analysis [planned, v0.4]
  → verified TIR + retained semantic facts
  → effects and capability contract enforcement
  → ownership-valid bytecode lowering and verification [planned gate]
  → Tempra Script stack VM
       → controlled runtime errors / stdlib native boundary
```

The ownership stages above are **requirements, not claims of deployed passes**. The exact TIR representation of ownership/loans is deliberately unspecified by the memory specification: it may involve explicit IR instructions, metadata, proven type states or combinations. The key requirement is that no backend can reinterpret an illegal source program as legal.

**Promotion rule:** replace HIR as the default runtime only after supported-language parity, error/Result parity, capability enforcement, memory and ownership gates, stdlib behavior, cache migration, diagnostics and release tests have passed. There must be no silent HIR fallback that bypasses safety enforcement. Maintaining a narrow TIR micro-interpreter for tests does not count as shipping a second general scripting runtime.

### Tempra Script memory contract

- Values have **value semantics**. An implementation may optimize a duplicable copy into a physical move only without altering observable independence.
- Nonduplicable resources (e.g., owned OS handles when introduced) cannot be implicitly copied; a semantic transfer requires an explicit `consume` contract.
- `ref` is read-only shared access; `inout` is exclusive mutation. Borrow provenance and early release/last use must be checked, including branches, loops and calls.
- `Slice(T)` is a **non-owning, read-only** view; writable access requires `inout Slice(T)`. A slice cannot keep its owner alive.
- Acyclic owning graphs are the safe-language requirement. Tempra Script's intended owning representation is **ARC NO-CYCLE**, not tracing GC, general object references or unconstrained strong ownership cycles.
- Cleanup must account for normal return, `Result` error paths, early exit, task cancellation and resource shutdown without double-destruction.
- `fn[ref x]`, `fn[inout x]`, `fn[consume x]` and recursive `fn[self]` preserve explicit capture semantics; recursive closure implementation may not introduce a strong cycle.
- Borrowed references/slices may not escape owners or cross `.await()` where a dynamic loan would remain live. Safe actor handles and lifetime semantics remain **deferred after the NO-CYCLE gate**.

The existing Rust `Rc`/interior-mutability representations are **implementation details**, not evidence that v0.4 is already enforced. This distinction is fundamental to the migration.

## 5. Compiler layers and ownership of responsibilities

| Layer | Main crates (inspected `main`) | Responsibilities |
| --- | --- | --- |
| Frontend | `tempra-lexer`, `tempra-parser`, `tempra-ast` | Tokens, syntax, source spans and AST |
| Semantic lowering | `tempra-hir`, `tempra-typeck` | Typed HIR, generic/interface resolution, value-use/type checking and future borrow contracts |
| Current comptime | `tempra-interpreter` via `comptime.rs` | K1 canonical evaluated expression/session policy, currently HIR based |
| TIR | `tempra-tir`, `tempra-tir-lower` | Typed, verifiable control flow and lowering from supported HIR |
| Effects | `tempra-effects` | Pure verified-TIR analysis; per-function summaries, call graph, witnesses |
| Orchestration | `tempra-driver` | Compilation inputs, verification/analyze/enforce ordering, cache boundary, module graph |
| Bytecode | `tempra-bytecode-lower`, `tempra-bytecode` | Instruction selection, encoding, verifier, cache and optimization |
| Execution | `tempra-vm`, `tempra-runtime`, `tempra-native` | Stack VM, checked common arithmetic and native stdlib boundary |
| Validation only | `tempra-tir-interp` | Small TIR executable subset for differential/IR tests |
| Tools | `tempra-cli`, `tempra-analysis`, `tempra-diagnostics`, `tempra-lsp` | User and editor interfaces to compiler facts |

A crate's presence is not proof of completeness, runtime eligibility or a stable API. See [Repository File Map](FILE_MAP.md) for verified source paths and planned scaffolds.

## 6. Effect and capability analysis architecture

E1–E6 and the K4 specialized-effects track are documented as implemented on a **bounded TIR-supported surface**, not as universal coverage for every HIR construct.

```text
verified TIR
  → direct instruction / classified native extern requirements
  → call graph
  → transitive fixed-point capability summaries + witnesses
  → bind source contracts to concrete specialized function IDs
  → broad denial checks, then specialized refinements
  → either diagnostics or permission to lower verified code
```

The seven internal capabilities are `Alloc`, `Panic`, `Io`, `Process`, `Spawn`, `Nondeterminism` and reserved `Unsafe`. The six public broad `@deny` spellings exclude unsafe. `@pure` means deny `io`, `process`, `spawn`, `nondeterminism` **only**; it does not prohibit allocation or panics. K4 specialized families/categories and operation-resource denials refine but never relax broad restrictions.

Facts are based on *semantic runtime operations*, not on the spelling of a `const` or `make` instruction alone. Checked integer arithmetic can conservatively require `Panic`. Memory v0.4 makes `@deny(alloc)` a conservative backend-independent promise; possible runtime copy/compaction cannot be declared allocation-free solely because an optimizer might remove it.

- Source span provenance belongs to frontend/lowering side tables; structural TIR locations alone do not identify reliable source spans.
- Production native registry and effect classification must remain consistent. The current native boundary is **not** a filesystem sandbox or a runtime capability guard.
- Unknown called extern classifications fail closed on paths requiring sound analysis; unused extern declarations do not necessarily invalidate a module.
- A cache hit uses a trusted compiler fingerprint rather than reconstructing all semantic facts; new semantic obligations require explicit invalidation.
- `@unsafe` and `@verified` are deferred; neither can be invented as an implemented escape hatch for source-level memory rules.

## 7. Comptime authority and migration

K1's current canonical `comptime` kernel is the HIR interpreter behind a compiler-facing facade. K1 provides module-aware lookup on supported typechecking paths, deterministic evaluator-progress fuel, an eight-active-frame default call-depth guard and rejection of host-observable I/O/process/external operations. The compiler materializes supported values and prunes unselected `if comptime`/`comptime match` branches **before runtime TIR emission**.

**Target:** run compile-time evaluation on Tempra Script's VM without weakening these semantics. The VM cannot assume a complete runtime TIR module already exists: compile-time expressions are needed during type checking, generic specialization and branch selection. The migration therefore needs a separate eligible evaluation representation/session, explicit effect restrictions, controlled diagnostics, a fuel-accounting equivalence or approved revision, and memory/ownership parity. Executing the same apparent source program at runtime is not by itself sufficient proof.

This is a **planned migration**. The current HIR comptime kernel must not be removed until the replacement passes its explicit gate. The revised [Comptime Specification](../COMPTIME_SPEC.md) governs the detailed contract.

## 8. Reproducibility, cache and standard-library discovery

- K2's accepted bounded reproducibility contract establishes **Level 3** on the supported same-build, same-host/target bytecode surface; not cross-platform byte identity.
- `TirFunctionId` identifies a compilation-local semantic function, **not** the index of a vector element. Bytecode lowering emits deterministic function order and resolves calls through an explicit identity-to-index map.
- Standard-library location is supplied through `CompilationConfig`: explicit root or default discovered relative to the **compiler executable**, not by searching process cwd. The resolver receives the resolved root and maintains module identities.
- The documented remote baseline uses bytecode cache format `2`, magic `TMBC` and semantic trust revision `semantics-8`. Those numbers are **historical checked baseline**, not a reserved version for the future v0.4 ownership implementation.
- A cache artifact stores verified bytecode without TIR/effect facts. A new safety analysis must change its trust boundary rather than reinterpret old artifacts as ownership-certified.

## 9. Runtime failure, FFI and module boundaries

Arithmetic overflow/division, bounds, unsupported instructions, source Result propagation and verifier errors must follow controlled Tempra contracts. Typechecking and capability failures occur **before** execution on enforcing paths. No host panic, native trap or fallback execution may substitute for an expected typed diagnostic.

Extern calls cross a trusted Rust/native boundary. FFI safety policy from Memory v0.4 applies to future external pointer/resource APIs; raw unsafe authorization and detailed cross-language ABI are deferred. Temporary buffers used for external/async I/O must respect owner lifetime, stable address needs, cancellation and exclusive mutation.

The module system retains qualified module identity; compiled specializations and diagnostics must not depend on hash iteration. Imports, source paths and explicit stdlib roots affect reproducibility as described in the module and cache specifications.

## 10. Tempra Native LLVM AOT and other targets

**Tempra Native target:** native AOT code generation through LLVM after Tempra Script's scripting maturity and required semantic gates. The AOT backend must consume the same type, ownership, borrowing, effect and cleanup contracts. **Zero GC/ARC is an architectural optimization/research objective, not a proven property of the backend**. A safe and correct implementation may need fallback management or a narrower proven subset until its absence of runtime reference counting is demonstrated.

A dedicated Wasm/WASI code-generation path is separate and remains incomplete in the inspected baseline; Wasm hosting and Wasm backend emission are not interchangeable. Neither target is an Tempra Script scripting release prerequisite. There is **no planned Cranelift-based AOT backend** and **no planned JIT**.

The legacy JIT and native backend scaffold have been removed from the workspace, VM, driver, CLI and CI. Historical design notes remain archived; the retirement change requires its own build and test validation.

## 11. Semantic milestones and release gates

The roadmap's phase identities must not be silently renumbered:

| Track | Meaning | Boundary |
| --- | --- | --- |
| E / K4 | Capability/specialized-effect enforcement | Implemented for documented bounded verified-TIR surface; known tooling and native gaps |
| K1 / K2 | Comptime HIR authority and reproducible specialization/compilation | Historical implementation gates completed on their stated surfaces |
| K3 | Runtime parity | In progress in inspected documentation; no whole-language certificate |
| K5 / K6 | Phase timings, latency baselines and regression budgets | Measurement and budgets; exact active-branch status to be refreshed |
| R | Result/value-use hardening | Partial implementation reported; separate from a formal R proof |
| M / M5 | Acyclic ownership and memory stability | **Mandatory gate** before safe actor concurrency C |
| C | Structured concurrency and actors | Deferred until M5; actor handles later after NO-CYCLE |
| A | Tempra Native native AOT | Future LLVM backend; no Cranelift commitment |
| O / P | Ownership-informed optimizations / deeper performance work | Separate from memory correctness |
| U / V | Conditional unsafe boundary / future verification constructs | Not implemented as general user escapes |

The Tempra Script release plan additionally requires representative default-VM scripts, stdlib usability, CLI/distribution smoke tests, controlled diagnostics, performance evidence and developer tooling. See [Tempra Script release gates](../roadmap/release-gates.md).

## 12. Required evidence before declaring this architecture implemented

1. Compare paths and crate APIs against the **actual feature branch** and record the commit SHA.
2. Run a supported-surface HIR/VM differential matrix including errors, `Result`, patterns, closures, module imports, CLI entrypoints and stdlib boundaries.
3. Establish ownership compiler errors and runtime cleanup for the complete v0.4 conformance matrix; do not infer them from `Rc` internals or stack verification.
4. Validate broad/specialized effects with fresh build, cache hit/miss, profile mode and unsupported-path denial.
5. Prove that older cache revisions cannot skip changed semantics; retain K2's determinism tests.
6. Gate `comptime` migration independently, including fuel, effect policy, specialization and values with ownership obligations.
7. Keep the retired JIT and Cranelift paths absent; verify the dependency/CI/CLI gate.
8. Update [Pipeline](PIPELINE.md), [File Map](FILE_MAP.md), [VM Coverage](../VM_COVERAGE.md) and roadmap statuses only with collected test evidence.

**Architectural invariant:** one language memory model, one supported scripting runtime, verified semantic boundaries and independently validated backends.
