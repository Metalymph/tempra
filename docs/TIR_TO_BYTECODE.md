# Tempra — TIR to Bytecode Lowering

**Version:** v0.2 (documentation consolidation)
**Status:** Existing SSA-like TIR → stack-bytecode strategy, with future ownership-aware obligations explicitly identified.
**Authority:** [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) governs semantic correctness. This document is not evidence that all v0.4 rules are implemented.

## 1. Place in the pipeline

```text
Source → Typed HIR → TIR → TIR verification
       → Bytecode lowering → Bytecode verification → Tempra Script VM
```

The initial implementation prioritises a simple, auditable mapping instead of whole-program stack scheduling. A source feature may be accepted by the parser/type checker but unsupported by TIR or bytecode lowering. This must produce a controlled diagnostic; it must **never** silently fall back to the HIR interpreter after a semantic, capability or verifier failure.

The current remote `main` snapshot may still default to HIR execution. The planned VM-first promotion requires parity gates. No JIT or Cranelift backend belongs to the target execution pipeline.

## 2. Persistent values: SSA-like IDs → bytecode locals

Each `TirValueId` is assigned a bytecode temporary local:

1. **Produce:** emit instructions for the TIR producer, then `StoreLocal(temp)`.
2. **Use:** emit `LoadLocal(temp)` before the consuming operation.
3. **Return:** load the result local, then `Return` (or return directly when equivalent).
4. **Parameters:** initialized in the function entry; their identity is not assumed to match an unrelated function index.

This yields redundant loads and stores but makes scheduling and debugging straightforward. In the **legacy v0 physical design**, a load may be repeated. Under **Memory v0.4**, repetition is only valid if it preserves semantic value independence, resource ownership and borrowing constraints. Loading a nonduplicable resource twice is **not** a licence to create two owners.

`TirSlotId` for explicit mutable slots maps to a distinct local range from temporaries. Source immutability and `inout` exclusivity must be preserved through the mapping.

### Exclusive parameter places (A1 implementation)

For the ownership-aware VM, `BindInout(slot, parameter_index)` explicitly binds
an incoming non-slice exclusive parameter to a storage slot. The binding is
verified in the entry block and lowers to the corresponding parameter local,
not to `LoadLocal` followed by `StoreLocal`. Ordinary `Load` and `Store` operate
on the value at that place; only the explicit borrow instructions transfer an
exclusive handle. Hidden mutable closure captures use the same binding and
parameter access metadata. Public `inout` callable arguments remain unsupported.

### 2.1 Demonstration

```text
TIR:
    %0 = load $a
    %1 = load $b
    %2 = add.i64 %0, %1
    ret %2

Bytecode schematic:
    LoadLocal a
    StoreLocal tmp0
    LoadLocal b
    StoreLocal tmp1
    LoadLocal tmp0
    LoadLocal tmp1
    AddI64
    StoreLocal tmp2
    LoadLocal tmp2
    Return
```

This is illustrative. The actual lowerer can omit unnecessary temporaries after verified optimisation.

## 3. Typed instruction selection

The lowerer **consumes verified TIR type information**, not the apparent source spelling of a literal.

| TIR operation | Bytecode families |
|---|---|
| `Const` | `ConstUnit`, `ConstBool`, `ConstI64`, `ConstU64`, `ConstF64`, `ConstString` |
| `Add`, `Sub`, `Mul`, `Div` | Matching `I64`, `U64` or `F64` typed arithmetic |
| `Lt`, `Gt` | Matching `LtI64/GtI64`, `LtU64/GtU64` or `LtF64/GtF64` |
| `Eq` | `Eq`, only for permitted comparable types |
| `Call` | `Call` with verified target and arity |
| `Ret` | Return according to verified signature |
| `Br`, `CondBr` | `Jump`, `JumpIfFalse`, with patching |
| Enum/tuple/array/string operations | Only implemented opcode families verified for their type and ownership semantics |

The original v0 lowerer used `verify_module_with_types` to prevent unsigned or float operations from being emitted as signed `Int64`. This remains mandatory. Unsupported or unresolved numeric types are rejected rather than coerced.

The historical snapshot reports that `UInt8` arithmetic is not expanded by TIR lowering. `UInt64` bytecode/VM arithmetic has dedicated checked semantics, although source-level reachability may be narrower. Reachability must be verified against the active branch before a release claim.

