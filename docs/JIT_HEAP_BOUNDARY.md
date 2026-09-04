# JIT Heap Boundary Design

> [!WARNING]
> **Historical experimental boundary.** This document describes the opt-in Cranelift JIT runtime prototype. Its handle ownership and possible-GC language do not define Tempra's normative ownership model. The safe-language acyclic ownership rules belong to roadmap Phase M.

This document defines the interface and ownership model for the Tempra Cranelift JIT to support heap-allocated types in future phases.

## 1. Objective

The primary goal is to establish a safe, stable boundary between native compiled code (Cranelift) and the VM-managed heap. This allows the JIT to eventually support Strings, Arrays, Structs, and Enums without becoming coupled to the internal memory layout of the VM.

## 2. ABI Model

### 2.1 Primitive Types
| Tempra Type | Native ABI Type | Description |
|---|---|---|
| `Int64` | `i64` | Native 64-bit signed integer. |
| `Bool` | `i8` | Internal representation only. `0` for false, `1` for true. |

### 2.2 Heap Types (Opaque Handle ABI)
All heap-allocated values are represented in the JIT as a **HeapHandle**.

- **Type**: `u64` (opaque token).
- **Policy**: The JIT compiler and the generated native code must treat this value as an opaque identifier. Native code **MUST NOT** attempt to dereference this handle or assume it is a raw pointer.
- **V0 Encoding**: Currently encoded as a 64-bit index/pointer, but the specific representation is subject to change.

## 3. Ownership & Memory Model

- **VM Supremacy**: The VM's heap manager (and eventually its GC) maintains absolute ownership of all objects.
- **Synchronous Execution**: The JIT currently executes synchronously within the VM thread. Handles passed to the JIT are guaranteed to be valid for the duration of the native call.
- **No Direct Mutation**: The JIT cannot directly modify heap memory. All mutations must go through runtime helpers to ensure invariants (like string immutability or array bounds) are maintained.
- **No Free**: The JIT does not manage object lifetimes. It neither allocates nor deallocates memory directly.

## 4. Runtime Helpers (The Boundary)

Interaction with the heap is performed via static calls to "Runtime Helpers" provided by the VM.

### 4.1 Fallible Helper ABI: (Value, Status)
Helpers that can fail (e.g., due to bounds checks or type mismatches) follow a dual-return ABI:
- **Return 0**: The primary result value (often a `HeapHandle` or `Int64`).
- **Return 1**: A status code (`Int64`). `0` indicates success; non-zero indicates a specific error.

### 4.2 Initial Helper Set (Proposed)

| Helper | Signature | Description |
|---|---|---|
| `tmpr_jit_string_len` | `(HeapHandle) -> i64` | Returns the number of characters. |
| `tmpr_jit_array_get` | `(HeapHandle, i64) -> (HeapHandle, i64)` | Returns `(element_handle, status)`. |
| `tmpr_jit_struct_get` | `(HeapHandle, i64) -> (HeapHandle, i64)` | Returns `(field_handle, status)`. |

## 5. Error Handling

- **Soft Errors**: Bounds checks, type mismatches, and null-checks in helpers return a non-zero status code. The JIT code can then decide whether to trap or branch to a fallback path.
- **Hard Errors / Traps**: Cranelift traps are used only for:
    - Internal JIT compiler bugs.
    - Violations of impossible invariants.
    - Floating point exceptions (if applicable).
- **Deoptimization**: While not implemented in v0.1, the `(Value, Status)` ABI prepares the JIT for future deoptimization triggers when a helper returns a "complex" status requiring VM intervention.

## 6. Security & Stability

- **Sandboxing**: By using opaque handles and runtime helpers, we prevent the JIT from accidentally corrupting VM state or accessing unauthorized memory regions.
- **Validation**: Every helper validates the handles and arguments it receives.
- **Opt-in Boundary**: Functions requiring heap operations are rejected during the eligibility check until specific helpers are implemented and verified.

## 7. Phased Implementation

