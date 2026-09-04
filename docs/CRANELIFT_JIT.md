# Cranelift JIT Prototype

This document describes the Phase 15.8 prototype of a Just-In-Time (JIT) compiler for the Tempra VM using [Cranelift](https://cranelift.dev/).

## Objective

The goal of this prototype is to demonstrate the ability to compile hot bytecode functions into native machine code to reduce execution overhead. The prototype focuses on a minimal subset of the language to establish the end-to-end pipeline:
1. Identify JIT-candidate functions.
2. Lower Tempra bytecode to Cranelift IR.
3. Compile and finalize native code.
4. Execute native code via a safe wrapper.

## Supported Scope (v0.1)

The prototype currently supports an `Int64 + internal Bool + Heap Read-Only Subset`:
- **Types**: `Int64` parameters, return values, and locals. `Bool` local variables (internal only). Heap handles (`String`, `Array`) passed opaquely via Context Marshalling.
- **Heap Operations (v0.2)**: `String.len`, `String.size`, `Array.len`, `Array.get_i64`.
- **Arithmetic**: `Add`, `Sub`, `Mul`, `Div` (signed).
- **Comparisons**: `Eq`, `Lt`, `Gt`.
- **Boolean Logic**: `Not`, `And`, `Or` (logical/bitwise).
- **Control Flow**: 
    - Conditional branches (`JumpIfFalse`).
    - Unconditional jumps (`Jump`).
    - Recursive and iterative loops.
    - Deeply nested `if-else` and `else-if` chains.
- **Function Calls (v1)**: 
    - Static `Call` to other JIT-eligible Int64 functions.
    - Automatic pre-compilation of dependencies.
- **Superinstructions**:
    - `AddI64LL` (Fused local-local add).
    - `AddI64LC` (Fused local-constant add).

### Unsupported Features (Fallback to VM)
Any function using the following features is currently rejected by the JIT and will execute via the standard stack interpreter:
- **Bool in ABI**: Boolean parameters or return types are rejected (ABI mismatch). Bools are only supported as internal locals.
- **Heap Mutation**: `ArrayPush`, `MakeArray`, `Slice` (mutation or production of new heap handles is isolated to interpreter until Phase 18).
- Structs, Enums.
- Option, Result.
- Recursion (recursive function calls are currently rejected to prevent compilation loops).
- Closures and methods.
- Floating point or unsigned arithmetic.
- Functions returning heap handles (`String`, `Any`).

## Source-Level JIT Operation Status (v0.2)

| Operation       | Source-level JIT | Behavior on rejection     |
|-----------------|------------------|---------------------------|
| `String.len`    | ✅ Supported      | —                         |
| `String.size`   | ✅ Supported      | —                         |
| `Array.len`     | ✅ Supported      | —                         |
| `Array.get_i64` | ✅ Supported      | —                         |
| `String.concat` | ❌ Rejected       | VM fallback (interpreter) |
| `Array.push`    | ❌ Rejected       | VM fallback (interpreter) |
| `Struct.get`    | ❌ Rejected       | VM fallback (interpreter) |
| `Struct.set`    | ❌ Rejected       | VM fallback (interpreter) |
| `Enum.tag`      | ❌ Rejected       | VM fallback (interpreter) |
| `Enum.unwrap`   | ❌ Rejected       | VM fallback (interpreter) |

## Implementation Details

### Stack-to-SSA Lowering
Tempra's stack-based bytecode is lowered to Cranelift's SSA-based IR. The lowering pass simulates the operand stack during translation using a `Vec<Value>`. Local variables are mapped to Cranelift `Variable`s, allowing the Cranelift frontend to handle register allocation and SSA construction automatically.

### Block Management
Jump targets are pre-scanned to create Cranelift `Block`s. To ensure valid SSA, blocks are sealed only after all predecessors are identified.

### Architecture Support
The JIT is configured to support both `x86_64` and `AArch64` (Apple Silicon). On AArch64, the ISA is specifically tuned to avoid PLT requirements for simple JIT usage.

## VM Integration (Phase 15.1)

The JIT is integrated into the Tempra VM as an opt-in component.

### Usage
To enable the JIT, use `VmOptions`:
```rust
let mut options = VmOptions::default();
options.jit_enabled = true;
options.hot_call_threshold = 1000;
run_function_with_options(module, "main", args, options);
```

## Performance Tracking (Phase 15.4)

The prototype exposes precise timings through `VmProfile` to evaluate Time-To-Profitability. The metrics include:
- `jit_compile_time_ns`: Total time spent translating and emitting machine code (aggregate for dependencies in v0 policy).
- `jit_total_native_time_ns`: Total duration executing inside the JIT wrapper (only active when profiling is enabled).
- `jit_rejected_count`: Explicit occurrences where the Engine attempts compilation but actively refuses.

**Cold vs Warm Benchmarks:**
The performance harness in `tempra-driver/tests/perf_baseline.rs` introduces isolated benchmarks to distinguish between a "Cold Run" (where compilation latency masks native speedup) and a "Warm Run" (where functions are pulled directly from the compiled memory cache). This facilitates exact Time-To-Profitability configurations.

### Fallback Policy
The VM always maintains the stack-based interpreter as the primary runtime. If:
1. A function uses unsupported types (Strings, Structs, etc.).
2. JIT compilation fails.
3. The function signature is not compatible with the JIT ABI.

Then the VM transparently falls back to interpreting the bytecode, ensuring correctness over performance.

### JIT Cache
Compiled functions are stored in an in-memory cache (`HashMap<FunctionId, JitFunctionI64>`) within the `Interpreter` instance.

## JIT Observability (Phase 15.2)

To ensure JIT behavior is transparent and safe, the VM includes detailed observability metrics in its profiling output:

### Profiling Metrics
When `VmOptions::profiling` and `jit_enabled` are true, `VmProfile` tracks:
- **Compile Attempts/Successes/Failures**: How often the VM tried to compile a function and the result.
- **Runtime Calls**: How many times the native compiled code was executed.
- **Fallback Calls**: How many times a function with a compiled version fell back to the interpreter (e.g., due to incompatible arguments).

### Eligibility Diagnostics and Rejection Caching
Not all functions can be compiled to native code. The JIT provides diagnostics (`JitEligibility`) explaining *why* a function is rejected (e.g., `UnsupportedType("String")`).
When a compilation fails or is rejected, the VM records the reason in the function's profile and **caches the rejection**. This prevents the VM from repeatedly attempting to compile an unsupported function, maintaining performance.

## Verification

The JIT is verified via differential tests comparing VM results with native execution:
```bash
cargo test -p tempra-jit-cranelift
```

## JIT Policy v1 (Phase 15.5)

### VmOptions Reference

| Field | Default | Description |
|---|---|---|
| `jit_enabled` | `false` | Master switch. JIT is opt-in only. |
| `hot_call_threshold` | `100` | Minimum call count before a function is considered hot. |
| `hot_backedge_threshold` | `1000` | Minimum backedge count (loop iterations) to declare hotness. |
| `jit_min_instruction_count` | `8` | Functions with fewer bytecode instructions are never JIT-compiled. Tiny functions (e.g. single-expression wrappers) pay more in Cranelift overhead than they save. |
| `jit_retry_limit` | `1` | Maximum Cranelift compilation failures allowed before a function is permanently rejected. Set to 1 means: one attempt; if it fails, never try again. |

### Compilation Decision (per call-site)

The guard in `maybe_compile_jit` evaluates in this order:

1. `jit_enabled` is false → return immediately, no stats recorded.
2. `jit_is_permanently_rejected` is set → return immediately (zero-cost cache hit).
3. `call_count < hot_call_threshold && backedge_count < hot_backedge_threshold` → `jit_skipped_below_threshold++` and return.
4. `bytecode_len < jit_min_instruction_count` → `jit_skipped_too_small++` and return.
5. `jit_compile_failures >= jit_retry_limit` → `jit_skipped_retry_limit++` and return.
6. Proceed to eligibility check (`explain_jit_eligibility`):
   - Ineligible → `jit_is_permanently_rejected = true`, `jit_permanent_rejections++`.
7. Invoke Cranelift:
   - Success → store in cache, record `jit_compile_time_ns`.
   - Failure → `jit_compile_failures++`. If failures now `>= retry_limit`: mark permanently rejected. Otherwise: `jit_retryable_failures++`.

> **Note:** `jit_skipped_below_threshold` is a *decision-check counter*, not a unique-functions counter. It increments every time a non-hot function is encountered at the call-site. For long-running programs with many cold calls, this number can be large and is diagnostic only.

### Policy Stats in VmProfile::summary()

```
JIT Policy:
  skipped below threshold: N   (decision checks where function was not yet hot)
  skipped too small:       N   (bytecode len < jit_min_instruction_count)
  retry-limit skips:       N   (permanently failed functions bypassed)
  permanent rejections:    N   (unsupported types or exhausted retries)
```

### Current Recommendation (v1)

- **JIT opt-in only**: leave `jit_enabled = false` in production until threshold tuning is complete.
- **High thresholds**: default `hot_call_threshold = 100` is conservative. For compute-heavy workloads that run thousands of calls, lower to 20–50.
- **No JIT for tiny functions**: `jit_min_instruction_count = 8` is a safe floor. Micro-functions (simple getters, one-expression helpers) cost more to compile than they save.
- **Retry limit = 1**: a single transient Cranelift failure on a given function is treated as permanent. Raise to 2 only if you observe false transient failures in practice.

## CLI Usage (Phase 15.6)

The JIT is available in the `tempra` CLI as an experimental, opt-in feature.

### Prerequisites

To use the JIT, the CLI must be compiled with the `jit-cranelift` feature enabled:

```bash
cargo build --features jit-cranelift
```

### Execution

To run a script with the JIT enabled, you must provide both the `--experimental-vm` and `--experimental-jit` flags:

```bash
tempra run script.tmpr --experimental-vm --experimental-jit
```

If you provide `--experimental-jit` without `--experimental-vm`, the CLI will return an error. If the CLI was not built with `jit-cranelift` support, enabling the flag will also result in a clear error message.

### Observability

To see JIT statistics from the CLI, you can use the profiling mode (if exposed or via test-drivers):

```bash
# Example if using a driver that supports profiling
tempra run examples/jit_benchmark.tmpr --experimental-vm --experimental-jit
```

Currently, the CLI does not automatically print the `VmProfile` summary to stdout unless specifically triggered by a profiling flag (planned for future phases).

## Stability Burn-in (Phase 15.7)

To ensure long-term stability, a specialized burn-in corpus is maintained in `examples/jit_burnin/`. These scripts are used to validate:
- **Semantic Parity**: JIT and VM must agree on all results.
- **Graceful Rejection**: Unsupported features (Strings, Structs) must never cause a crash, only a clean fallback.
- **Warm-run Integrity**: Compiled functions must be reused correctly across multiple calls.

## Future: Heap Boundary (Phase 16.0+)

As the JIT expands beyond primitives, it adopts a strict **Heap Boundary** design to maintain safety and VM isolation:
- **Opaque Handles**: All heap types (String, Array, etc.) are treated as opaque `u64` tokens.
- **Runtime Helpers**: Native code interacts with the heap exclusively via VM-provided helper functions.
- **Fallible ABI**: Soft errors (bounds checks) use a `(Value, Status)` return pattern to avoid unnecessary traps.
- **Generic Intrinsics (Phase 17.5)**: Source-level JIT execution routes generic instructions (`Len`, `Size`, `GetIndex`) through generic helper intrinsics (`tmpr_jit_generic_len`, `tmpr_jit_generic_size`, `tmpr_jit_generic_get_i64`). These dynamic dispatchers enforce language-level type correctness and routing, while specialized type-specific helpers act as granular building blocks.

See [JIT_HEAP_BOUNDARY.md](JIT_HEAP_BOUNDARY.md) for full details.

## Running Heap-aware JIT Examples

The `examples/jit_heap/` directory contains five source-level examples that exercise the supported read-only heap subset. Each file includes an `// expected: N` comment with the expected integer return value.

| File | Operation tested | Expected |
|---|---|---|
| `string_len.tmpr` | `String.len` | `2` |
| `string_size.tmpr` | `String.size` | `3` |
| `array_len.tmpr` | `Array.len` | `3` |
| `array_get.tmpr` | `Array.get_i64` | `20` |
| `mixed_heap_int.tmpr` | All four combined | `28` |

### Quick commands

```bash
# Run the integration test suite (19 tests, includes Phase 17.4/17.5/17.6)
just jit-heap

# CLI smoke test — runs all 5 examples; exit code 0 = pass
just jit-heap-smoke

# Performance harness (ignored by default, requires --ignored)
just jit-heap-perf
```

### Manual run

```bash
cargo run -p tempra-cli --features jit-cranelift -- run \
  examples/jit_heap/string_len.tmpr \
  --experimental-vm --experimental-jit
```

> **Note:** Unsupported operations (Struct, Enum, `Array.push`, `String.concat`) trigger
> a permanent JIT rejection and fall back transparently to the standard stack interpreter.
> No panic occurs and no correctness is lost.
