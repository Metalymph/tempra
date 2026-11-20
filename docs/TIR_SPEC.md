# Tempra Intermediate Representation (TIR) — Specification v0.2

**Status:** Consolidated technical specification. Existing TIR v0 capabilities are documented separately from the **approved but not implemented** Memory & Ownership v0.4 obligations. This is not a claim of completed ownership verification.

**Authorities:** [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) is the sole normative memory model; [Surface Syntax v0.2](TEMPRA_SURFACE_SPEC_v0.2.md) defines the language surface; [Capability Contracts](specs/effects-v0.1.md) and [Specialized Effects v0.2](specs/specialized-effects-v0.2.md) govern effects. This document governs TIR structural representation and verification within those constraints.

## 1. Purpose

TIR is Tempra's typed, explicit, verifiable intermediate representation. Its principal uses are:

- a stable semantic boundary between typed HIR and executable lowering;
- a carrier for typed operations, control flow and per-function analysis facts;
- input to the Tempra Script bytecode pipeline and future LLVM-based Tempra Native AOT;
- a readable debugging/inspection representation;
- a validation target for the limited TIR micro-interpreter.

TIR is **not** a second language-level memory model. Physical slots, stack values, references or VM-specific runtime handles cannot change the semantics of a Tempra value.

## 2. Compiler pipelines and status

```text
Source → lexer/parser → AST → HIR → typechecking/specialization
     → verified TIR → capability analysis/contract enforcement
     → bytecode lowering → bytecode verifier → Tempra Script stack VM

Future: verified TIR + ownership/effect summaries → LLVM AOT (Tempra Native)
```

In the remote `main` baseline, `tempra run` still defaults to the HIR interpreter, and the VM is opt-in. Promotion to VM-default is a **planned gate**, not an implemented fact. Likewise, the current K1 `comptime` kernel uses HIR execution, while VM-based compile-time evaluation remains a **planned parity-gated migration**. The target architecture includes **neither JIT nor Cranelift**; removal of historical implementation code is a separate task.

TIR's supported executable subset can be narrower than HIR. Reject an unsupported lowering explicitly; never silently claim execution parity or bypass effect checks.

## 3. Identity and determinism

- `TirFunctionId` is a compilation-local semantic identifier, **not** an index into `TirModule.functions` or the bytecode function vector.
- `TirBlockId` is scoped to its function.
- `TirValueId` names a typed computed value; `TirSlotId` names explicitly addressable local storage.
- Module emission and function-ID to bytecode-index mapping must be deterministic. Existing K2 work emits in canonical ID order and resolves calls through an explicit map.
- Function specialization may produce new concrete TIR functions. Never assume a generic template has executable concrete code before specialization.

Cross-process reproducibility under K2 is a bounded same-build, same-source/module-order, same-host/target contract, **not** a universal cross-machine promise.

## 4. Type vocabulary

The documented TIR v0 representation covers:

| Group | Types |
|---|---|
| Basic | `Unit`, `Bool`, `Never` |
| Numeric | `Int64`, `UInt64`, `UInt8`, `Float64` |
| Text | `String` |
| Aggregates | `Tuple`, `Array`, `Slice`, `Struct`, `Enum`, `Option`, `Result` |
| Additional | `Tag`, `Function` |

`Tag` is reserved/represented differently from existing ADT tag instructions: in the historical v0 TIR/bytecode/VM representation `tag` yields a `String`. Do not silently substitute the type `Tag`. The set of TIR-representable types does not by itself certify that every source-level operation is lowerable or executable.

`Range` is a **language-level owned descriptor** in Memory v0.4. Its final TIR physical representation is **not fixed here**. Likewise, resource nonduplicability, loan provenance and closure environment types need explicit verifiable contracts without presuming invented opcodes.

## 5. Core instruction families (existing v0)

TIR is an SSA-like value IR with mutable slots. These are the documented instruction *families*, not a claim about final binary layout.

| Family | Instruction | Meaning |
|---|---|---|
| Literals | `const` | Produce a constant value |
| Aggregate | `make` | Construct a typed value (array, tuple, struct, enum) |
| Slots | `slot`, `load`, `store` | Declare, read, or overwrite a local storage slot |
| Access | `get`, `set` | Field or statically indexed access/update |
| Indexed | `get_index` | Dynamically indexed Array/Slice access |
| Views | `slice` | Read-only borrowed view of Array/Slice |
| Sizing | `len`, `size`, `bytes` | Collection length, UTF-8 byte size, read-only byte view |
| ADT | `tag`, `unwrap` | Inspect and extract a variant payload |
| Numeric | `add`, `sub`, `mul`, `div` | Typed arithmetic |
| Comparisons | `eq`, `lt`, `gt` | Typed comparisons |
| Boolean | `and`, `or`, `not` | Boolean operators; short-circuit semantics must be preserved in source lowering |
| Invocation | `call` | Typed call to known target |

