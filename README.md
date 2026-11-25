# Tempra Documentation

**Document role:** documentation index and authority map
**Updated:** 2026-11-15
**Status:** active index; documentation reconciliation in progress
**Scope:** language specifications, implementation architecture, roadmap, tooling, and historical records

Tempra is a scripting-first, statically checked, value-oriented language. Its execution and compilation paths are developed incrementally. A design decision, an implementation, a passing regression test, and a formal proof are **different claims**; this documentation must not conflate them.

This page is the entry point for `docs/`. It does not redefine language semantics. For a given topic, use the authoritative document identified below; when a document describes the historical implementation, do not interpret it as the current language contract.

## 1. Normative sources and precedence

| Domain | Source | Authority and limits |
| --- | --- | --- |
| Memory safety, ownership, borrowing, value semantics, closure captures, async ownership, resource cleanup | [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) | **Canonical and overriding memory-model specification.** Approved design, not an implementation or proof certificate. |
| Surface syntax and source constructs | [Surface Syntax v0.1](TEMPRA_SURFACE_SPEC_v0.1.md) | Existing surface-language reference **pending full reconciliation**. Statements about memory semantics that conflict with Memory v0.4 are obsolete. |
| Parser-oriented syntax | [Grammar](GRAMMAR.md) | Parser-oriented description, subordinate to the agreed surface language and memory constraints; implementation status must be checked against current parser tests. |
| Comptime language rules, restrictions, and current execution policy | [Comptime Specification](COMPTIME_SPEC.md) | Governs compile-time evaluation; current HIR-based execution status and planned VM migration must remain distinct. Memory behavior must conform to Memory v0.4. |
| Broad capability contracts | [Capability Contracts v0.1](specs/effects-v0.1.md) | `@deny(...)`, `@pure`, effect analysis, and explicitly documented enforcement boundaries. |
| Refined effect families and operation semantics | [Specialized Effects v0.2](specs/specialized-effects-v0.2.md) | Current specialized-effect refinement. Historical v0.1 is archived. Memory v0.4 governs allocation-sensitive ownership/copy semantics. |
| Module system | [Module System v0.1](specs/module-system-v0.1.md) | Module organization and resolution, subject to current implementation checks. |
| Pattern matching | [Pattern Matching v1](specs/pattern_matching_v1.md) | Pattern semantics within its documented implemented subset. |
| Typed intermediate representation | [TIR Specification](TIR_SPEC.md) | Current TIR design/implemented subset; ownership-aware invariants in Memory v0.4 are the **target**, not an assertion that all TIR passes already enforce them. |
| Bytecode and VM representation | [Bytecode ISA](BYTECODE_ISA.md), [Bytecode VM](BYTECODE_VM.md), [Bytecode Cache](BYTECODE_CACHE.md) | Descriptions of implementation contracts, encoding, and cache boundaries. Verify status against the corresponding source and tests. |
| Delivery order and gates | [Roadmap](roadmap.md) | Authority for milestones, prerequisites, and reported progress **only**. A milestone description does not override a semantic specification. |

**Conflict rule:** a historical implementation description, test report, prototype document, or roadmap entry cannot authorize behavior forbidden by Memory v0.4. When sources conflict, record the discrepancy explicitly and reconcile the affected documents and implementation; do not silently assume the current compiler already complies.

## 2. Runtime architecture: actual state versus target

### Scripting runtime — Tempra Script

**Target pipeline:**

```text
Source → Lexer → Parser → typed HIR → TIR → verification
       → bytecode lowering → bytecode verification → stack VM
```

