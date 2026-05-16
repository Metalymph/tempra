# JIT Heap Boundary Design

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
    - **Prototype Context**: Introduced `JitRuntimeContext` for handle-to-value resolution.
    - **Context-First ABI**: Verified `extern "C" fn(ctx, handle) -> (value, status)`.
    - **Safety Verified**: Safe failures for `null` context and invalid handles.
- **Phase 16.3**: Implement Array indexing and bounds-check helpers. [PLANNED]
- **Phase 16.3**: Implement Struct field access.
- **Phase 16.4**: Finalize Enum/Option support via Tag/Unwrap helpers.
