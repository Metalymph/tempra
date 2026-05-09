# Tempra TIR Interpreter — Phase 12.5

## Status: Experimental Micro-Step Interpreter (Hardened)

The `tempra-tir-interp` crate provides a minimal interpreter for TIR modules.
It is **not** the runtime for `tempra run` and is used exclusively for
semantic validation and testing of the TIR v0.1 core instruction set.

## Design

The interpreter uses a **frame-based micro-step** execution model:

- **Value table**: `TirValueId → TirRuntimeValue` — stores SSA values produced by instructions.
- **Slot table**: `TirSlotId → TirRuntimeValue` — stores mutable local variables.
- **Block cursor**: Executes instructions sequentially within a block, then follows the terminator to the next block (or returns).

### Entry Point

```rust
interpret_function(
    module: &TirModule,
    function_name: &str,
    args: Vec<TirRuntimeValue>,
) -> Result<TirRuntimeValue, TirInterpError>
```

The module is verified (`verify_module`) before execution. If verification
fails, `TirInterpError::VerifierFailed` is returned immediately.

## Phase 12.5 — Internal Pipeline Hook

A new internal hook has been added to the `tempra-driver` to allow end-to-end testing of the TIR pipeline:

```rust
interpret_tir_source(
    source: &str,
    function_name: &str,
    args: Vec<TirRuntimeValue>,
) -> Result<TirRuntimeValue, DriverError>
```

This hook is used exclusively for regression testing of the compiler frontend and the TIR lowerer. It ensures that real Tempra code (within the supported TIR subset) can be successfully compiled and executed.

## Supported Instructions

| Instruction | Status |
|---|---|
| `const` | ✅ All primitive types |
| `slot` / `load` / `store` | ✅ |
| `add` / `sub` / `mul` / `div` | ✅ Int64, UInt64, Float64 |
| `eq` / `lt` / `gt` | ✅ Primitive types |
| `and` / `or` / `not` | ✅ Bool |
| `call` | ✅ Intra-module calls |
| `ret` | ✅ With/without value |
| `br` / `condbr` | ✅ |
| `unreachable` | ✅ Returns error |
| `make` / `get` / `set` | ❌ `UnsupportedInstr` |
| `tag` / `unwrap` | ❌ `UnsupportedInstr` |

## Runtime Values

```
TirRuntimeValue::Unit
TirRuntimeValue::Bool(bool)
TirRuntimeValue::Int64(i64)
TirRuntimeValue::UInt64(u64)
TirRuntimeValue::Float64(f64)
TirRuntimeValue::String(String)
```

## Error Handling

All errors are represented as `TirInterpError`. The interpreter never panics
on user input. Notable error cases:

- **DivisionByZero** — checked before executing `div`
- **UnreachableExecuted** — deterministic error on `unreachable` terminator
- **UnsupportedInstr** — clean error for `make`/`get`/`set`/`tag`/`unwrap`
- **WrongArity** / **UnknownFunction** — checked at call sites

## Differences from Future Bytecode VM

| Aspect | TIR Interpreter (current) | Bytecode VM (future) |
|---|---|---|
| Input | TIR Module (tree) | Flat bytecode |
| Dispatch | Match on enum variant | Opcode table |
| Performance | Not optimized | Optimized dispatch loop |
| Scope | Validation only | Production runtime |
| Heap/GC | None | Required |
| Stdlib | None | Full |

## Non-Goals

- This interpreter does **not** replace the HIR interpreter (`tempra run`).
- No stdlib, no I/O, no heap allocation, no closures.
- Not exposed as a user-facing command (internal/test only).
