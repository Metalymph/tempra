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

TIR evolution follows the [roadmap](roadmap.md)'s layered dependency model
rather than assigning concurrency instructions to the next TIR version. The
roadmap is authoritative for sequencing, statuses, dependencies and gates.

- **Semantic:** E (implemented for the current surface) → R (Result/value-use
  hardening, not a capability/effect) → M (acyclic ownership and ARC-cycle
  prevention; M5 stabilization gate) → C (pluggable actor concurrency: task,
  actor, channel and selection) → V (`@verified`, consuming prior facts). Actor
  concurrency implementation requires M5.
- **Native AOT:** First-class milestone A, with Cranelift as the initial backend.
  It may progress incrementally and does not wait for O, NR or P.
- **Optimization:** O covers uniqueness, escape, stack promotion, reuse and
  retain/release elimination after M ownership facts; P covers deep latency,
  invalidation, specialization control and data-structure work after K5/K6
  measurements. O is not a prerequisite for C or A.
- **Research:** NR is standalone experimental RC-free research with fallback ARC
  and never blocks baseline AOT.
- **Conditional:** U — Unsafe Boundary is DEFERRED / CONDITIONAL and activates
  only when a real source operation requires memory/type-safety authorization.

E1–E6 are implemented for the currently meaningful surface; K3 runtime parity is
IN PROGRESS for currently executable surfaces, K4 is DONE, and K5/K6 are
PLANNED. E6 reuses current facts and is not a complete TIR Graph. No current TIR
capability origin emits Unsafe.

K1.7 historically changes cache trust to `semantics-2`. K2.2 changes current
trust to `semantics-3` so artifacts that may contain the former specialization
ID/index miscompilation are not reused. K2.4 changes current trust to
`semantics-4` so interface candidate selection and diagnostics are not reused
from unordered iteration. The bytecode format remains an independent
structural version, and neither TIR nor capability facts are rebuilt on a
compatible bytecode-only cache hit. `TirFunctionId` is a compilation-local
identity, not a physical position in `TirModule.functions`; bytecode lowering
emits functions by ascending ID and resolves calls through an explicit mapping.
K2.5 keeps revision 4 and proves exact cross-process artifact equality for the
currently supported same-build, same-host/target bytecode surface; no Level 4
cross-machine guarantee is implied.

K3.2 raises cache trust to `semantics-5` without changing TIR or bytecode
formats. For `Int64`, `add`, `sub` and `mul` are checked, `div` distinguishes
zero from signed overflow, and lowered negation inherits checked `sub` behavior.
All supported runtime consumers use the shared contract; host panics, native
traps and profile-dependent wrapping are not semantic outcomes.

K4.5B historically raised cache trust to `semantics-6` without changing TIR or
bytecode formats. Revision-5 artifacts can retain specialized `@deny` clauses
that were not statically enforced. K4.9D subsequently raises current trust to
`semantics-7` for v0.2 family/category enforcement, and K4.9E confirms
compatibility without another semantic change. Current compilation matches the
retained clauses against the existing specialized-effect summaries and
witnesses. These revisions do not change TIR or bytecode formats.

This section does not define future TIR opcodes. Their exact representation belongs to the corresponding phase specifications.

## Capability-analysis contract

E1 scans `TirInstr::Call` to build a function call graph. E2 retains structural
provenance (`TirFunctionId`, function-scoped `TirBlockId`, instruction index),
edges, origins and witnesses. It adds no source mapping: TIR does not retain a
reliable one. A future lowering side table maps locations to module identity and
existing source spans.

Direct `Alloc` classification is based on semantic runtime allocation in the current representation; `const` and `make` are not normative allocation sites by syntax alone. Direct `Panic` classification is conservative may-fail information. Checked `Int64` add, sub, mul and div contribute `Panic`; later verified analysis may prove individual arithmetic, divisions or bounds accesses safe without changing E1's collection policy.

An extern requires a known capability classification only when a TIR call targets it. Unused extern declarations do not invalidate a module.
