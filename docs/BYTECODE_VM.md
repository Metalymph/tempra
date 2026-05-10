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

## Error Handling

The VM implements strictly controlled error handling via `VmError`. It never panics on valid bytecode. 
Errors include:
- `DivisionByZero`: Integer division by zero.
- `StackUnderflow`: Attempt to pop from an empty stack (should be caught by verifier).
- `TypeMismatch`: Runtime type error (should be caught by verifier).
- `UnreachableExecuted`: Execution reached an `Unreachable` instruction.

## Non-Goals (v0)

- **Performance**: v0 is a naive interpreter. Optimization (JIT, Register VM, etc.) is deferred.
- **Public Runtime**: The VM is currently internal and used for testing. `tempra run` still uses the HIR interpreter.
- **Cache**: Bytecode is generated and verified on the fly. Persistent cache is planned for a future phase.
