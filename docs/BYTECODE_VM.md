# Bytecode VM (v0)

This document describes the design and execution model of the Tempra Stack-based Bytecode Virtual Machine.

## Overview

The Bytecode VM (v0) is a minimal, stack-based interpreter designed to execute verified Tempra bytecode. It serves as the initial execution engine for the bytecode pipeline and a foundation for future performance optimizations.

## Execution Model

### Frames

Execution is organized into a call stack of **Frames**. Each frame represents an active function call and contains:

- **Function ID**: Reference to the bytecode function being executed.
- **Instruction Pointer (IP)**: Offset of the next instruction to execute.
- **Locals**: A fixed-size array of `Option<VmValue>`. 
    - Parameters are initialized to `Some(value)`.
    - Other local slots (slots, temporary variables) are initialized to `None`.
    - This provides a safety layer against using uninitialized data, complementing the static verification.
- **Operand Stack**: A stack of `VmValue` used for intermediate computations.

### Calling Convention

1. **Argument Passing**: The caller pushes arguments onto its operand stack from left to right.
2. **Call Dispatch**: The `Call` instruction pops arguments from the caller's stack and initializes the callee's local slots (0 to N-1).
3. **Return Value**:
    - **Unit functions**: `Return` pops nothing. The caller stack remains unchanged.
    - **Non-Unit functions**: `Return` pops exactly one value from the callee's stack and pushes it onto the caller's stack.
    - **Top-level**: The final return value of the entry function is returned to the VM host.

## Safety & Verification

### Verifier-First Rule

The VM **MUST** call `verify_module` before executing any function. 
- Execution of unverified bytecode is disallowed to prevent crashes and undefined behavior.
- The interpreter assumes that the bytecode has been validated for:
    - Stack depth (MaxStack).
    - Stack type consistency.
    - Local initialization (init-before-load).
    - Jump target validity.

## Value Model

The VM uses the `VmValue` enum for all runtime values:

- `Unit`: Represents the `()` type.
- `Bool(bool)`: Boolean values.
- `Int64(i64)`: 64-bit signed integers.
- `UInt64(u64)`: 64-bit unsigned integers.
- `Float64(f64)`: 64-bit floating point numbers.
- `String(String)`: UTF-8 strings.
- `Enum(String, String, Vec<VmValue>)`: Enum variants, represented as `(EnumName, VariantName, Payloads)`. Used for `Option` and `Result`.
- `Array(Rc<Vec<VmValue>>)`: Reference-counted collection of values.
- `Slice { parent, start, end }`: A read-only view into a portion of an array or another slice.
- `Struct { name, fields }`: A named struct with field-value pairs.
- `Any`: An internal "escape hatch" type used for complex types (Enums, Structs, etc.) during verification.

## Error Handling

The VM implements strictly controlled error handling via `VmError`. It never panics on valid bytecode. 
Errors include:
- `DivisionByZero`: Integer division by zero.
- `StackUnderflow`: Attempt to pop from an empty stack (should be caught by verifier).
- `TypeMismatch`: Runtime type error (should be caught by verifier).
- `UnreachableExecuted`: Execution reached an `Unreachable` instruction.

## Experimental Integration

The VM is currently integrated into `tempra run` as an **opt-in experimental feature**. You can test it using the `--experimental-vm` flag:

```bash
tempra run --experimental-vm file.tmpr
```

The experimental VM pipeline works as follows:
1. It computes a cache key (hash) of the source file.
2. It attempts to load a pre-compiled bytecode artifact from `.tempra/cache/`.
3. If successful and compatible, it decodes and verifies the bytecode, then executes the `main` function.
4. On a cache miss or corruption, it lowers the code from source all the way to bytecode, verifies it, writes a new cache artifact, and then executes it.

**Note:** The fallback policy is strict. If the code uses language features not yet supported by the Bytecode Lowering phase (e.g., loops, classes, standard library), the execution will fail cleanly. It will **not** silently fall back to the HIR interpreter.

### Supported Language Features (v0.x)

- **Basic Arithmetic & Logic**: `+`, `-`, `*`, `/`, `==`, `!=`, `<`, `>`, `&&`, `||`, `!`.
- **Variables**: `let`, `var`, `assign`.
- **Functions**: Calls, parameters, returns.
- **Control Flow**: `if`/`else`.
- **Enums (Option/Result)**:
    - Construction: `Some(x)`, `None`, `Ok(x)`, `Err(e)`.
    - Propagation: The `?` operator for `Result` types is supported via explicit desugaring in TIR.
- **Arrays & Slices**:
    - Creation: Array literals `[1, 2, 3]`.
    - Indexing: `xs[i]` for both arrays and slices.
    - Slicing: `xs[a..b]` creates a view without copying data.
    - Length: `xs.len()` or `xs.len` retrieves the element count.
    - Iteration: `for x in xs` is supported via index-based lowering.
- **Match Expressions**: Sequential arm checking for literals, wildcards, and variants.
- **Bytecode Cache**: Persistent storage in `.tempra/cache/`.
- **Peephole Optimization**: Conservative post-pass to eliminate redundant `StoreLocal/LoadLocal` pairs for intermediate temporary values.

## Bytecode Lowering & Optimizations

The TIR-to-Bytecode lowering phase transforms the high-level Typed IR into a sequence of stack operations. To improve execution speed and reduce binary size, the lowerer includes a **Peephole Optimizer**:

1. **Pattern Matching**: The optimizer scans for adjacent instruction sequences like `StoreLocal(tmp) -> LoadLocal(tmp)`.
2. **Temp Local Analysis**: It only optimizes "temporary" locals (those generated for intermediate computations). User-defined variables and parameters are never removed to preserve debugging fidelity and program semantics.
3. **Control Flow Integrity**: The optimizer identifies jump targets and only applies transformations within safe instruction ranges. It automatically fixes up all jump offsets when instructions are removed.
4. **Safety Verification**: Every optimized function is re-verified by the Bytecode Verifier. If verification fails for any reason (e.g., stack depth mismatch), the VM automatically falls back to the unoptimized version of that function.


## Non-Goals (v0)

- **Performance**: v0 is a naive interpreter. Optimization (JIT, Register VM, etc.) is deferred.
- **Default Runtime**: The default `tempra run` still uses the HIR interpreter. The VM remains strictly opt-in.