A `make` operation is **not automatically** a heap allocation. Effect classification must be based on the semantic operation and its admitted implementations, not syntax alone. Similarly `load` is not automatically a semantic copy and `store` is not automatically a legal ownership transfer.

### 5.1 Arithmetic and comparisons

The typed verifier exposes operand/result types to bytecode lowering. `Int64` and `UInt64` integer arithmetic must produce controlled checked failures on overflow/underflow/division by zero; `Int64::MIN / -1` is overflow. `Float64` follows IEEE 754, including NaN and signed zero. Ordered float comparisons return false on NaN. Unsupported/unresolved numeric combinations are errors before bytecode generation; **no silent conversion to `Int64`**.

`UInt8` exists as a TIR type but historic TIR arithmetic lowering is limited; do not infer complete source-level numeric parity.

### 5.2 Borrowed `slice` and `bytes`

`Slice(T)` is a non-owning read-only view with provenance. Writable element access requires an exclusive `inout Slice(T)` contract. `bytes` must not provide writable access to a String's UTF-8 storage. The existing physical view instruction does not, by itself, prove owner lifetime, non-escape or exclusivity. Those are obligations from Memory v0.4.

## 6. Basic blocks and terminators

Every completed block has exactly one terminating control-flow operation:

- `br(target)` — unconditional branch;
- `condbr(condition, true_block, false_block)` — Bool conditional branch;
- `ret(value?)` — return matching the function's signature;
- `unreachable` — logically impossible path; executing it is a controlled error.

A builder must reject appending instructions to a terminated block; the lowering must not manufacture joins for branches that all terminate. Returned non-`Unit` values must be present on all reachable returning paths; missing returns are reported, not synthesized.

### Schematic example

```text
fn choose(%0: Int64, %1: Int64) -> Int64 {
  entry:
    %2 = gt %0, %1
    condbr %2, left, right
  left:
    ret %0
  right:
    ret %1
}
```

This is illustrative TIR pseudotext, not a serialization grammar or literal pretty-printer output.

## 7. Existing structural and type verifier

The documented v0 verifier checks, where supported:

1. Functions contain a valid entry block and valid block references.
2. Terminators are well formed; `condbr` requires Bool.
3. `ret` matches the declared return type.
4. Slots exist; `load` and `store` agree on slot types.
5. Arithmetic and ordering are typed and restricted to supported operand kinds.
6. Struct-field inference unifies field types across corresponding nominal `make` constructs; contradictions are rejected (`StructFieldTypeConflict`). Internal `$`-prefixed iterator state was excluded from the legacy nominal field inference.
7. Unsupported or unresolved numeric types are rejected before bytecode lowering.

The older verifier described `get`, `set`, `tag` and `unwrap` as having smoke coverage rather than complete structural-layout checking. **Do not upgrade this historical assurance without active-branch tests.**

## 8. Required ownership-aware verifier (Memory v0.4; not complete)

The approved memory model requires a checkable semantic trace/facts for:

| Invariant | Rejection example |
|---|---|
| Copyability | Duplicate a nonduplicable resource via `load`, aggregate construction or call |
| Semantic `consume` | Read source after its ownership was transferred |
| Shared loan | Mutate owner through an incompatible path while `ref`/Slice loan lives |
| Exclusive loan | Two simultaneous `inout` or incompatible shared/exclusive loans |
| Provenance | Return, store or escape a borrowed temporary beyond its owner |
| CFG lifetime | Illegally keep a loan live across a join, loop or unwind path |
| Closure captures | Hidden capture or unsafe escaping `ref`/`inout` environment |
| Suspension | Live dynamic loan across `.await()` or into `spawn` |
| Destruction | Leak/double release/use-after-destroy on return, failure or cancellation |
| Aggregates | Non-escaping borrowed fields follow their owner's lifetime |
| Effects | Potentially allocating copy under `@deny(alloc)` |

