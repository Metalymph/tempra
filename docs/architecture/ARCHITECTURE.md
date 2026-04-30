# Fero Architecture

This document defines the architectural principles, crate responsibilities, and implementation constraints for the Fero programming language.

## 1. Design Summary

Fero is a script-first programming language designed to transition seamlessly from interpreted execution to native binaries. The architecture is built around a multi-stage compilation pipeline that lowers high-level source code into a platform-agnostic intermediate representation (Fero IR). This IR is the single source of truth for all three execution backends:

1. **Interpreter**: Direct execution of Fero IR for rapid development and scripting.
2. **Wasm Backend**: Generation of standalone WebAssembly modules and components.
3. **Cranelift Backend**: AOT (Ahead-Of-Time) native machine code generation for performant binaries.

## 2. Architectural Rules

- **Unidirectional Flow**: Information flows only "forward" through the pipeline stages (AST -> HIR -> MIR -> Fero IR).
- **Representation Purity**: Each stage must have its own distinct data structures. Mixing AST nodes with Typechecked HIR nodes is prohibited.
- **Backend Isolation**: Backends (Interpreter, Wasm, Cranelift) must be entirely isolated and communicate only via Fero IR.
- **Cranelift usage**: Cranelift is strictly for native AOT. It must **not** be used to generate WebAssembly output.
- **Host vs. Backend**: A backend *generates* code; a host *executes* code. Wasm output and Wasm hosting are separate concerns handled by separate crates.
- **Diagnostic Agnosticism**: Error reporting and source spans are managed independently of the compilation backend.

## 3. Compiler Pipeline

1. **Source** (`fero-fs`)
2. **Lexer** (`fero-lexer`)
3. **Parser** (`fero-parser`) -> **AST** (`fero-ast`)
4. **Lowering** -> **HIR** (`fero-hir`)
5. **Type Checking** (`fero-typeck`)
6. **Lowering** -> **MIR** (`fero-mir`)
7. **Optimization** (`fero-opt`) -> **Fero IR** (`fero-ir`)
8. **Backend Emission**:
    - `fero-interpreter` (Virtual Machine execution)
    - `fero-backend-wasm` (Wasm output)
    - `fero-backend-cranelift` (Native AOT output)

---

## 4. Crate Responsibilities & File Layouts

### Infrastructure Core

#### fero-cli

**Purpose**: CLI entrypoint and argument parsing.

- `src/main.rs`: Entrypoint logic.
- `src/args.rs`: CLI argument definitions (using clap or equivalent).
- `src/commands//`: Individual command implementations (run, build, check).

#### fero-driver

**Purpose**: Orchestrates the compilation pipeline.

- `src/lib.rs`: The main `Driver` struct.
- `src/session_ext.rs`: Integration with `fero-session`.
- `src/pipeline.rs`: Definition of stages and execution flow.

#### fero-session

**Purpose**: Global compilation state, target configuration, and inter-crate session data.

- `src/session.rs`: The `Session` struct containing target info and global options.
- `src/config.rs`: Build profiles and configuration file parsing.

#### fero-span

**Purpose**: Source code location tracking (SourceMap) and spans.

- `src/source.rs`: `SourceFile` and `SourceMap`.
- `src/span.rs`: `Span` and `BytePos` definitions.

#### fero-diagnostics

**Purpose**: Error reporting, warnings, and diagnostic emission.

- `src/handler.rs`: Central diagnostic handling.
- `src/emitter.rs`: Formatting and terminal output.
- `src/error.rs`: Standardized error codes.

#### fero-fs

**Purpose**: Abstraction over the filesystem for source loading and artifact emission.

- `src/vfs.rs`: Virtual File System interface for testing and LSP support.
- `src/loader.rs`: Source file loading logic.

### Frontend Stages

#### fero-lexer

**Purpose**: Tokenization of raw source code.

- `src/lex.rs`: Lexing engine.
- `src/token.rs`: Token definitions and keywords.

#### fero-parser

**Purpose**: Recursive descent parsing and AST construction.

- `src/parser.rs`: Main parser implementation.
- `src/grammar/`: Individual grammar rules (expressions, statements, declarations).

#### fero-ast

**Purpose**: Abstract Syntax Tree definitions.