**Arithmetic guarantees:** checked `Int64`/`UInt64` operations report controlled failures; `Float64` preserves IEEE 754, including signed zero and NaN. Numeric type selection is not an optimisation hint.

## 4. Control-flow blocks and relocation

Lowering proceeds block-by-block:

1. Assign each TIR block a symbolic label.
2. Emit instructions for the block, recording its start offset.
3. Emit branches with relocation placeholders.
4. After code emission, patch branch operands to verified target instruction boundaries.
5. Verify stack depth, type consistency, reachability and valid terminators.

The translator must not accidentally attach a source `TirFunctionId` to a different position in the bytecode function table. Specialization IDs and physical indices must remain separate and deterministic.

`if`, loops, `match`, early return and `Result` propagation must preserve CFG and evaluation order. `?` lowers through ordinary `Result` branching/unwrapping; it does not introduce exceptions.

## 5. Existing v0 coverage versus historical restrictions

The original `TIR_TO_BYTECODE.md` described `Make/Get/Set` (struct/tuple) and `Tag/Unwrap` (enum) as unsupported and listed peephole optimisation as future work. **That list was tied to an early bootstrap stage and is not a valid description of every later VM phase.**

Other current repository docs describe subsequent ADT, array, tuple, index, pattern and peephole work. Therefore:

- Do not restore the old blanket `Unsupported Instructions (v0)` list as current truth.
- A construct may be *documented as present* at VM level while its current HIR → TIR → bytecode path remains incomplete.
- Use a branch-specific coverage ledger and regression tests to establish which sources are executable today.
- Unsupported lowering must fail deterministically with a useful diagnostic.

See [VM_COVERAGE.md](VM_COVERAGE.md), [TIR_LOWERING.md](TIR_LOWERING.md) and [BYTECODE_VM.md](BYTECODE_VM.md). They also require branch-aligned review.

## 6. Ownership-aware lowering obligations — v0.4, not certified as implemented

The source checker, typed TIR and lowerer must collectively preserve these contracts:

| Semantic event | Required verified behaviour |
|---|---|
| Passing a duplicable value | Original stays semantically valid; independent copy semantics |
| Last-use physical move | Permitted only if observationally equivalent |
| Explicit `consume` | Ownership transfers and source becomes unusable |
| Nonduplicable resource | One owner; no implicit duplicate through local loads, `Dup`, call or closure |
| `ref` access | Valid owner, shared read-only loan, non-owning |
| `inout` access | Exclusive mutating loan and no incompatible simultaneous access |
| `Slice(T)` | Borrowed, read-only; does not extend owner lifetime |
| `inout Slice(T)` | Exclusive write access to underlying elements |
| `Range` | Owned independent descriptor; bounds checked when applied |
| Closure environment | Explicit `ref`, `inout`, `consume`; recursive `fn[self]` cannot create a strong owner cycle |
| Async suspension | No live dynamic borrow crosses `.await()` |
| Exit/error/cancellation | Proper ownership cleanup; no double destruction or use-after-release |

No concrete new opcode names are assigned here. The final mechanism may use explicit instructions, ownership-typed operands, verifier metadata, or a combination. The verifier must have enough normalized facts to reject an incorrect lowering; it must not rely on an undocumented optimizer convention.

In particular, **replacing `StoreLocal(tmp); LoadLocal(tmp)`** is only legal when it does not change ownership transfer, copy effects, borrower lifetime, diagnostics or resource cleanup.

## 7. Effects, specializations and module boundaries

`@deny` and `@pure` constraints are established by compiler-owned semantic facts on supported paths; lowering must not bypass them. For `@deny(alloc)`, possible physical allocations from semantic copies or compacting buffers cannot be dismissed because one backend *might* optimise them away.

For imported functions and specializations, the contract must be available at the boundary through verified summaries. Modifying semantics or specialization identity can invalidate dependent caches.

If a bytecode cache artifact lacks sufficient trust to prove newly added ownership or effect obligations, reject the hit and recompile; **do not** assume old verified bytes imply current semantic correctness.

## 8. Optimisation policy

The bootstrap translator emits deliberately redundant `LoadLocal` and `StoreLocal` instructions. The VM documentation records later conservative peephole optimisation for generated temporaries and optional superinstructions. Treat that as an implementation optimisation, not a change to the source language.

Required guards:

