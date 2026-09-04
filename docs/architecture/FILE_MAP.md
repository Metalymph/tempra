# Tempra Repository File Map

This index points to current source-of-truth files. A crate's presence in the workspace does not imply production completeness.

## Project documentation

- `README.md`: project overview and current high-level status.
- `docs/roadmap.md`: normative implementation order, including E → M → C → V → O.
- `docs/TEMPRA_SURFACE_SPEC_v0.1.md`: current surface-language reference.
- `docs/GRAMMAR.md`: parser-oriented grammar for implemented syntax.
- `docs/specs/effects-v0.1.md`: implemented deny acquisition, planned enforcement and Phase E architecture.
- `docs/E1_CAPABILITY_ANALYSIS.md`: implemented internal scope, API, and retained boundaries for E1.
- `docs/TIR_SPEC.md`: TIR model, verifier role, and analysis insertion point.
- `docs/architecture/ARCHITECTURE.md`: actual compiler paths and crate boundaries.
- `docs/architecture/PIPELINE.md`: current default and experimental data flow.
- `docs/DEV_GUIDE.md`: contributor workflow.
- `docs/TOOLING.md`: current CLI diagnostics and LSP behavior.

`docs/specs/syntax-v0.1.md` is an early historical sketch. It is not authoritative where it conflicts with the surface spec or grammar. Phase-numbered JIT, VM, and lowering documents are implementation records for their named prototypes, not the current language roadmap.

## Frontend and semantic analysis

- `crates/tempra-lexer/src/`: tokens and lexical analysis.
- `crates/tempra-parser/src/`: recursive-descent parsing.
- `crates/tempra-ast/src/`: source-shaped AST definitions.
- `crates/tempra-hir/src/hir.rs`: HIR definitions.
- `crates/tempra-hir/src/lower.rs`: AST-to-HIR lowering.
- `crates/tempra-typeck/src/check_file.rs`: file-level typechecking coordination.
- `crates/tempra-typeck/src/check_item.rs`, `check_stmt.rs`, `check_expr.rs`: item, statement, and expression checking.
- `crates/tempra-typeck/src/context.rs`: semantic context and collected facts.

## Driver and execution

- `crates/tempra-cli/src/app.rs`: CLI definitions and command dispatch.
- `crates/tempra-cli/src/commands/`: implemented `run`, `check`, diagnostics, symbols, test, REPL, and TIR inspection commands.
- `crates/tempra-driver/src/pipeline.rs`: default HIR interpreter and experimental VM orchestration.
- `crates/tempra-driver/src/compilation.rs`: private verify/analyze/enforce/bytecode boundary and public immutable `VmCompilation` product for fresh compilation.
- `crates/tempra-driver/src/contracts.rs`: canonical conversion (including the sole pure expansion), distinct deny/pure provenance and compilation-local contract binding.
- `crates/tempra-driver/src/enforcement.rs`: contract/summary intersection, structured violations with E1 witnesses, and typed invariant failures.
- `crates/tempra-driver/src/module_graph.rs`: filesystem module loading, ordering, and cycle detection.
- `crates/tempra-driver/src/build.rs`, `install.rs`: incomplete build and install scaffolds.
- `crates/tempra-interpreter/src/`: default HIR evaluator and runtime value representation.

## TIR and experimental VM

- `crates/tempra-tir/src/ir.rs`: TIR IDs, types, instructions, blocks, functions, and modules.
- `crates/tempra-tir/src/builder.rs`: deterministic TIR construction and function table.
- `crates/tempra-tir/src/verifier.rs`: structural and type verification.
- `crates/tempra-tir-lower/src/lower.rs`: typed-HIR-to-TIR lowering for the supported subset.
- `crates/tempra-tir-interp/src/`: focused TIR interpreter used for validation.
- `crates/tempra-bytecode/src/`: bytecode model, verifier, cache format, and profiling support.
- `crates/tempra-bytecode-lower/src/`: TIR-to-bytecode lowering.
- `crates/tempra-vm/src/`: experimental bytecode VM.
- `crates/tempra-jit-cranelift/src/`: opt-in Cranelift JIT for eligible VM functions.

## Standard library and native boundary

- `std/`: Tempra declarations for standard modules and extern functions.
- `crates/tempra-native/src/registry.rs`: native extern registry and lookup.
- `crates/tempra-native/src/std/`: implemented filesystem, path, strings, and console I/O externs.
- `crates/tempra-stdlib/`: legacy placeholder; new standard-library work belongs in `std/` and `tempra-native`.

## Diagnostics and tooling

- `crates/tempra-diagnostics/src/lib.rs`: structured diagnostics, labels, notes, spans, and JSON conversion.
- `crates/tempra-analysis/src/lib.rs`: editor-oriented semantic analysis over driver/frontend data.
- `crates/tempra-lsp/src/server.rs`: active LSP server implementation.

## Capability analysis

- `crates/tempra-effects/src/capability.rs`: internal capability identifiers and compact stable set representation.
- `crates/tempra-effects/src/analysis.rs`: pure TIR scan, call graph, fixed-point propagation, structured errors, and witness reconstruction.
- `crates/tempra-effects/src/externs.rs`: called-extern classification for the current native registry.

## Incomplete and planned middle-end/backend crates

- `crates/tempra-mir/`: MIR scaffold; lowering is not implemented.
- `crates/tempra-ir/`: small generic IR scaffold, separate from the active TIR path.
- `crates/tempra-opt/`: optimization scaffold; ARC and escape work are not implemented language guarantees.
- `crates/tempra-runtime/`: shared runtime scaffold.
- `crates/tempra-backend-cranelift/`: native AOT scaffold, distinct from the experimental JIT.
- `crates/tempra-backend-wasm/`: dedicated Wasm backend scaffold.
- `crates/tempra-wasm-host/`, `crates/tempra-component-host/`: Wasm hosting scaffolds, not compiler backends.
- `crates/tempra-linker/`: linker scaffold.

## Phase E ownership

E1 added `crates/tempra-effects/`, depending only on `tempra-tir`. It owns the internal capability vocabulary, sets, per-function summaries, direct discovery, call graph, fixed-point propagation, structured analysis errors, and deterministic provenance.

E2 integrates analysis only in fresh VM compilations, retaining the typed result
in `VmCompilation`. Cache hits, TIR emission/interpreter and HIR paths remain
independent. E3.1 adds syntax-only AST/HIR contracts and driver-owned declaration
metadata exposed by `VmCompilation::contract`/`contracts`. E3.2 adds driver
enforcement before bytecode, semantic cache fingerprinting, and explicit rejection
of annotated execution on HIR/REPL/source-TIR paths using the shared HIR presence
scan. E4 adds AST/HIR `pure_span`, driver-only expansion into effective denied sets,
and pure detection in that same scan; E3.2 enforcement and the cache fingerprint
are unchanged. Unsafe, rendered diagnostics, inspection and instruction-level
source mapping remain deferred. Frontend-only `check` does not certify contracts.