Per Memory v0.4, these need normalized verifier-level facts, not merely a frontend-only promise. The *representation*—explicit IR instructions, typed operations, summaries, provenance or equivalent proof material—remains a design/implementation decision. No specific `move`, `borrow`, `drop` or `retain` opcode is declared implemented by this document.

Static/owned descriptors and owned values may cross suspension when their contracts allow; dynamic borrowed slices, `ref` and `inout` cannot remain live across `.await()` in v1. Actor handles remain **DEFERRED POST NO-CYCLE**.

## 9. Capability/effect analysis boundary

E1 is a pure scan over verified TIR. The documented pipeline constructs per-function direct capability origins, a call graph, transitive required sets and witnesses; E2 retains those immutable facts in fresh compilation; E3 checks source contracts before executable bytecode is accepted.

- `@pure` denies `io`, `process`, `spawn`, `nondeterminism`; alone it does not deny allocation or panic.
- `@deny(alloc)` must conservatively reject semantic operations that may allocate dynamically, including copies of owned dynamic buffers as specified by v0.4.
- Checked integer operations can conservatively require `Panic`.
- Only operations present in TIR can be analyzed by E1; HIR-only operations are explicit coverage gaps, not silently allowed operations.
- Source-derived specialized effects, including `fs`/`net` families and `io.read`/`io.write` categories, remain governed by the effects specifications.
- Raw TIR interpretation and mere TIR emission do **not** certify source capability contracts.

Direct `Alloc` must reflect the semantic/runtime allocation contract. A syntactic `const` or `make` alone is not automatically an allocation origin, and an optional optimizer cannot justify accepting source that violates `@deny(alloc)`.

### 9.1 Summaries across modules

The current analysis can retain `TirFunctionId`, block and instruction identities as structural provenance. A reliable mapping to original source spans belongs to a lowering-owned side table. Future ownership analyses extend deterministic `BorrowSummary`-style facts with copyability, escape, capture, suspension, cleanup and effects; generic specialization must preserve these facts.

## 10. Result and control-flow conventions

`Result` propagation with `?` is lowered to explicit branching over a discriminant, error extraction and early return. `Result` is a value type, **not** a hidden exception/unwind mechanism. The lowering is required to preserve correct cleanup and resource ownership on the early-error path when v0.4 is implemented.

`if`/`match` expressions and loops may produce values; joining values must follow the language type rules, independent of whether the physical TIR uses slots, block parameters or another future encoding.

## 11. Cache trust, revisions and reproducibility

Bytecode cache trust revision and bytecode format version are independent. The historical milestones documented revisions 2 (K1), 3–4 (K2), 5 (K3 checked arithmetic), 6–7 (K4 specialized effects) and 8 (K3 numeric parity). **Revision 8 was the remote-main baseline**, not a guarantee of current local branch state. Never fabricate a newer revision number. Adding executable ownership guarantees or changing comptime authority requires correct trust invalidation; compatible bytecode-only cache hits do not reconstruct TIR or analyzer facts.

## 12. Runtime consumers and non-goals

- **Tempra Script VM:** intended default scripting runtime after parity. [BYTECODE_ISA.md](BYTECODE_ISA.md), [TIR_TO_BYTECODE.md](TIR_TO_BYTECODE.md) and [BYTECODE_VM.md](BYTECODE_VM.md) specify the runtime-side contract.
- **TIR micro-interpreter:** a limited validation tool, documented in [TIR_INTERP.md](TIR_INTERP.md); not a general scripting VM, nor the approved final comptime engine.
- **Tempra Native:** future LLVM AOT; no new user-visible memory model. Its RC-free aim is experimental until proven.
- **JIT/Cranelift:** historical; excluded from target architecture, implementation removal pending.
- **Actors/concurrency:** no TIR opcodes or actor scheduling policy is frozen here; safety gate M5 applies.
- **FFI/`@unsafe`:** wrapper contracts and external-resource policy follow v0.4; concrete raw ABI and unsafe surface are deferred.

## 13. Validation work before declaring full compliance

Required tests include bad TIR rejection; CFG joins/loops; deterministic function identities; arithmetic type correctness; error and early-return cleanup; copy and loan exclusivity; nonduplicable resources; borrowed aggregates; escape analysis; closure environments; async suspension; effect classification; stale-cache handling; VM/AOT agreement for the supported subset. A passing structural TIR verifier does **not** prove these v0.4 properties until they are implemented and tested.

**Rule:** TIR is a verified carrier of Tempra semantics, never an alternative source of them.
