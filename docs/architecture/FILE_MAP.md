# Tempra — Repository & Documentation File Map

**Documentation revision:** v0.2 — 2026-11-16
**Purpose:** directory navigation and responsibility ownership; **not** an implementation or feature-completeness certificate.
**Source baseline:** repository `Metalymph/tempra`, GitHub `main` tree inspected on 2026-11-16. Paths may differ on the local `feature/k5-compiler-timings` branch.

## 1. How to use this index

This map answers *where should I inspect or change a specific part of Tempra?* It does not redefine syntax, effects, borrowing or memory. Use the following authority order:

1. [Memory & Ownership v0.4](../Tempra_Memory_Ownership_v0.4.md) is the **only normative memory model**.
2. [Surface Syntax v0.2](../TEMPRA_SURFACE_SPEC_v0.2.md) and [Grammar](../GRAMMAR.md) define the documented source surface; explicitly open actor/receiver forms are not assumed implemented.
3. [Comptime](../COMPTIME_SPEC.md), [Capability Contracts](../specs/effects-v0.1.md) and [Specialized Effects v0.2](../specs/specialized-effects-v0.2.md) define their respective semantic domains.
4. [Architecture](ARCHITECTURE.md), [Pipeline](PIPELINE.md) and compiler/VM documents describe implementation boundaries, not a second language semantics.
5. [Roadmap](../roadmap.md) governs work ordering and reported milestone status, not the behavior of a legal program.

`docs/README.md` is the documentation index and authority map. This file is a **source navigation companion** for implementers.

## 2. Documentation tree

The following describes the approved consolidated layout. Files revised in this documentation series are intended to replace their former versions **under the same current paths** unless named otherwise. Archived paths and proposals are documented separately. The exact local tree should be checked before the PR.

```text
docs/
├── README.md
├── Tempra_Memory_Ownership_v0.4.md
├── TEMPRA_SURFACE_SPEC_v0.2.md
├── GRAMMAR.md
├── COMPTIME_SPEC.md
├── TIR_SPEC.md
├── TIR_LOWERING.md
├── TIR_TO_BYTECODE.md
├── TIR_INTERP.md
├── BYTECODE_ISA.md
├── BYTECODE_VM.md
├── BYTECODE_CACHE.md
├── VM_COVERAGE.md
├── VM_PERF_BASELINE.md
├── STDLIB.md
├── STDLIB_TUTORIAL.md
├── TYPES_TUTORIAL.md
├── TOOLING.md
├── E1_CAPABILITY_ANALYSIS.md
├── K4_SPECIALIZED_EFFECTS_GATE.md
├── DEV_GUIDE.md
├── roadmap.md
├── architecture/
│   ├── ARCHITECTURE.md
│   ├── PIPELINE.md
│   └── FILE_MAP.md
├── specs/
│   ├── effects-v0.1.md
│   ├── specialized-effects-v0.2.md
│   ├── module-system-v0.1.md
│   └── pattern_matching_v1.md
├── roadmap/
│   ├── Tempra Script.md
│   ├── status.md
│   ├── migration.md
│   ├── k5.md
│   ├── release-gates.md
│   └── formal-verification.md
├── editor/
│   ├── README.md
│   ├── neovim.md
│   ├── vscode.md
│   └── zed.md
├── cli/                            # local layout; confirm tracked files
├── proposals/
│   └── INDENTATION_SYNTAX_PROPOSAL_v0.1.md
└── archive/
    ├── jit/
    ├── syntax/
    └── effects/
```

`docs/proposals/` contains **non-normative exploration**. The indentation/Python-style proposal is not a change to the canonical parser syntax, and no runtime, AST or ownership behavior is implied by its presence.

`docs/archive/` contains preserved history. The reviewed archival candidates include the original `CRANELIFT_JIT.md`, `JIT_BURNIN.md`, `JIT_HEAP_BOUNDARY.md`, `JIT_PHASE_17_CLOSURE.md`, `specs/syntax-v0.1.md` and `specs/specialized-effects-v0.1.md`. Later superseded copies may also be retained there. Do not treat an archived document as current or restore obsolete links without redirecting them to the active specification.

