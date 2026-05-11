# TIR Specification (v0.1)

## Overview
TIR (Tempra Intermediate Representation) is the core representation designed to be strongly typed, verifiable, readable, serializable, and backend-friendly. It serves as the bridge between Tempra's frontend (HIR) and various backend execution strategies.

**Current Stage**: Phase 12 - TIR Bootstrap.
The goal of this phase is to establish the minimal data model and verification logic for TIR without completely removing the existing HIR interpreter. 

## Non-Goals
- Complete virtual machine execution (currently).
- High-performance definitive bytecode.
- Cranelift or Wasm backends.
- Advanced optimization passes.
- JIT compilation.
- Removing the current HIR interpreter.

## Pipeline
`Source -> AST -> HIR -> Typeck -> TIR -> (Interpreter / VM / Compiler)`

Currently, HIR interpreter continues to be used. TIR generation will be incrementally integrated.

## TIR v0.1 Types
TIR supports a minimal subset of types in v0.1:
- `Unit`, `Bool`, `Never`
- Numeric: `Int64`, `UInt64`, `UInt8`, `Float64`
- Strings: `String`
- Composite: `Tuple`, `Array`, `Slice`, `Struct`, `Enum`, `Option`, `Result`
- Special: `Tag`, `Function`

## Instruction Set (v0.1 Core)
TIR uses explicit values with numeric identifiers (e.g., `%0`, `%1`).

### Values and Constants
- `const`: Creates a literal constant value.
- `make`: Allocates/creates a composite type (Struct, Enum, Tuple, Array).

### Mutability (Slot / Load / Store Model)
Variables in TIR are managed explicitly through memory slots:
- `slot`: Declares a local, typed storage slot.
- `load`: Reads a value from a slot.
- `store`: Writes a value to a slot.

### Operations
- `get` / `set`: Access elements (fields, constant indices) within objects.
- `get_index`: Access an element of an Array or Slice using a dynamic index.
- `slice`: Create a read-only Slice view from an Array or another Slice.
- `len`: Retrieve the length of an Array or Slice (element count) or String (scalar count).
- `size`: Retrieve the UTF-8 byte size of a String.
- `bytes`: Retrieve a read-only Slice view of a String's bytes as UInt8.
- `tag` / `unwrap`: Support for algebraic data type destructuring.
- `add`, `sub`, `mul`, `div`: Numeric arithmetic operations.
- `eq`, `lt`, `gt`: Numeric comparison, producing boolean results.
- `and`, `or`, `not`: Boolean logic operations.
- `call`: Invokes a TIR function.

### Terminators
Every block must end with a terminator:
- `br`: Unconditional branch to a block.
- `condbr`: Conditional branch based on a boolean value.
- `ret`: Returns a value or `Unit` from the function.
- `unreachable`: Marks code paths that should logically never execute.

## Verifier Invariants
The verifier ensures the structural and type soundness of a TIR module:
- Every function must have at least one block (the entry block).
- All branched-to block IDs must exist within the function.
- Condition branches (`condbr`) must act on `Bool` type.
- `ret` instructions must return a value compatible with the function's return type.
- Arithmetic operations are strictly typed.
- `load` and `store` must operate on declared slots, matching their type.
- `get`, `set`, `tag`, and `unwrap` have smoke validation but defer strict structural layout type checking until fully implemented.

## Future Roadmap
- **v0.1**: Core (Constants, Slots, Branching, Primitives). *(Current)*
- **v0.2**: Memory references (`ref`), async execution (`spawn`, `await`).
- **v0.3**: Channels, message passing (`channel`, `send`, `recv`, `select`).
- **v0.4**: Actor model invocations (`actor_call`).
