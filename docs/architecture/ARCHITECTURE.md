# Tempra Architecture

This document defines the architectural principles, crate responsibilities, and implementation constraints for the Tempra programming language.

## 1. Design Summary

Tempra is a script-first programming language designed to transition seamlessly from interpreted execution to native binaries. The architecture is built around a multi-stage compilation pipeline that lowers high-level source code into a platform-agnostic intermediate representation (Tempra IR). This IR is the single source of truth for all three execution backends:

1. **Interpreter**: Direct execution of Tempra IR for rapid development and scripting.
2. **Wasm Backend**: Generation of standalone WebAssembly modules and components.
3. **Cranelift Backend**: AOT (Ahead-Of-Time) native machine code generation for performant binaries.

## 2. Architectural Rules

- **Unidirectional Flow**: Information flows only "forward" through the pipeline stages (AST -> HIR -> MIR -> Tempra IR).
- **Representation Purity**: Each stage must have its own distinct data structures. Mixing AST nodes with Typechecked HIR nodes is prohibited.
- **Backend Isolation**: Backends (Interpreter, Wasm, Cranelift) must be entirely isolated and communicate only via Tempra IR.
- **Cranelift usage**: Cranelift is strictly for native AOT. It must **not** be used to generate WebAssembly output.
- **Host vs. Backend**: A backend *generates* code; a host *executes* code. Wasm output and Wasm hosting are separate concerns handled by separate crates.
- **Diagnostic Agnosticism**: Error reporting and source spans are managed independently of the compilation backend.

## 3. Compiler Pipeline

1. **Source** (`tempra-fs`)
2. **Lexer** (`tempra-lexer`)
3. **Parser** (`tempra-parser`) -> **AST** (`tempra-ast`)
4. **Lowering** -> **HIR** (`tempra-hir`)
5. **Type Checking** (`tempra-typeck`)
6. **Lowering** -> **MIR** (`tempra-mir`)
7. **Optimization** (`tempra-opt`) -> **Tempra IR** (`tempra-ir`)
8. **Backend Emission**:
    - `tempra-interpreter` (Virtual Machine execution)
    - `tempra-backend-wasm` (Wasm output)
    - `tempra-backend-cranelift` (Native AOT output)

---

## 4. Crate Responsibilities & File Layouts

### Infrastructure Core

#### tempra-cli

**Purpose**: CLI entrypoint and argument parsing.

- `src/main.rs`: Entrypoint logic.
- `src/args.rs`: CLI argument definitions (using clap or equivalent).
- `src/commands//`: Individual command implementations (run, build, check).

#### tempra-driver

**Purpose**: Orchestrates the compilation pipeline.

- `src/lib.rs`: The main `Driver` struct.
- `src/session_ext.rs`: Integration with `tempra-session`.
- `src/pipeline.rs`: Definition of stages and execution flow.

#### tempra-session

**Purpose**: Global compilation state, target configuration, and inter-crate session data.

- `src/session.rs`: The `Session` struct containing target info and global options.
- `src/config.rs`: Build profiles and configuration file parsing.

#### tempra-span

**Purpose**: Source code location tracking (SourceMap) and spans.

- `src/source.rs`: `SourceFile` and `SourceMap`.
- `src/span.rs`: `Span` and `BytePos` definitions.

#### tempra-diagnostics

**Purpose**: Error reporting, warnings, and diagnostic emission.

- `src/handler.rs`: Central diagnostic handling.
- `src/emitter.rs`: Formatting and terminal output.
- `src/error.rs`: Standardized error codes.

#### tempra-fs

**Purpose**: Abstraction over the filesystem for source loading and artifact emission.

- `src/vfs.rs`: Virtual File System interface for testing and LSP support.
- `src/loader.rs`: Source file loading logic.

### Frontend Stages

#### tempra-lexer

**Purpose**: Tokenization of raw source code.

- `src/lex.rs`: Lexing engine.
- `src/token.rs`: Token definitions and keywords.

#### tempra-parser

**Purpose**: Recursive descent parsing and AST construction.

- `src/parser.rs`: Main parser implementation.
- `src/grammar/`: Individual grammar rules (expressions, statements, declarations).

#### tempra-ast

**Purpose**: Abstract Syntax Tree definitions.

- `src/ast.rs`: AST node structures.
- `src/visit.rs`: Visitor patterns for AST traversal.

#### tempra-hir

**Purpose**: High-level IR (desugared AST, resolved names).

- `src/hir.rs`: HIR node structures.
- `src/lower.rs`: Lowering logic from AST to HIR.

#### tempra-typeck

**Purpose**: Type inference and static analysis.

- `src/check.rs`: Main type checking loop.
- `src/inference.rs`: Type unification and inference engine.
- `src/interface.rs`: Structural interface matching logic.
- `src/refinement.rs`: Branch-local fact tracking and variable narrowing.
- `src/patterns/`: Pattern semantic analysis and exhaustiveness (Domain/Space model).

#### Refinement Architecture

