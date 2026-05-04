# Tempra Repository File Map

This document provides a precise mapping of the Tempra repository structure. It serves as a guide for contributors to locate specific logic within the multi-crate workspace.

## 1. Root Files

- **`Cargo.toml`**: The workspace configuration file, defining all crates and shared dependencies.
- **`rust-toolchain.toml`**: Pins the Rust version and required components to ensure build reproducibility.
- **`README.md`**: Project overview, architectural rules, and high-level design summary.
- **`docs/roadmap.md`**: The strategic implementation timeline and priority tracking.

## 2. Documentation & Specifications

- **`docs/architecture/ARCHITECTURE.md`**: The central design document explaining the compiler philosophy and crate boundaries.
- **`docs/architecture/FILE_MAP.md`**: This document; a precise index of the repository.
- **`docs/architecture/PIPELINE.md`**: Documentation of the data flow from source to backend emission.
- **`specs/syntax-v0.1.md`**: Reference specification for the Tempra language grammar and keywords.
- **`specs/module-system-v0.1.md`**: Specification for package layout, imports, and the role of the entrypoint.
- **`specs/effects-v0.1.md`**: [FUTURE] Preliminary design for the algebraic effects system.

## 3. CLI (Command Line Interface)

- **`crates/tempra-cli/src/main.rs`**: The actual binary entrypoint; responsible for initializing the driver.
- **`crates/tempra-cli/src/app.rs`**: Manages the application state and top-level CLI configuration.
- **`crates/tempra-cli/src/commands/run.rs`**: Orchestrates the "run" command for direct source execution via the interpreter.
- **`crates/tempra-cli/src/commands/check.rs`**: Orchestrates the "check" command for verification-only passes.
- **`crates/tempra-cli/src/commands/build.rs`**: Orchestrates the "build" command, selecting between Native AOT and Wasm targets.
- **`crates/tempra-cli/src/commands/install.rs`**: Manages local installation of Tempra binaries to the system path.

## 4. Pipeline Orchestration

- **`crates/tempra-driver/src/pipeline.rs`**: The central coordinator that executes compiler stages in sequence.
- **`crates/tempra-driver/src/build.rs`**: Orchestrates the emission of build artifacts (Wasm modules or Native binaries).
- **`crates/tempra-driver/src/install.rs`**: Handles filesystem moves and metadata updates for installed tools.

## 5. Frontend Stages

- **`crates/tempra-lexer/src/lexer.rs`**: The implementation of the lexical scanner that transforms text into tokens.
- **`crates/tempra-parser/src/parser.rs`**: The recursive descent parser that produces the Abstract Syntax Tree (AST).
- **`crates/tempra-ast/src/exprs.rs`**: Definitions for expression-level AST nodes (loops, matches, if-expressions).
- **`crates/tempra-hir/src/lower.rs`**: Lowers the AST into the High-level Intermediate Representation (HIR), performing name resolution.
- **`crates/tempra-typeck/src/check_expr.rs`**: The core type-checking logic for expressions and structural interface matching.

## 6. Common IR & Middle-end

- **`crates/tempra-mir/src/lower.rs`**: Lowers HIR into the Mid-level IR (MIR), focusing on control-flow graphs and explicit side effects.
- **`crates/tempra-ir/src/module.rs`**: The top-level container for a compiled Tempra program units.
- **`crates/tempra-ir/src/function.rs`**: Defines the structure of functions at the IR level, including signature and metadata.
- **`crates/tempra-ir/src/instr.rs`**: The definition of the Tempra IR instruction set (the platform-neutral "bytecode").
- **`crates/tempra-ir/src/types.rs`**: The representation of types at the IR level, used for backend lowering.

## 7. Execution & Runtime

- **`crates/tempra-interpreter/src/eval.rs`**: The core execution loop for the stack-based Tempra VM.
- **`crates/tempra-interpreter/src/frame.rs`**: Manages activation records, local variables, and the call stack.
- **`crates/tempra-runtime/src/value.rs`**: The unified representation of runtime data (Integers, Strings, ADT payloads).
- **`crates/tempra-stdlib/src/process.rs`**: Bridge logic for system process execution and piping.

## 8. WebAssembly (Wasm)

- **`crates/tempra-backend-wasm/src/lower.rs`**: The lowering pass from Tempra IR directly to Wasm instructions.
- **`crates/tempra-backend-wasm/src/module.rs`**: Handles the serialization of the binary WebAssembly module format (.wasm).
- **`crates/tempra-wasm-host/src/module.rs`**: Integration layer for executing third-party Wasm modules within Tempra.
- **`crates/tempra-component-host/src/bindgen.rs`**: [FUTURE] Placeholder for Wasm Component Model binding generation.

## 9. Native AOT (Ahead-Of-Time)

- **`crates/tempra-backend-cranelift/src/lower.rs`**: Translates Tempra IR instructions into Cranelift IR for machine code generation.
- **`crates/tempra-backend-cranelift/src/abi.rs`**: Implements the Application Binary Interface (ABI) for native platform calling conventions.
- **`crates/tempra-linker/src/exe.rs`**: The wrapper around system linkers (ld, lld) to produce final executables.

## 10. Future-Focused Files

- **`crates/tempra-opt/src/passes/arc.rs`**: [FUTURE] Placeholder for automated ARC elimination and optimization passes.
- **`crates/tempra-opt/src/passes/escape.rs`**: [FUTURE] Placeholder for escape analysis to enable stack promotion of heap values.
- **`crates/tempra-lsp/src/server.rs`**: [FUTURE] Implementation of the Language Server Protocol for IDE integration.
- **`crates/tempra-repl/src/loop.rs`**: [FUTURE] Implementation of the interactive Read-Eval-Print Loop.