## 3. Frontend: source to typed HIR

| Path (relative to repo root) | Responsibility | Status on inspected `main` |
| --- | --- | --- |
| `crates/tempra-lexer/src/` | Tokens, lexical spans | Present |
| `crates/tempra-parser/src/` | Source grammar and parsing | Present |
| `crates/tempra-ast/src/` | AST item/expression/type structures | Present |
| `crates/tempra-hir/src/hir.rs` | HIR nodes | Present |
| `crates/tempra-hir/src/lower.rs` | AST → HIR lowering | Present |
| `crates/tempra-typeck/src/check_file.rs` | File-level typing coordination | Present |
| `crates/tempra-typeck/src/check_item.rs` | Item declarations, interfaces, methods | Present |
| `crates/tempra-typeck/src/check_stmt.rs` | Statements and local binding checks | Present |
| `crates/tempra-typeck/src/check_expr.rs` | Expression type checking | Present |
| `crates/tempra-typeck/src/context.rs` | Typed semantic context | Present |
| `crates/tempra-typeck/src/types.rs` | Type definitions/helpers | Present |
| `crates/tempra-typeck/src/pattern.rs` | Pattern analysis | Present |
| `crates/tempra-typeck/src/result_obligations.rs` | Result/value-use obligations | Present |
| `crates/tempra-typeck/src/projection.rs` | Associated-type projection | Present |

**Memory work:** the v0.4 type/borrow checker belongs in semantic/compiler analysis, not in a separate user-visible ownership language and not as an implementation-specific VM hack. The listed current typecheck files **do not by themselves prove** nonduplicability, borrow provenance, no-cycle safety or complete ownership enforcement.

## 4. Driver: compilation orchestration and contracts

| Path | Responsibility |
| --- | --- |
| `crates/tempra-driver/src/pipeline.rs` | HIR execution / experimental VM path orchestration |
| `crates/tempra-driver/src/compilation.rs` | Fresh verify → analyze → enforce → bytecode boundary; compilation products |
| `crates/tempra-driver/src/config.rs` | Explicit compilation configuration and executable-relative `std/` root discovery |
| `crates/tempra-driver/src/module_graph.rs` | Module loading/import graph and resolution |
| `crates/tempra-driver/src/contracts.rs` | Source contract normalization and concrete function binding |
| `crates/tempra-driver/src/enforcement.rs` | Broad/specialized deny enforcement and typed violations |
| `crates/tempra-driver/src/specialized_matching.rs` | Specialized-effect matching |
| `crates/tempra-driver/src/effect_tooling.rs` | E6 diagnostics/inspection projection |
| `crates/tempra-driver/src/cache.rs` | VM cache integration |
| `crates/tempra-driver/src/build.rs` | Build scaffold; not proof of completed native compiler |
| `crates/tempra-driver/src/install.rs` | Install scaffold; not proof of production distribution |

The driver is the owner of compiler-wide enforcement and explicit compilation inputs. It must not route around contracts by selecting a backend lacking a required analyzer. The accepted `CompilationConfig` stdlib root is an input; no implicit cwd discovery belongs in module resolution.

## 5. TIR, analysis, bytecode and Tempra Script VM