The `Refinement` system manages branch-local semantic facts derived from `is` tests and `match` arms.
- **Refinement facts**: Contains pattern bindings and the identity of the narrowed variable.
- **Context injection**: facts are injected into the scope of the "true" branch.
- **Isolation**: No hidden scope mutation occurs outside the direct refinement path.

#### Pattern & Exhaustiveness

Pattern matching follows a centralized semantic model:
- **Domain**: Represents the set of all possible values for a type (e.g., variants for Enums, True/False for Bool).
- **Space**: Represents the coverage provided by specific patterns.
- **Analysis**: Exhaustiveness is determined by subtracting Space from Domain.
- **Topology**: Generic exhaustiveness is topological; it ignores type parameters and focuses on variant structure.

### Intermediate Stages

#### tempra-mir

**Purpose**: Mid-level IR (Control-flow oriented, explicit side effects).

- `src/mir.rs`: MIR graph structures (Basic Blocks).
- `src/lower.rs`: Lowering logic from HIR to MIR.

#### tempra-ir

**Purpose**: Platform-neutral final IR for all backends.

- `src/ir.rs`: IR instruction set and program representation.
- `src/repr.rs`: Binary/Textual serialization of Tempra IR.

#### tempra-opt

**Purpose**: IR-level optimizations.

- `src/pass.rs`: Optimization pass traits.
- `src/passes/`: Specific optimizations (DCE, inlining, constant folding).

### Execution & Backends

#### tempra-interpreter

**Purpose**: Virtual Machine executing Tempra IR directly.

- `src/vm.rs`: Execution loop and stack management.
- `src/opcodes.rs`: Implementation of IR instructions.

#### tempra-runtime

**Purpose**: Shared runtime primitives (ARC management, intrinsic functions).

- `src/mem.rs`: ARC implementation and allocation strategies.
- `src/intrinsics.rs`: Low-level system functions available to all backends.

#### tempra-stdlib

**Purpose**: The Tempra standard library written in Tempra.

- `io/`, `fs/`, `os/`: Core namespaces.

#### tempra-backend-wasm

**Purpose**: Generates Wasm output (.wasm) from Tempra IR.

- `src/emitter.rs`: Generation of WebAssembly binary format.
- `src/lower.rs`: Lowering Tempra IR to Wasm instructions.

#### tempra-wasm-host

**Purpose**: Execution of third-party Wasm modules (non-Tempra modules).

- `src/host.rs`: Integration with Wasmtime.
- `src/import.rs`: Standardizing imports for external Wasm modules.

#### tempra-component-host

**Purpose**: Wasm Component Model integration and execution.

- `src/component.rs`: Interaction with Wasm component binaries.

#### tempra-backend-cranelift

**Purpose**: Generates native AOT object/binary output using Cranelift.

- `src/translator.rs`: Translating Tempra IR to Cranelift IR.
- `src/codegen.rs`: Native code generation and machine target handling.

#### tempra-linker

**Purpose**: Invokes system linkers to produce final executables.

- `src/bin.rs`: Native binary linking logic.

### Tooling & Support

#### tempra-package

**Purpose**: Dependency management and `tempra.mod` resolution.

- `src/resolver.rs`: Dependency graph resolution.

#### tempra-repl

**Purpose**: Interactive Read-Eval-Print-Loop.

- `src/loop.rs`: Input handling and incremental compilation.

#### tempra-lsp

**Purpose**: Language Server Protocol implementation.

- `src/server.rs`: LSP message handling.

#### tempra-testing

**Purpose**: Compilation and integration test framework.

- `src/expect.rs`: UI/Snapshot testing utilities.

---

## 5. Backend Separation & Build Modes

### Backend Rules

- **Interpretation**: Focused on instant startup. Debug symbols should be preserved.
- **Wasm**: Focused on isolation and interoperability. Should not assume a specific native host.
- **Native AOT**: Focused on performance. Leverages Cranelift for backend machine optimizations.

### Build Modes

- **`fast`**:
  - Target: `tempra-backend-cranelift`.
  - Optimizations: O3 (via `tempra-opt`) + Native codegen opts.
  - Runtime: Aggressive ARC elimination and stack promotion.
- **`light`**:
  - Target: Any (usually Native or Wasm).
  - Optimizations: Size-oriented passes (OZ).
  - Runtime: Minimum runtime footprint, stripped symbols.

## 6. Implementation Ordering & Constraints

### Sequencing

1. **Core Foundation**: `tempra-span`, `tempra-session`, `tempra-diagnostics`.
2. **Frontend Plumbing**: `tempra-lexer`, `tempra-parser`, `tempra-ast`.
3. **Semantic Analysis**: `tempra-hir`, `tempra-typeck`.
4. **Backend Target**: `tempra-ir`.
5. **Execution (Priority 1)**: `tempra-interpreter` (essential for rapid iteration).
6. **Codegen (Priority 2)**: `tempra-backend-cranelift`, `tempra-backend-wasm`.

### Source of Truth

The **Tempra IR** remains the canonical representation of a program once semantic checks pass. No backend is permitted to perform proprietary semantic changes to the program logic. Any logic optimization that affects program semantics must occur in `tempra-opt` before reaching the backend emitters.