- **Phase 16.0**: Define the boundary, ABI, and diagnostics. [COMPLETE]
- **Phase 16.1**: Implement String length and basic handle passing. [COMPLETE]
- **Phase 16.2**: VM-owned JIT handle arena prototype. [COMPLETE]
- **Phase 16.3**: JIT Helper Probe: String.size + Status Handling. [COMPLETE]
- **Phase 16.4**: JIT Helper Probe: Array.len. [COMPLETE]
- **Phase 16.5**: JIT Helper Probe: Array.get Int64. [COMPLETE]
- **Phase 16.6**: Heap Helper Status Consumption Probe. [COMPLETE]
- **Phase 16.7**: Heap Helper Error Policy Design. [COMPLETE]
- **Phase 16.8**: JIT Heap Helper Wrapper API. [COMPLETE]
- **Phase 16.9**: JIT Helper Probe: Struct.get Int64. [COMPLETE]
- **Phase 16.10**: JIT Helper Probe: Enum.tag. [COMPLETE]
- **Phase 16.11**: JIT Helper Probe: Enum.unwrap Int64. [COMPLETE]
- **Phase 16.12**: JIT Heap Allocation Boundary Design. [COMPLETE]
    - **Allocation Boundary**: Formalized "context-owned" allocation model.
    - **Failure Policy**: Introduced `JIT_STATUS_ERROR_ALLOCATION_FAILED`.
    - **Invalid Handle**: Formally reserved `HeapHandle(0)` as invalid/null.
    - **Probe Implementation**: Added `tmpr_jit_alloc_string_empty` to validate return-path for new handles.
- **Phase 16.13**: JIT Helper Probe: String.concat. [COMPLETE]
    - **Heap-Producing Helper**: Implemented `tmpr_jit_string_concat`, returning a new context-owned handle.
    - **Value Semantics**: Confirmed original strings are never mutated.
    - **Unicode Safety**: Verified correct UTF-8 concatenation interacting correctly with `String.len` and `String.size`.
    - **Type Safety**: Verified `TypeError` when concatenating non-strings.
- **Phase 16.14**: JIT Helper Probe: Array.push Int64. [COMPLETE]
    - **Value Semantics**: Confirmed `tmpr_jit_array_push_i64` allocates a new array, leaving the original unchanged.
    - **Length Consistency**: Verified `Array.len` on old and new handles.
    - **Type Safety**: Verified `TypeError` on non-array handles.
- **Phase 16.15**: (Optional) JIT memory/arena bounds enforcement probe. [PLANNED]

## Heap Allocation Boundary (Phase 16.12)

To ensure memory safety and maintain the integrity of the VM garbage collector (or arena), the JIT compiler adheres to strict rules regarding heap allocation:

1. **No Direct Allocation**: JIT-compiled native code must *never* directly allocate heap memory. All object creation is delegated to the `JitRuntimeContext` via external C ABI helpers (e.g., `alloc_string`).
2. **Context Ownership**: Objects created during JIT execution are owned by the active `JitRuntimeContext`. Their lifetime is guaranteed for the duration of the JIT session (until `JitRuntimeContext::clear()` is called).
3. **Value Semantics (Handles)**: Allocation helpers return a `JitHelperResult`. On success, the `value` field contains a new `HeapHandle`. The JIT treats this handle as an opaque, immutable token.
4. **Invalid Handle**: `HeapHandle(0)` is explicitly reserved as the invalid or null handle representation.
5. **Allocation Failure**: If the runtime fails to allocate an object (e.g., Out Of Memory), the helper returns `JIT_STATUS_ERROR_ALLOCATION_FAILED`. The JIT must map this to `JitRuntimeError::AllocationFailed` and trap/fallback to the VM.
6. **No In-Place Mutation**: To preserve value semantics and simplify the boundary, operations like string concatenation or array appending return *new* handles rather than mutating existing ones.

- **Phase 17.1**: Integrate heap-aware JIT into VM runtime. [COMPLETE]
    - **Context Management**: Scoped `JitRuntimeContext` lifetime to JIT execution invocation.
    - **Type Marshalling**: Bi-directional type conversion (from VM values to JIT handles on entry, and back to VM values on exit).
    - **Fallback Execution**: Elegant fallback to the standard interpreter when JIT execution traps or returns error status.
- **Phase 17.2**: Heap-aware JIT Performance & Lifetime Validation. [COMPLETE]
    - **Heap Telemetry**: Extended `JitRuntimeContext` tracking and VM profiling stats.
    - **Performance Benchmarks**: Documented significant speedups (up to **2x speedup** on warm JIT reuse).