| Path | Responsibility | Qualification |
| --- | --- | --- |
| `crates/tempra-tir/src/ir.rs` | Typed TIR IDs, functions, CFG blocks and instructions | Structural IR; v0.4 representation still to be designed/verified |
| `crates/tempra-tir/src/builder.rs` | TIR construction / stable function identities | Presence does not guarantee generic/ownership coverage |
| `crates/tempra-tir/src/verifier.rs` | Structural/typed TIR checking | Not a certified v0.4 borrow checker |
| `crates/tempra-tir/src/pretty.rs` | Readable TIR output | Inspection only |
| `crates/tempra-tir-lower/src/lower.rs` | Typed HIR → TIR | Subset coverage; unsupported constructs fail |
| `crates/tempra-tir-interp/src/` | Narrow TIR micro-interpreter | Tests/validation, not an Tempra Script general runtime |
| `crates/tempra-effects/src/capability.rs` | Capability vocabulary and sets | Compiler-internal types |
| `crates/tempra-effects/src/analysis.rs` | Per-function direct/transitive effects, graph and witnesses | Verified-TIR-surface scope |
| `crates/tempra-effects/src/operations.rs` | Specialized operation vocabulary | Compiler-owned taxonomy |
| `crates/tempra-effects/src/facts.rs` | Specialized analysis facts | No extra independent effects analyzer |
| `crates/tempra-effects/src/externs.rs` | Classification of native calls | Must match registry metadata |
| `crates/tempra-bytecode-lower/src/lower.rs` | TIR → stack bytecode | Typed opcode selection and relocation |
| `crates/tempra-bytecode-lower/src/peephole.rs` | Local bytecode optimization | Must preserve ownership/effects |
| `crates/tempra-bytecode/src/opcode.rs` | Bytecode opcode enum | ISA's implementation authority for exact variants |
| `crates/tempra-bytecode/src/verify.rs` | Stack bytecode verifier | Does not yet certify v0.4 ownership |
| `crates/tempra-bytecode/src/serialize.rs` | Serialized cache/artifact format | Cache format version separate from semantic revision |
| `crates/tempra-vm/src/interpreter.rs` | Stack VM dispatch | Experimental on inspected `main`; Tempra Script target |
| `crates/tempra-vm/src/value.rs` | VM value representation | Current Rust representation ≠ normative Tempra memory model |
| `crates/tempra-vm/src/profile.rs` | Counters and VM profiling | Hotness is telemetry; no future JIT promotion |

**Identity invariant:** `TirFunctionId` must not be conflated with a vector index or bytecode function position; the lowering is responsible for a deterministic explicit mapping.

## 6. Current comptime and shared runtime

| Path | Responsibility |
| --- | --- |
| `crates/tempra-interpreter/src/comptime.rs` | Historical canonical K1 HIR-evaluator facade, fuel, mode and boundary handling |
| `crates/tempra-interpreter/src/` | HIR interpreter, evaluator and values; historical default execution path |
| `crates/tempra-interpreter/src/arithmetic.rs` | Interpreter-side numeric compatibility helpers |
| `crates/tempra-runtime/src/arithmetic.rs` | Shared checked numeric semantics and errors |
| `crates/tempra-runtime/src/` | Runtime support shared by covered consumers |

**TARGET:** migrate comptime to the Tempra Script VM **only after** the dedicated [Comptime](../COMPTIME_SPEC.md) parity gate. This does not mean deleting `tempra-interpreter` now: its current comptime authority and HIR-reference testing must be retained until verified replacements exist.

## 7. Standard library and native extern boundary

| Path | Responsibility |
| --- | --- |
| `std/` | `.tmpr` standard-library declarations (`fs`, `io`, `path`, `strings`, …) |
| `crates/tempra-native/src/registry.rs` | Native callable registry |
| `crates/tempra-native/src/std/fs.rs` | Filesystem externs |
| `crates/tempra-native/src/std/io.rs` | Console/stream I/O externs |
| `crates/tempra-native/src/std/path.rs` | Path externs |
| `crates/tempra-native/src/std/strings.rs` | Strings externs |
| `crates/tempra-stdlib/` | Existing placeholder/legacy crate on inspected tree; verify before deletion |

An extern declared in `std/` is not evidence of a fully implemented native API. The registry, effect classifier, ownership/cancellation rules and runtime tests must agree. External pointer/FFI rules remain subject to Memory v0.4; `@unsafe` does not currently authorize arbitrary public raw access.

## 8. CLI, tooling and editor integrations

| Path | Responsibility |
| --- | --- |
| `crates/tempra-cli/src/app.rs` | CLI command/argument definitions |
| `crates/tempra-cli/src/commands/` | `run`, frontend checks, IR/effect inspection and other implemented commands |
| `crates/tempra-repl/` | REPL integration |
| `crates/tempra-analysis/src/lib.rs` | Analysis adapters for editors/tools |
| `crates/tempra-diagnostics/src/lib.rs` | Structured diagnostics, labels/spans, JSON projection |
| `crates/tempra-lsp/src/server.rs` | Language server |
| `docs/TOOLING.md` | Tooling contract/documentation (not an implementation file) |
| `docs/editor/` | Editor setup guides |

