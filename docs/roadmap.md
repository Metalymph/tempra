# Tempra Roadmap

This document outlines the implementation strategy and developmental phases for Tempra, a fast, lightweight scripting language that compiles to native binaries.

## 1. Implementation Phases

### Phase 1: Core Foundation (v0.1) - Current Focus

- **Compiler Frontend:** Implementation of the lexer, recursive descent parser, and Abstract Tree (AST).
- **Semantics:** Initial symbol resolution, type checking for core compound types (`struct`, `enum`), and Zig-style generic stabilization.
- **Pattern Matching:** Centralized pattern semantics with OR-patterns, nesting, and exhaustiveness checking for finite domains.
- **Bytecode VM:** A lightweight stack-based interpreter for rapid cold-starts and low memory overhead in script mode. See [BYTECODE_ISA.md](BYTECODE_ISA.md).
- **Minimal Stdlib:** Foundation for `io`, `env`, and basic string/collection manipulation.
- **Script Mode:** Support for running `.tempra` files directly without a `main` entrypoint.

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

- **Production-wide JIT Compilation:** While a fully-featured, production-wide JIT is not a core goal for standard v0.2 releases, a robust experimental, opt-in Cranelift JIT prototype has been established (Phases 15-17) specifically for performance exploration on hot `Int64` and read-only heap workloads.
- **Heavy VM Runtime:** No GC pause or heavy virtual machine environments.
- **Async/Await Sprawl:** Avoiding the "colored function" problem; preferring lightweight concurrency or explicit event-loop management.
- **Nominal Trait Hierarchies:** No complex inheritance or rigid trait implementations.

---

## 4. Experimental JIT Development & Decision Gates

### Phase 17: Heap-Aware JIT Prototype Slice (COMPLETE)
Established a highly secure, isolated type-safe JIT boundary:
- **Lifetime Isolation**: Context-based heap object tracking (`JitRuntimeContext`), preventing handle identifier leaks.
- **Generic Intrinsics**: Dynamic routing of core operations (`Len`, `Size`, `GetIndex`) through runtime Rust dispatch helper boundaries.
- **Rejection & Fallback Hardening**: Formulated descriptive per-opcode rejection diagnostics and guaranteed transparent fallback execution to the stack interpreter on unsupported operations.

### Phase 18 Decision Gate: Next Pathways
With Phase 17 consolidated, the development team faces a choice of four potential pipelines for Phase 18:
- **Option 18A: Heap Mutation & Allocation Support**
  - Add source-level heap-producing operations to the JIT (e.g. `String.concat`, `Array.push`, `Struct` construction, and dynamic variant payloads).
- **Option 18B: Advanced Diagnostics & IDE Tooling**
  - Integrate structural JIT checks inside LSP/tooling to warn developers of bytecode functions rejected by JIT, and output tracing tools for deoptimization analysis.
- **Option 18C: Language Semantics & Method Dispatch**
  - Implement full method calls, generic functions, and external module calls across the JIT execution boundary.
- **Option 18D: Unified Native AOT & WebAssembly Pipelines**
  - Standardize lowering pipelines so AOT native compilations share optimal Cranelift IR optimizations and target-independent bytecode translations.