- `src/ast.rs`: AST node structures.
- `src/visit.rs`: Visitor patterns for AST traversal.

#### fero-hir

**Purpose**: High-level IR (desugared AST, resolved names).

- `src/hir.rs`: HIR node structures.
- `src/lower.rs`: Lowering logic from AST to HIR.

#### fero-typeck

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

#### fero-mir

**Purpose**: Mid-level IR (Control-flow oriented, explicit side effects).

- `src/mir.rs`: MIR graph structures (Basic Blocks).
- `src/lower.rs`: Lowering logic from HIR to MIR.

#### fero-ir

**Purpose**: Platform-neutral final IR for all backends.

- `src/ir.rs`: IR instruction set and program representation.
- `src/repr.rs`: Binary/Textual serialization of Fero IR.

#### fero-opt

**Purpose**: IR-level optimizations.

- `src/pass.rs`: Optimization pass traits.
- `src/passes/`: Specific optimizations (DCE, inlining, constant folding).

### Execution & Backends

#### fero-interpreter

**Purpose**: Virtual Machine executing Fero IR directly.

- `src/vm.rs`: Execution loop and stack management.
- `src/opcodes.rs`: Implementation of IR instructions.

#### fero-runtime

**Purpose**: Shared runtime primitives (ARC management, intrinsic functions).

- `src/mem.rs`: ARC implementation and allocation strategies.
- `src/intrinsics.rs`: Low-level system functions available to all backends.

#### fero-stdlib

**Purpose**: The Fero standard library written in Fero.

- `io/`, `fs/`, `os/`: Core namespaces.

#### fero-backend-wasm

**Purpose**: Generates Wasm output (.wasm) from Fero IR.

- `src/emitter.rs`: Generation of WebAssembly binary format.
- `src/lower.rs`: Lowering Fero IR to Wasm instructions.

#### fero-wasm-host

**Purpose**: Execution of third-party Wasm modules (non-Fero modules).

- `src/host.rs`: Integration with Wasmtime.
- `src/import.rs`: Standardizing imports for external Wasm modules.

#### fero-component-host

**Purpose**: Wasm Component Model integration and execution.

- `src/component.rs`: Interaction with Wasm component binaries.

#### fero-backend-cranelift

**Purpose**: Generates native AOT object/binary output using Cranelift.

- `src/translator.rs`: Translating Fero IR to Cranelift IR.
- `src/codegen.rs`: Native code generation and machine target handling.

#### fero-linker

**Purpose**: Invokes system linkers to produce final executables.

- `src/bin.rs`: Native binary linking logic.

### Tooling & Support

#### fero-package

**Purpose**: Dependency management and `fero.mod` resolution.

- `src/resolver.rs`: Dependency graph resolution.

#### fero-repl

**Purpose**: Interactive Read-Eval-Print-Loop.

- `src/loop.rs`: Input handling and incremental compilation.

#### fero-lsp

**Purpose**: Language Server Protocol implementation.

- `src/server.rs`: LSP message handling.

#### fero-testing

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
  - Target: `fero-backend-cranelift`.
  - Optimizations: O3 (via `fero-opt`) + Native codegen opts.
  - Runtime: Aggressive ARC elimination and stack promotion.
- **`light`**:
  - Target: Any (usually Native or Wasm).
  - Optimizations: Size-oriented passes (OZ).
  - Runtime: Minimum runtime footprint, stripped symbols.

## 6. Implementation Ordering & Constraints

### Sequencing

1. **Core Foundation**: `fero-span`, `fero-session`, `fero-diagnostics`.
2. **Frontend Plumbing**: `fero-lexer`, `fero-parser`, `fero-ast`.
3. **Semantic Analysis**: `fero-hir`, `fero-typeck`.
4. **Backend Target**: `fero-ir`.
5. **Execution (Priority 1)**: `fero-interpreter` (essential for rapid iteration).
6. **Codegen (Priority 2)**: `fero-backend-cranelift`, `fero-backend-wasm`.

### Source of Truth

The **Fero IR** remains the canonical representation of a program once semantic checks pass. No backend is permitted to perform proprietary semantic changes to the program logic. Any logic optimization that affects program semantics must occur in `fero-opt` before reaching the backend emitters.
