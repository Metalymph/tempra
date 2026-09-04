# JIT Phase 17 Closure & Release Checklist

This document formally closes **Phase 17** of the experimental heap-aware JIT compiler for the Tempra language. It establishes the stable baseline for heap-aware JIT operations, ensuring robust boundaries, graceful deoptimization fallbacks, clear diagnostics, and comprehensive test coverage.

---

## 1. Summary of Phase 17 Achievements

Phase 17 has transformed the experimental Cranelift JIT compiler from a primitive `Int64`-only compiler into a type-safe, heap-aware execution layer.

- **Phase 17.1 - 17.3 (Lifetime Scoping & Profiling)**:
  - Formulated a dynamic `JitRuntimeContext` allocation scheme to isolate intermediate heap object lifetimes, completely eliminating cross-invocation handle leaks.
  - Hardened JIT profiling telemetry via the VM profile builder, allowing detailed deoptimization stats and execution tracking.
- **Phase 17.4 - 17.5 (Generic Intrinsics Layer)**:
  - Transitioned from ad-hoc, type-specific compiled helper dispatches to a centralized, type-safe generic intrinsics layer (`tmpr_jit_generic_len`, `tmpr_jit_generic_size`, `tmpr_jit_generic_get_i64`).
  - Allowed the compiler frontend to generate unified bytecode operations while moving heap-type routing to runtime pattern-matching in Rust.
- **Phase 17.6 - 17.7 (Diagnostics, Fallbacks & CLI Smoke)**:
  - Replaced ambiguous phase-number rejection diagnostics with explicit, operation-named diagnostic messages (e.g. `"Array.push is not supported in source-level JIT v1"`).
  - Verified robust fallback to the bytecode stack interpreter on all unsupported operations with zero panics or correctness errors.
  - Implemented 5 canonical source-level heap examples under `examples/jit_heap/` along with automation commands in `justfile`.

---

## 2. Frozen Scope Boundary (Milestone v0.2)

To maintain absolute stability, the JIT boundary for Phase 17 is frozen to the following exact subset. Any operation outside of this subset will trigger JIT eligibility rejection and transparently fall back to the interpreter.

### ✅ Supported Subset
- **String length extraction**: `String.len()` (returns count of Unicode scalar values).
- **String byte-size extraction**: `String.size()` (returns raw UTF-8 byte count).
- **Array length extraction**: `Array.len()` (returns the element count).
- **Array element retrieval**: `Array.get_i64` (`xs[index]` for `Int64` arrays).
- **Mixed arithmetic and read-only heap expressions**.

### ❌ Explicitly Rejected / Interpreter Fallback
- **Heap mutations**: `Array.push`, `String.concat` (at source-level).
- **Structs**: Struct construction, field getters (`Struct.get`), and field setters (`Struct.set`).
- **Enums**: Enum variants, `Tag` extraction, and variant `Unwrap`.
- **Option / Result types**: Desugared variant branches are rejected.
- **Returning Heap Handles**: Any JIT function returning a heap-allocated type (`String`, `Any`) is rejected (must return `Int64` or primitive types).

---

## 3. Examples & Demonstration Commands

The canonical heap JIT demonstration suite is located in [`examples/jit_heap/`](../examples/jit_heap/):
1. [`string_len.tmpr`](../examples/jit_heap/string_len.tmpr) (expected: 2)
2. [`string_size.tmpr`](../examples/jit_heap/string_size.tmpr) (expected: 3)
3. [`array_len.tmpr`](../examples/jit_heap/array_len.tmpr) (expected: 3)
4. [`array_get.tmpr`](../examples/jit_heap/array_get.tmpr) (expected: 20)
5. [`mixed_heap_int.tmpr`](../examples/jit_heap/mixed_heap_int.tmpr) (expected: 28)

---

## 4. Phase 17 Release & Validation Checklist

Before releasing or expanding JIT capabilities, the following checklist must be run locally to guarantee zero regressions.

```text
Phase 17 JIT Validation Checklist:
[ ] cargo check --workspace
[ ] cargo check --workspace --features jit-cranelift
[ ] cargo test --workspace --features jit-cranelift
[ ] just jit-heap
[ ] just jit-heap-smoke
[ ] just jit-heap-perf
```

These checks are consolidated into a single milestone verification command:
```bash
just jit-phase17
```