- **Current baseline on GitHub `main` at the time of this index:** the HIR interpreter remains the default `tempra run` path; the bytecode VM is an experimental path with a smaller supported surface. Current local development may be ahead of `main`.
- **Transition:** make the VM the authoritative scripting execution path only after the VM parity, correctness, capability, and implementation gates have passed. No silent fallback may bypass semantic or capability checks.
- **Memory target:** ARC **NO-CYCLE** for Tempra Script's owning runtime values, with Memory v0.4 constraints enforced at the compiler/runtime boundary. This is a planned implementation gate, not a completed guarantee.
- **Compile-time execution:** existing K1 documentation identifies the HIR interpreter as the canonical comptime kernel. VM-backed comptime is a planned transition and requires independent parity checks for determinism, fuel/termination, capabilities, cache trust, and ownership rules.

### Native compilation — Tempra Native

- **Planned backend:** LLVM-based native AOT, consuming the same ownership and effect contracts as Tempra Script.
- **Memory research objective:** eliminate GC/reference counting for managed native values when sound and practical. RC-free behavior is **not** an established implementation guarantee.
- **Do not treat a compiler scaffold or an implementation experiment as a released backend.**

### Retired design direction — JIT and Cranelift

The experimental Cranelift JIT and Cranelift native backend scaffold are retired. Historical documents are preserved under [`docs/archive/jit/`](docs/archive/jit/).

The removal eliminates the runtime dispatch tier, Cargo features, CLI switch and dedicated CI jobs. VM profiling remains available for measurement only. No JIT or Cranelift backend is part of the forward architecture.

## 3. Documentation map

### Language and semantics

- [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) — single source of truth for the memory model.
- [Surface Syntax v0.1](TEMPRA_SURFACE_SPEC_v0.1.md) — current source-language reference; scheduled for full syntax and semantic review.
- [Grammar](GRAMMAR.md) — parser-oriented grammar; scheduled for consistency review.
- [Comptime Specification](COMPTIME_SPEC.md) — compile-time evaluation, current HIR authority and future VM parity gate.
- [Capability Contracts](specs/effects-v0.1.md) and [Specialized Effects v0.2](specs/specialized-effects-v0.2.md) — effect and capability contracts.
- [Module System](specs/module-system-v0.1.md) and [Pattern Matching](specs/pattern_matching_v1.md) — domain-specific specifications.

### Compiler and runtime

- [Architecture](architecture/ARCHITECTURE.md), [Pipeline](architecture/PIPELINE.md), and [Repository File Map](architecture/FILE_MAP.md) — architectural boundaries and repository layout; **pending VM-parity review**.
- [TIR Specification](TIR_SPEC.md), [TIR Lowering](TIR_LOWERING.md), [TIR Interpreter](TIR_INTERP.md), and [TIR to Bytecode](TIR_TO_BYTECODE.md) — intermediate representation and transformations.
- [Bytecode ISA](BYTECODE_ISA.md), [Bytecode VM](BYTECODE_VM.md), and [Bytecode Cache](BYTECODE_CACHE.md) — scripting execution and artifact format.
- [VM Coverage](VM_COVERAGE.md) and [VM Performance Baseline](VM_PERF_BASELINE.md) — bounded historical or measured evidence; not a current feature certification without fresh tests.
- [K4 Specialized Effects Gate](K4_SPECIALIZED_EFFECTS_GATE.md) and [E1 Capability Analysis](E1_CAPABILITY_ANALYSIS.md) — implementation and validation history, retaining their dated scope.

### Roadmap, development and tooling

- [Roadmap](roadmap.md) and [roadmap subdocuments](roadmap/) — milestones, Tempra Script release gates, status snapshots, performance work, and proof requirements.
- [Developer Guide](DEV_GUIDE.md) and [Type Tutorial](TYPES_TUTORIAL.md) — contributors' implementation guidance; **verify HIR/VM paths before following**.
- [Standard Library](STDLIB.md) and [Stdlib Tutorial](STDLIB_TUTORIAL.md) — currently documented stdlib and contribution workflow; **verify actual runtime support**.
- [Tooling](TOOLING.md) and [Editor Integration](editor/README.md) — diagnostics, language tooling, and editor setup.

## 4. Historical archive

The [`archive/`](archive/) directory preserves superseded specifications and prototype write-ups **for traceability**, not as current engineering instructions.

