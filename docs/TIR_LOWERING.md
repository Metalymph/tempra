# Tempra TIR Lowering (Phase 12.2 Hardening)

This document describes the compilation strategy from Tempra Typed HIR to TIR.

## Status: Phase 12.2 — Hardening
We have moved from a smoke-test implementation to a more robust, verified lowering process.

### Major Changes in 12.2:
- **Strict Return Checks**: Functions now require an explicit `return` unless they return `Unit`. Implicit `ret` is only emitted at the end of `Unit` functions.
- **Assignment Validation**: The lowerer now tracks mutability of local variables. Assigning to a `let` binding or a non-local target results in a `TirLowerError`.
- **Parameter Lowering**: Function parameters are automatically lowered to local slots in the entry block. They are immutable by default.
- **Function Calls**: Support for simple calls to known functions. The lowerer verifies arity against the `TypeContext`.
- **CFG Improvements**: The `If` statement lowering is more intelligent about branch termination. If both branches terminate, no join block is created, avoiding dead code generation.
- **Builder Hardening**: The TIR builder now returns `Result` for all operations. Attempts to append instructions to an already-terminated block result in an explicit error.

## Supported Subset

### Types
- `Unit`, `Bool`, `Int64`, `UInt64`, `Float64`, `String`, `Never`.

### Expressions
- **Literals**: Integer, Bool, String.
- **Arithmetic**: `+`, `-`, `*`, `/`, including unary `-`.
- **Comparisons**: `==`, `!=`, `<`, `>`, `<=`, `>=`.
- **Logic**: `and`, `or`, `not`.
- **Variables**: Load from local slots.
- **Calls**: Direct calls to functions declared in `TypeContext`.

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
