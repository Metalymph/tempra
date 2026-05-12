# Cranelift JIT Prototype

This document describes the Phase 15.0 prototype of a Just-In-Time (JIT) compiler for the Tempra VM using [Cranelift](https://cranelift.dev/).

## Objective

The goal of this prototype is to demonstrate the ability to compile hot bytecode functions into native machine code to reduce execution overhead. The prototype focuses on a minimal subset of the language to establish the end-to-end pipeline:
1. Identify JIT-candidate functions.
2. Lower Tempra bytecode to Cranelift IR.
3. Compile and finalize native code.
4. Execute native code via a safe wrapper.

## Supported Scope (v0.1)

The prototype currently supports only `Int64` pure functions:
- **Types**: `Int64` parameters, return values, and locals.
- **Arithmetic**: `Add`, `Sub`, `Mul`, `Div` (signed).
- **Comparisons**: `Eq`, `Lt`, `Gt`.
- **Control Flow**: 
    - Conditional branches (`JumpIfFalse`).
    - Unconditional jumps (`Jump`).
    - Recursive and iterative loops.
- **Superinstructions**:
    - `AddI64LL` (Fused local-local add).
    - `AddI64LC` (Fused local-constant add).

### Unsupported Features (Fallback to VM)
Any function using the following features is currently rejected by the JIT and will execute via the standard stack interpreter:
- Strings, Arrays, Slices.
- Structs, Enums.
- Option, Result.
- Function calls (static or dynamic).
- Closures and methods.
- Floating point or unsigned arithmetic.

## Implementation Details

### Stack-to-SSA Lowering
Tempra's stack-based bytecode is lowered to Cranelift's SSA-based IR. The lowering pass simulates the operand stack during translation using a `Vec<Value>`. Local variables are mapped to Cranelift `Variable`s, allowing the Cranelift frontend to handle register allocation and SSA construction automatically.

### Block Management
Jump targets are pre-scanned to create Cranelift `Block`s. To ensure valid SSA, blocks are sealed only after all predecessors are identified.

### Architecture Support
The JIT is configured to support both `x86_64` and `AArch64` (Apple Silicon). On AArch64, the ISA is specifically tuned to avoid PLT requirements for simple JIT usage.

## Future Integration

In future phases, the JIT will be integrated directly into the `Interpreter` loop:
- The VM will monitor function "hotness" via `VmProfile`.
- Once a threshold is reached, the JIT will compile the function in the background.
- Subsequent calls to the function will swap the execution path from `Interpreter::execute` to native code.

## Verification

The JIT is verified via differential tests comparing VM results with native execution:
```bash
cargo test -p tempra-jit-cranelift
```