- Apply transformations only to verified instruction ranges with adjusted branch targets.
- Re-run the relevant bytecode verifier after rewriting.
- Keep user-visible local/debug semantics consistent.
- Reject or roll back an invalid optimisation without executing unverified bytecode.
- Preserve effects and ownership facts; structural stack verification alone is not enough for v0.4.

AOT LLVM may optimise representations independently; it must satisfy the same language-level memory/effect semantics.

## 9. Comptime migration

On the documented K1 baseline, `comptime` is executed by the HIR interpreter before runtime TIR emission. The future VM-based evaluator must preserve semantics for compile-time branches, specialization, errors, effect restrictions, deterministic fuel/call-depth and memory invariants.

A compile-time VM evaluator is not automatically available merely because runtime bytecode executes; it must support evaluation during type checking before a complete runtime TIR module necessarily exists.

## 10. Validation matrix

| Gate | Minimum validation |
|---|---|
| Numeric typing | Signed/unsigned/float opcode matches checked TIR |
| CFG | All forward/backward targets patched to instruction boundaries |
| Function identity | Specialization and call targets deterministic |
| Stack/locals | Init-before-load, arity and merge checks pass |
| ADT/collection | Only supported typed operations lower; unsupported fail closed |
| Result | Propagation, early return, controlled failures preserved |
| Ownership | Copy/move/consume, exclusivity, provenance and cleanup verified |
| Effects | Deny/pure contracts survive compilation and cache hits |
| Reproducibility | Same supported inputs/build/options produce canonical artifacts |
| Backend parity | VM and future LLVM AOT agree on observable semantics |

The ownership row describes a **future gate**, not a passed implementation milestone.

## 11. Status and next review

The currently accessible source is the GitHub `main` snapshot, not the local `feature/k5-compiler-timings` branch. Before merging the documentation PR, compare this specification with the active `tempra-bytecode-lower` and verifier implementations, record the exact tested commit and revise any remaining historical status statements.

**Principle:** the lowering stage schedules and encodes computation; it does not define who owns memory.

### A3.2a — Typed read-return provenance

`TirFunction::return_borrow` identifies a runtime parameter, not a source
parameter or SSA value. The source `BorrowSummary` index is translated when
compile-time type parameters are omitted. The TIR verifier independently
checks reachable return paths with a must-provenance CFG analysis. Only aliases,
subviews and direct calls with a proven return contract preserve the origin.
Storage exposed to exclusive writes cannot establish the proof. Bytecode
lowering preserves this index and maps ordinary `Slice(T)` to `ReadSlice`,
including the result of `Slice` and `Bytes`. Exclusive parameter slots remain
`ExclusiveSlice`. Format 11 and semantic revision 19 replace older artifacts.

The bytecode verifier repeats the return-provenance check across its own stack,
locals and CFG. The VM checks the physical slice descriptor at typed boundaries.
This is the typed direct-return foundation, not the final non-owning slice
representation or a complete read-loan liveness verifier. `Any` aggregates,
static/temporary byte views and Exclusive-to-Read execution remain follow-up
work. No new opcode is introduced by this gate.

### A3.2b.1 — Direct exclusive-to-read downgrade

`ReadInoutSlice(result, parameter, start, end)` creates a non-owning read
subview of an incoming scalar `inout Slice(T)` parameter. Bounds are relative
to the incoming interval; execution preserves the original owner frame and
slot and converts the interval to absolute bounds. The result maps to
`ReadSlice`, not to an exclusive handle or an owned array. The opcode is
`ReadInoutSlice` (63); format 12 and semantic revision 20 are required.

The declared read-return source may be an exclusive slice parameter. TIR and
bytecode independently prove the return source and perform may-provenance
checks for locally created frame read loans and direct downgrade results.
A write or exclusive reborrow conflicts with a still-live read loan of that
owner, including aliases in slots/SSA temporaries and live operand-stack
values. Last use follows reachable CFG paths, not the physical lifetime of
an SSA slot. Nested reborrows preserve the original owner. The runtime also
checks the live owner, bounds, return source and local/stack liveness.

This is a deliberately bounded direct-descriptor gate: legacy `Slice` and
`Bytes` still use their prior representation, and frame-derived read loans
are not admitted into opaque aggregates, closure captures, indirect calls,
extern/native or host ABIs, or untyped slots. The HIR reference interpreter
supports the corresponding source behavior with its existing slice storage.
General non-owning read loans, aggregate provenance, temporary/static byte
views and full lifetime certification remain follow-up work.
