# Fero Roadmap

This document outlines the implementation strategy and developmental phases for Fero, a fast, lightweight scripting language that compiles to native binaries.

## 1. Implementation Phases

### Phase 1: Core Foundation (v0.1) - Current Focus

- **Compiler Frontend:** Implementation of the lexer, recursive descent parser, and Abstract Tree (AST).
- **Semantics:** Initial symbol resolution, type checking for core compound types (`struct`, `enum`), and static structural interface resolution.
- **Bytecode VM:** A lightweight stack-based interpreter for rapid cold-starts and low memory overhead in script mode.
- **Minimal Stdlib:** Foundation for `io`, `env`, and basic string/collection manipulation.
- **Script Mode:** Support for running `.fero` files directly without a `main` entrypoint.

### Phase 2: Native Compilation (v0.2)

- **Cranelift Backend:** Integration with Cranelift for Ahead-Of-Time (AOT) native code generation.
- **Build Modes:**
  - `light`: Default AOT mode optimized for binary size.
  - `fast`: Initial runtime optimizations.
- **App Mode:** Formalization of the package system and `main` entrypoint requirement for binaries.
- **Extended Stdlib:** Implementation of `fs`, `path`, and basic `process` execution.

### Phase 3: Wasm & Advanced Runtime (v0.3)

- **Wasm Backend:** Dedicated output support for WebAssembly MVP.
- **ARC Core:** Implementation of the base Automatic Reference Counting (ARC) memory management.
- **Module System:** Full versioning and dependency resolution for external packages.
- **Standard Library:** Completion of `os`, `time`, and advanced `cmd` piping support.

### Phase 4: Optimization & Maturity (v1.0)

- **Fast Mode Enhancements:** Perceus-inspired ARC elimination and escape analysis for stack promotion.
- **Comptime:** Design and implementation of compile-time execution for generics and specialization.
- **Wasm Hosting:** Integration with Wasmtime for specialized Wasm component hosting.
- **Concurrency:** Introduction of lightweight event-loop mechanics or explicit threading/channels.

## 2. Technical Priorities

### Immediate Priorities

1. **Frontend Stability:** Ensuring the AST and IR (Intermediate Representation) are robust enough to support both interpretation and AOT.
2. **Structural Interfaces:** Finalizing the rules for static duck typing to ensure zero-cost abstractions where possible.
3. **IO/FS Basics:** Providing the essential tools needed for 80% of scripting tasks.

### Future Priorities

1. **Memory Optimization:** Reducing ARC overhead in the AOT backend.
2. **SIMD Support:** Exploiting hardware capabilities in `fast` build mode.
3. **Minimal Effect System:** Exploring structured error handling and resource management.

## 3. Explicitly Out of Scope (Non-Goals)

- **JIT Compilation:** Fero prioritizes low cold-start (interpreter) and predictable performance (AOT) over JIT complexity.
- **Heavy VM Runtime:** No GC pause or heavy virtual machine environments.
- **Async/Await Sprawl:** Avoiding the "colored function" problem; preferring lightweight concurrency or explicit event-loop management.
- **Nominal Trait Hierarchies:** No complex inheritance or rigid trait implementations.
