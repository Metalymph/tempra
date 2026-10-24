# TIR to Bytecode Lowering

This document describes the strategy used to translate Tempra Intermediate Representation (TIR) into stack-based bytecode.

## Lowering Strategy

The current lowering strategy prioritizes **correctness and simplicity** over performance. It follows a "pragmatic" approach suitable for the initial bootstrap phases.

### 1. Persistent Value Materialization (SSA to Locals)
TIR is an SSA-like representation where values are explicitly named (e.g., `%0`, `%1`). To avoid complex stack scheduling or register allocation, every `TirValueId` is mapped to a dedicated bytecode **local variable**.

- **Production**: When an instruction produces a result (e.g., `%2 = add %0 %1`), the lowerer emits the instruction and immediately stores the result into its corresponding local slot (`StoreLocal tmp2`).
- **Consumption**: When a value is used as an operand, it is loaded from its local slot (`LoadLocal tmpX`).

This approach ensures that values are always available, regardless of stack depth or use order, at the cost of many `StoreLocal`/`LoadLocal` pairs.

### 2. Stack Machine Mapping
TIR instructions map directly to their bytecode counterparts:
- `Const` -> `ConstI64`, `ConstU64`, `ConstF64`, `ConstBool`, etc.
- `Add`, `Sub`, `Mul`, `Div` -> the opcode matching the verified numeric type:
  `AddI64`/`AddU64`/`AddF64`, and likewise for the other arithmetic operations.
- `Eq` -> `Eq` for comparable values.
- `Lt`, `Gt` -> `LtI64`/`GtI64`, `LtU64`/`GtU64` or `LtF64`/`GtF64` according
  to the verified operand type.
- `Call` -> `Call`.

### Numeric opcode selection

Lowering consumes `verify_module_with_types`; verified TIR types are
authoritative. This prevents `UInt64` or `Float64` arithmetic from being emitted
with `Int64` opcodes when source literals or inferred types do not carry enough
information. The verifier rejects arithmetic with unsupported or unresolved
numeric types before any bytecode is emitted. `UInt8` arithmetic is not expanded
in TIR lowering; it is rejected rather than silently reinterpreted as `Int64`.
The bytecode format and generic `Eq` opcode are unchanged.

`UInt64` has no reachable source construction path today: integer literals are
typed as `Int64` and there is no implemented numeric conversion constructor.
Its checked contract is therefore validated at the TIR, bytecode and VM
boundaries rather than claimed as source-level parity. `Float64` literals and
unary negation are reachable from source.

### 3. Control Flow & Jump Patching
Lowering is performed block-by-block. 
- **Labels**: Each `TirBlock` corresponds to a starting offset in the bytecode.
- **Relocation**: Branch instructions (`Br`, `CondBr`) are emitted with placeholder offsets.
- **Fixup**: After all blocks are lowered, the lowerer performs a second pass to patch the placeholder offsets with the actual bytecode addresses of the target blocks.

### 4. Memory Slots
`TirSlotId` (explicit memory slots used for mutable variables) are mapped to a separate range of bytecode locals, distinct from the temporary locals used for SSA values.

## Unsupported Instructions (v0)
The following TIR instructions are NOT supported in v0 and will cause a `BytecodeLowerError`:
- `Make`, `Get`, `Set` (Struct/Tuple operations).
- `Tag`, `Unwrap` (Enum operations).

## Future Optimizations (Phase 13.6+)
The current lowering generates redundant bytecode (e.g., `StoreLocal X` followed immediately by `LoadLocal X`). These will be optimized away in future phases using a **peephole optimizer** or a more sophisticated stack-tracking lowerer.
