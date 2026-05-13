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
- **Function Calls (v1)**: 
    - Static `Call` to other JIT-eligible Int64 functions.
    - Automatic pre-compilation of dependencies.
- **Superinstructions**:
    - `AddI64LL` (Fused local-local add).
    - `AddI64LC` (Fused local-constant add).

### Unsupported Features (Fallback to VM)
Any function using the following features is currently rejected by the JIT and will execute via the standard stack interpreter:
- Strings, Arrays, Slices.
- Structs, Enums.
- Option, Result.
- Recursion (recursive function calls are currently rejected to prevent compilation loops).
- Closures and methods.
- Floating point or unsigned arithmetic.

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