- **Phase 17.3**: Heap Handle Deduplication & Context Reuse. [COMPLETE]
    - **Deduplication**: Implemented `handle_cache` to deduplicate identical values passed into the JIT within the same context lifetime.
    - **Metrics**: Added `cache_hits` and `cache_misses` tracking.
- **Phase 17.4**: Heap-aware JIT Source-level Expansion. [COMPLETE]
    - **End-to-end Integration**: Connected `String.len`, `String.size`, `Array.len`, and `Array.get_i64` down to the source-level JIT execution pipeline.
    - **Differential Testing**: Implemented source-level differential test suites (`heap_jit_source_tests.rs`) to ensure parity between interpreter and JIT paths.
- **Phase 17.5**: Heap-aware JIT Generic Helper Intrinsics. [COMPLETE]
    - **Intrinsic Layer**: Introduced generic dispatcher intrinsics (`tmpr_jit_generic_len`, `tmpr_jit_generic_size`, `tmpr_jit_generic_get_i64`) to route JIT operations securely across distinct heap types at runtime.
    - **Source Level Integration**: Bound language-level generic bytecode (`Size`, `Len`, `GetIndex`) to these intrinsics instead of directly hardcoding type-specific helpers. Specialized helpers are retained purely as granular lower-level building blocks.
- **Phase 17.6**: Heap-aware JIT Rejection & Fallback Hardening. [COMPLETE]
    - **Named Diagnostics**: All eligibility rejection messages are now operation-specific (e.g. `"Array.push is not supported"`, `"Enum.tag is not supported"`) — no more phase-number-only messages.
    - **Fallback Safety**: Verified no panics on unsupported operations; VM interpreter fallback is transparent and correct.
    - **Profile Visibility**: `VmProfile::summary()` now always shows `permanent rejections:` when any function is rejected.
    - **Operation Status Table**: Added canonical operation-status table to `CRANELIFT_JIT.md`.
- **Phase 17.7**: Heap-aware JIT Docs, Examples & CLI Smoke. [COMPLETE]
    - **Examples**: Added `examples/jit_heap/` with 5 `.tmpr` files (`string_len`, `string_size`, `array_len`, `array_get`, `mixed_heap_int`). Each includes an `// expected: N` comment.
    - **Justfile commands**: `just jit-heap`, `just jit-heap-smoke`, `just jit-heap-perf`.
    - **CLI Smoke**: All 5 examples run via `tempra run --experimental-vm --experimental-jit` and exit with code 0.
    - **Documentation**: README Experimental JIT section expanded with subset table, build instructions, and `just` commands. `CRANELIFT_JIT.md` has a full *Running Heap-aware JIT Examples* section.
- **Phase 17.8**: Heap-aware JIT Phase Closure & Roadmap Gate. [COMPLETE]
    - **Milestone freezing**: Formally closed Phase 17 JIT slice. Created phase closure document `docs/JIT_PHASE_17_CLOSURE.md` and added Phase 18 Options Gate to the project roadmap.
    - **Verification automation**: Integrated the unified validation gate `just jit-phase17`.
    - **Docs consistency**: Guaranteed 100% consistent references to the read-only JIT subset (`String.len`, `String.size`, `Array.len`, `Array.get_i64`).

## Integrated JIT Telemetry & Lifetime Scoping (Phase 17.2)

### 1. Lifetime Isolation & Handle Garbage Collection
To prevent memory leaks and handle identifier collision, a new `JitRuntimeContext` is initialized dynamically at each JIT invocation boundary:
```rust
let mut ctx = JitRuntimeContext::new();
```
All intermediate values allocated by C helpers live within this context. When execution completes:
- The context goes out of scope.
- All allocated handles and underlying Rust data are automatically dropped.
- The VM memory stays pristine with zero long-lived resource leaks.

### 2. High-Fidelity JIT Profiling
The interpreter's profile builder tracks the precise execution behavior of each JIT-compiled function. It captures real-time helper density and JIT error metrics, letting VM performance tuning tools inspect:
- **Helper Call Density**: Number of helper calls compared to JIT invocations.
- **Deoptimization Traps**: Fallback count indicating where fallback paths are executed.
- **Compile Cost vs Runtime Gain**: Profitability ratios.