| Archive location | Historical material | Why archived |
| --- | --- | --- |
| [`archive/jit/`](archive/jit/) | `CRANELIFT_JIT.md`, `JIT_BURNIN.md`, `JIT_HEAP_BOUNDARY.md`, `JIT_PHASE_17_CLOSURE.md` | Experimental Cranelift JIT design, tests, and heap boundary. |
| [`archive/syntax/syntax-v0.1.md`](archive/syntax/syntax-v0.1.md) | Early syntax sketch | Already superseded by the later surface and parser documentation. |
| [`archive/effects/specialized-effects-v0.1.md`](archive/effects/specialized-effects-v0.1.md) | Early specialized-effects baseline | Superseded by the normative specialized-effects v0.2. |

Preserve original content where feasible. Correct links to archived historical references; do not edit historical records to pretend that later decisions were known at the time. Git history remains the final record of changes.

## 5. Known reconciliation points

The items below are **documentation and implementation alignment requirements**, not a claim that they are complete.

1. **`Slice(T)` mutability:** older Surface Syntax text permits direct slice writes; Memory v0.4 defines a read-only view and requires exclusive `inout` access for mutation of underlying elements.
2. **Explicit closure captures:** preserve the existing `fn[ref name]`, `fn[inout name]`, `fn[consume name]`, and recursive `fn[self]` syntax. No default capture mode, no invented `captures` keyword, and no strong ownership cycle for recursive `self`.
3. **By-value versus `consume`:** ordinary duplicable parameters preserve value semantics; a physical move on proven last use is an optimization. Explicit `consume` is a semantic ownership transfer that invalidates the source. Nonduplicable resources require appropriate explicit ownership contracts.
4. **Method receivers:** reconcile `self`, `ref self`, `inout self`, and proposed consuming receiver contracts with the actual grammar and type checker. Do not claim unimplemented spellings already work.
5. **Async/actors:** no live dynamic borrow across suspension; task-owned values and actor isolation use the single memory model. Actor-handle identity and lifetime remain **DEFERRED POST NO-CYCLE**.
6. **Allocation effects:** verify the conservative `@deny(alloc)` contract against physical copying, COW, buffer compaction, closure environments, and runtime allocation. Do not let backend-specific optimizations define source-level validity.
7. **Comptime authority:** verify that migration from HIR evaluation to VM evaluation preserves type, ownership, deterministic execution, effects, termination/fuel budgets, and semantic cache invalidation.
8. **JIT and Cranelift removal:** historical documentation remains archived; the removal has its own workspace and CI validation gate. The planned native AOT direction is LLVM.
9. **Implementation status:** update coverage, feature flags, CLI examples, test commands, and performance claims only against the exact commit and test results after VM parity.
10. **Links and references:** check links after historical files move to `archive/`; no living normative file should link to archived content as if it were current.

## 6. Documentation maintenance rules

- **One topic, one canonical contract.** Supporting tutorials and architecture explanations link to the relevant specification instead of restating incompatible rules.
- **Label implementation status explicitly.** Use *approved design*, *implemented (bounded surface)*, *experimental*, *planned*, *deferred*, or *historical* accurately.
- **Prefer source-tested examples.** A syntactically illustrative example must be marked as such; do not present it as accepted by the present compiler without validation.
- **Keep future migrations distinct from completed work.** Especially for VM parity, ARC NO-CYCLE, LLVM AOT, and comptime-on-VM.
- **Preserve history.** Move obsolete documents to `archive/` and retain their provenance; update incoming relative links.
- **Validate documentation changes.** At minimum, check relative links, source paths, compiler flags, example syntax, and claimed tests against the chosen commit.

**Revision workflow:** reconcile Surface Syntax and Grammar against Memory v0.4; audit comptime; synchronize architecture/runtime/TIR documents after VM parity; then update stdlib, developer guides, roadmap, tooling, and performance/coverage reports. The documentation consolidation should land as a coherent PR, not as a collection of mutually inconsistent partial releases.
