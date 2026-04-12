# Fero

**Fero** is a fast, lightweight scripting language that compiles to native binaries.

Write scripts. Ship binaries.

## Goals

Fero is designed around two primary workflows:

- **script-first development**: immediate execution, low cold-start, low memory usage
- **native deployment**: compile to fast native binaries with AOT

Fero is intentionally not trying to be a giant runtime platform. The design favors a small, disciplined language core, explicit semantics, strong tooling, and a clear path from script to binary.

## Core Design

Fero currently targets this architecture:

- **default execution**: interpreter
- **native compilation**: Cranelift AOT
- **Wasm output**: dedicated Wasm backend
- **Wasm hosting**: separate host layer using Wasmtime in the future
- **no JIT for now**
- **no heavy VM for now**

The language philosophy is:

- minimal syntax
- direct semantics
- explicit errors
- strong scripting ergonomics
- native binaries when needed

## Language Direction

Planned language characteristics:

- one versatile `for`
- `for` can also be an expression
- `if` is an expression
- `match` is an expression
- no exceptions
- explicit errors only
- core compound types are `struct` and `enum`
- structural static interfaces instead of a heavy trait system
- script mode without mandatory `main`
- package/app mode with an entrypoint

## Main Commands

Fero is designed to be as ergonomic as a shell script while providing the power of a compiled language.

- **Execution & Development**
  - `fero run file.fero`: Default execution mode via the interpreter for low cold-start and immediate feedback.
  - `fero repl`: Start an interactive session for quick prototyping and testing.
  - `fero check file.fero`: Perform syntax analysis and type checking without executing or emitting a binary.

- **Building & Tooling**
  - `fero build file.fero`: Compile a source file to a standalone binary or Wasm module.
  - `fero install file.fero`: Compile and install the native binary to your user path (e.g., `~/.fero/bin`).
  - `fero test`: Run all tests defined within the current package or file.
  - `fero fmt`: Automatically format code according to the official style guide.

```bash
# Example build commands
fero build main.fero --target native --mode fast
fero build app.fero --target wasm32-wasi --mode light
```

## Build Modes

Fero provides two primary build modes to balance binary characteristics:

- **`fast`**: Prioritizes runtime execution speed. In AOT mode, this enables aggressive optimizations, Perceus-inspired ARC elimination, and SIMD optimizations where applicable.
- **`light`**: Prioritizes minimal binary size and low memory usage. This mode focuses on dead-code elimination and stripping non-essential metadata, ideal for small CLI tools and constrained Wasm environments.

## Compiler Pipeline

The Fero compiler transforms source code through a series of intermediate representations before reaching one of the three supported execution/output paths.

1. **Source**: Plain text `.fero` files.
2. **Lexer/Parser**: Generation of the **AST** (Abstract Syntax Tree).
3. **HIR** (High-Level IR): Desugaring and initial name resolution.
4. **Type Checking**: Static structural interface resolution and type inference.
5. **MIR** (Middle-end IR): Control flow analysis and basic optimizations.
6. **Fero IR**: The final platform-neutral representation used as the source for all backends.

**Execution Paths from Fero IR:**

- `->` **Interpreter**: Direct stack-based execution (Script mode).
- `->` **Wasm Backend**: Custom backend targeting WebAssembly.
- `->` **Cranelift Backend**: AOT native machine code generation.

## Wasm Strategy

Fero treats WebAssembly as a first-class compilation target, distinct from native machine code.

- **Wasm Output**: Handled by a dedicated backend that emits `.wasm` modules directly from Fero IR. **Note**: The Wasm backend is independent of Cranelift; Fero does not use Cranelift to generate Wasm.
- **Wasm Hosting**: Separate from the compiler, Fero will leverage **Wasmtime** as the preferred host for executing Wasm components in a secure, sandboxed environment.

## Native AOT Strategy

Fero utilizes the **Cranelift** code generator exclusively for its **Native AOT** execution path.

- **Native Optimized**: Cranelift transforms Fero IR into optimized machine code (x86_64, AArch64) for high-performance CLI tools and system binaries.
- **Architecture Constraint**: Support for Cranelift is strictly limited to native targets. It is not involved in the Wasm compilation pipeline or the interpreted script mode.

## Roadmap

Near-term priorities:

1. Lexer, parser, AST, and source diagnostics
2. Typed HIR with a practical scripting-oriented type system
3. Interpreter-first execution for scripts and REPL
4. Native AOT through Cranelift
5. Dedicated Wasm backend for distribution and embedding

Longer-term directions:

- ARC with aggressive elimination
- escape analysis for native fast mode
- SIMD
- lightweight concurrency
- minimal effects system

## Repository Layout

```text
fero/
├─ Cargo.toml
├─ rust-toolchain.toml
├─ README.md
├─ docs/
│  ├─ architecture/
│  │  ├─ ARCHITECTURE.md
│  │  ├─ PIPELINE.md
│  │  └─ FILE_MAP.md
│  └─ roadmap.md
├─ specs/
│  ├─ syntax-v0.1.md
│  ├─ module-system-v0.1.md
│  └─ effects-v0.1.md
├─ examples/
├─ tests/
└─ crates/
   ├─ fero-cli/
   ├─ fero-driver/
   ├─ fero-session/
   ├─ fero-span/
   ├─ fero-diagnostics/
   ├─ fero-fs/
   ├─ fero-lexer/
   ├─ fero-parser/
   ├─ fero-ast/
   ├─ fero-hir/
   ├─ fero-typeck/
   ├─ fero-mir/
   ├─ fero-ir/
   ├─ fero-opt/
   ├─ fero-interpreter/
   ├─ fero-runtime/
   ├─ fero-stdlib/
   ├─ fero-backend-wasm/
   ├─ fero-wasm-host/
   ├─ fero-component-host/
   ├─ fero-backend-cranelift/
   ├─ fero-linker/
   ├─ fero-package/
   ├─ fero-repl/
   ├─ fero-lsp/
   └─ fero-testing/
```

## Current State

This repository is a serious bootstrap, not a finished compiler. Each crate starts with a narrow responsibility, explicit boundaries, and compileable placeholders so the implementation can grow without reworking the project shape.
