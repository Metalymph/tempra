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
`Source -> AST -> HIR -> Typeck -> TIR -> TIR verification -> analyses -> VM / Compiler`

The HIR interpreter remains the default runtime. TIR generation is used by experimental VM and inspection paths and supports a smaller language subset.

E2 inserts capability analysis after TIR verification in fresh VM compilations,
retaining the TIR and facts. E1 remains a pure TIR analyzer. TIR emission and
interpretation verify without capability analysis; cache hits and HIR paths do
not construct an E2 analysis product.

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
- `tag` / `unwrap`: Support for algebraic data type destructuring. `tag` produces
  `String` in the current TIR/bytecode/VM representation, not the reserved `Tag`
  type. Comparisons require the same operand type, including tag/string comparisons.
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

## Analysis Roadmap

TIR evolution follows the project-wide dependency order rather than assigning concurrency instructions to the next TIR version:

1. **E — Capability analysis:** direct operation classification, call graph propagation, stable per-function summaries, and deterministic provenance.
2. **R — Result / value-use hardening:** future obligations against silent Result discard, not a capability/effect.
3. **M — Acyclic ownership:** formalize legal strong-reference graphs and complete ARC-cycle prevention before concurrent transfer.
4. **C — Pluggable actor concurrency:** add task, actor, channel, and selection operations only after the preceding foundations define their boundaries.
5. **V — Verified functions:** consume prior facts for stronger proof obligations.
6. **O — Deeper memory optimizations:** uniqueness, escape, stack promotion, reuse, and retain/release elimination.

E1–E4 are implemented for the currently meaningful surface. E6 diagnostics and
inspect/query tooling is next active and reuses current facts; it is not a
complete TIR Graph. U — Unsafe Boundary is standalone DEFERRED / CONDITIONAL
work listed after O, activated only by a real source operation requiring
memory/type-safety authorization. No current TIR capability origin emits Unsafe.

This section does not define future TIR opcodes. Their exact representation belongs to the corresponding phase specifications.

## Capability-analysis contract

E1 scans `TirInstr::Call` to build a function call graph. E2 retains structural
provenance (`TirFunctionId`, function-scoped `TirBlockId`, instruction index),
edges, origins and witnesses. It adds no source mapping: TIR does not retain a
reliable one. A future lowering side table maps locations to module identity and
existing source spans.

Direct `Alloc` classification is based on semantic runtime allocation in the current representation; `const` and `make` are not normative allocation sites by syntax alone. Direct `Panic` classification is conservative may-panic information. Later verified analysis may prove individual divisions or bounds accesses safe without changing E1's collection policy.

An extern requires a known capability classification only when a TIR call targets it. Unused extern declarations do not invalidate a module.