Frontend-only `check` is **not** the current capability/memory execution certificate. Tooling must not silently claim greater analysis coverage than the driver actually establishes.

## 9. Incomplete backends, scaffolds and retired components

| Path | Inspected `main` state | Target handling |
| --- | --- | --- |
| `crates/tempra-mir/` | Scaffold | Design optional lower IR only if justified; no implemented production path claimed |
| `crates/tempra-ir/` | Generic IR scaffold | Distinct from active TIR |
| `crates/tempra-opt/` | Optimization scaffold | Ownership/escape/reuse passes require proven facts |
| `crates/tempra-backend-wasm/` | Wasm codegen scaffold | Dedicated Wasm/WASI future track |
| `crates/tempra-wasm-host/` | Wasm host scaffold | Runtime hosting, **not** Wasm emission |
| `crates/tempra-component-host/` | Component host scaffold | Separate from compiler backend |
| `crates/tempra-linker/` | Linker scaffold | Future verified native output |

**Tempra Native LLVM AOT:** approved future direction, **not** represented as a verified/implemented LLVM backend crate on the inspected repository tree. Do not invent `crates/tempra-backend-llvm/` as an existing path. Tempra Native's zero-GC/ARC objective is not proven and must not be presented as a shipping guarantee.

JIT and Cranelift historical prototype notes belong in `docs/archive/jit/`. The retirement change also removes the compiler/runtime code, Cargo dependencies, feature flags, CLI switches and dedicated CI jobs.

## 10. Where to make upcoming changes

| Goal | Primary starting point | Required reading |
| --- | --- | --- |
| Reconcile surface grammar | `tempra-parser`, `tempra-ast`, `tempra-hir` | Surface v0.2 and Grammar |
| Implement `ref`, `inout`, `consume`, nonduplicable resources | `tempra-typeck`, `tempra-hir`, `tempra-tir`, VM runtime | Memory v0.4; avoid VM-only safety semantics |
| Add ownership-aware verifier | `tempra-tir/src/verifier.rs`, lowering and bytecode verifier | Memory v0.4 and TIR Spec |
| Fix specialization function ID mapping | `tempra-driver`, `tempra-tir`, `tempra-bytecode-lower` | K2/reproducibility and cache rules |
| Extend capability classification | `tempra-effects` and `tempra-native` | Effects and K4 ledger |
| Improve Tempra Script stack execution | `tempra-vm`, `tempra-bytecode-lower` | Bytecode VM/ISA; benchmark before optimization |
| Move `comptime` to VM | `tempra-typeck`, `tempra-interpreter/comptime.rs`, VM | Comptime Spec and K1 parity obligations |
| Record K5/K6 | `docs/roadmap/k5.md` and benchmark harness | Exact commit/environment/raw data, no invented speedups |
| Remove JIT/Cranelift implementation | Cargo workspace, JIT/Cranelift crates, CLI/tests/CI/Just recipes | Retirement change; validate workspace and keep history archived |
| Develop Tempra Native | New backend design, only after requirements/gates | LLVM AOT target, Memory v0.4; no assumed RC-free proof |

## 11. Repository maintenance and verification

Before merging the unified documentation PR:

1. Pin the **actual** base commit on `feature/k5-compiler-timings` or its replacement branch and inspect its files; this map was verified against `main`.
2. Confirm that revised docs are present under their intended paths; update `docs/README.md` links from old Surface Syntax v0.1 to v0.2 and refresh archive references.
3. Check all relative Markdown links (including links from nested `architecture/`, `roadmap/`, `archive/` and `proposals/` directories).
4. Validate referenced source paths with `git ls-files`; archive changes can affect historical relative links.
5. Run compiler and driver regression suites before changing status labels. No result from a historical test report certifies a later feature branch.
6. Document which memory-v0.4 obligations are still *specified only*, which are implemented, and which have evidence of parity or formal proof.
7. Keep roadmap snapshot/version semantics/cache labels truthful; record actual K5 measurements from the active branch before claiming performance.

**Index invariant:** a file path identifies code or documentation to inspect; it does not prove implementation completeness or authorize conflicting language behavior.
