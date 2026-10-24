# Tempra TIR Lowering (Phase 12.2 Hardening)

This document describes the compilation strategy from Tempra Typed HIR to TIR.

## Status: Phase 14.4 — VM Coverage Expansion v2
We have expanded the TIR lowerer to handle Result propagation and basic ADT operations for the experimental VM.


### Major Changes in 12.2:
- **Strict Return Checks**: Functions now require an explicit `return` unless they return `Unit`. Implicit `ret` is only emitted at the end of `Unit` functions.
- **Assignment Validation**: The lowerer now tracks mutability of local variables. Assigning to a `let` binding or a non-local target results in a `TirLowerError`.
- **Parameter Lowering**: Function parameters are automatically lowered to local slots in the entry block. They are immutable by default.
- **Function Calls**: Support for simple calls to known functions. The lowerer verifies arity against the `TypeContext`.
- **CFG Improvements**: The `If` statement lowering is more intelligent about branch termination. If both branches terminate, no join block is created, avoiding dead code generation.
- **Builder Hardening**: The TIR builder now returns `Result` for all operations. Attempts to append instructions to an already-terminated block result in an explicit error.
- **Result Propagation (`?`)**: Support for desugaring `expr?` into explicit control flow using `tag`, `eq`, `condbr`, `unwrap`, and early `ret`.
- **ADT Operations**: Basic support for mapping Result/Option types and operations (`make`, `tag`, `unwrap`) to TIR.


## Supported Subset

### Types
- `Unit`, `Bool`, `Int64`, `UInt64`, `Float64`, `String`, `Never`.
- `Result(T, E)`, `Option(T)`.


### Expressions
- **Literals**: Integer, Float64, Bool, String.
- **Arithmetic**: `+`, `-`, `*`, `/`, including unary `-`. Integer arithmetic is
  checked; Float64 follows IEEE 754, and unary Float64 negation lowers to
  multiplication by `-1.0` so signed zero and infinity are preserved.
- **Comparisons**: `==`, `!=`, `<`, `>`, `<=`, `>=`.
- **Logic**: `and`, `or`, `not`.
- **Variables**: Load from local slots.
- **Calls**: Direct calls to functions declared in `TypeContext`.
- **Propagation**: `expr?` for Result types, desugared into explicit blocks.


### Statements
- **Let/Var**: Local bindings with mutability tracking.
- **Assignment**: Updating `var` slots.
- **If**: Branching logic with strict termination rules.
- **Return**: Explicit return of values.

## Non-Goals for Phase 12.2
- **Generic Inference**: All types must be resolvable to concrete TIR types.
- **Interfaces/Traits**: Dynamic dispatch and interface coercions are deferred.
- **Struct/Enum Layouts**: Member access and constructor mappings are minimal.
- **Closures**: Capture and closure lowering is not yet implemented.
- **If-as-expression**: Joining values from branches via phi-nodes or similar is deferred.

## Missing-Return Policy
The lowerer ensures that every control flow path in a non-Unit function ends in a `Ret` or `Unreachable` (if logically dead). If the end of the function body is reachable without a return, a `MissingReturn` error is raised.

## Param Lowering Policy
Parameters are treated as local slots initialized in the entry block. This ensures that they can be used interchangeably with local variables (though they are currently immutable).

## Call Policy
Functions must be present in the `TypeContext` to be callable. The lowerer performs a simple arity check and declares the function in the `ModuleBuilder` to obtain a stable `TirFunctionId`.

---

## Phase 12.3 — `emit-tir` CLI (Experimental)

### Usage
```bash
tempra emit-tir <file.tmpr>
```

This command parses, type-checks, lowers to TIR, verifies the result, and prints a human-readable representation to stdout. It does **not** execute the program.

### Pipeline
1. `parse_source` → AST
2. `lower_file` → HIR
3. `check_file_with_context` → TypeContext
4. `lower_function` (per function) → TIR instructions
5. `verify_module` → structural validation
6. `format_tir_module` → pretty-printed output

### Error Behavior
- **Parse/Type errors** are reported in the existing format.
- **Unsupported constructs** produce: `emit-tir unsupported in fn '<name>': <detail>`
- **Internal verifier failures** produce: `internal TIR verifier error: <detail>`
- The command **never panics** on user input.

### Non-Goals
- `emit-tir` does not affect `tempra run`, `tempra check`, or `tempra test`.
- TIR emission is not required for any existing workflow.
- This is an experimental developer tool for inspecting the lowering output.

---

## Phase 14.7 — VM Coverage Expansion v5 (for-in)

### 1. `for-in` on Arrays and Slices

In the experimental VM, `for-in` loops over `Array(T)` and `Slice(T)` are lowered to an explicit index-based loop. This avoids the overhead of the full `Iterator`/`Iterable` protocol while the VM pipeline is still in early development.

#### High-Level Pattern

```tempra
for x in xs {
    body
}
```

#### Lowered TIR Structure

1.  **Preparation**:
    - Evaluate `xs` and store it in a hidden slot `$iter_obj`.
    - Create a hidden slot `$idx` initialized to `0`.
    - Create a hidden slot `$len` initialized to `len($iter_obj)`.

2.  **CFG**:
    - **CondBlock**: Loads `idx` and `len`, performs `idx < len`, and branches to **BodyBlock** or **ExitBlock**.
    - **BodyBlock**:
        - `x = xs[idx]` (GetIndex).
        - Declares loop variable `x` as a local slot.
        - Executes loop body statements.
        - Increments `idx` (`idx = idx + 1`).
        - Branches back to **CondBlock**.
    - **ExitBlock**: Continues execution after the loop.

3.  **Implementation Details**:
    - **Hidden Slots**: The slots for `$iter_obj`, `$idx`, and `$len` are unique to each loop and are not accessible by user code.
    - **Loop Variable**: The loop variable `x` is declared as a local slot within the `BodyBlock`. Its scope is limited to the loop body.
    - **Bounds Checking**: Since TIR `GetIndex` performs runtime bounds checking, the loop is safe.
    - **Performance**: This direct lowering is efficient as it maps directly to basic arithmetic and jump instructions in the VM.
