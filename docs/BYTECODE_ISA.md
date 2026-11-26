# Tempra — Bytecode ISA

**Version:** v0.2 (documentation consolidation)
**Status:** Current v0 stack-VM instruction families plus normative requirements for future ownership-aware execution. **Not** an implementation or ABI-completeness certificate.
**Scope:** Tempra Script's portable bytecode instruction semantics, structural verification, arithmetic, calls, and the migration requirements from [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md).

## 1. Authority and boundaries

This document replaces the former *Bytecode ISA Specification v0.1*. It is read together with:

- [BYTECODE_VM.md](BYTECODE_VM.md) — VM execution, frames, runtime values and current integration.
- [TIR_SPEC.md](TIR_SPEC.md) — typed intermediate semantics.
- [TIR_TO_BYTECODE.md](TIR_TO_BYTECODE.md) — translation and relocation.
- [BYTECODE_CACHE.md](BYTECODE_CACHE.md) — cache container, trust, verification and invalidation.
- [Tempra Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md) — **sole normative authority** for value, ownership, borrow, resource and memory safety semantics.
- [Specialized Effects v0.2](specs/specialized-effects-v0.2.md) and [Capability Contracts v0.1](specs/effects-v0.1.md) — effect-contract meaning.

**Status convention:** *Existing v0 instruction* describes the documented bytecode family, **not** guaranteed full source-level reachability. *Required by v0.4* indicates a future lowering/verifier obligation, **not** an implemented opcode. Historical claims should be checked against the active feature branch before any release.

The stack VM is the planned Tempra Script scripting runtime; Tempra Native's planned native backend is LLVM AOT. **No JIT or Cranelift path belongs to the target architecture.** Removing experimental Rust code is a separate engineering change.

## 2. Pipeline and representation

```text
Source
  → Lexer / Parser
  → Typed HIR + specialization/comptime pruning
  → TIR + verification/semantic contracts
  → TIR → Bytecode lowering
  → Bytecode verification
  → Tempra Script stack VM
```

The default execution path on the remote `main` snapshot may still be the HIR interpreter. The VM must not silently fall back after a failed bytecode lowering, verifier check, capability restriction or memory safety check. Switching the default requires an explicit parity gate.

The VM is stack-based. A function has a code body, local slots, an operand stack, a signature, a maximum stack bound, and optional debug information. A bytecode module contains constants, functions and code.

- Arguments are evaluated and pushed **left to right**.
- A `Call` consumes the declared arguments and initializes the callee's parameter locals.
- A non-`Unit` `Return` transfers one result to the caller. A `Unit` `Return` transfers no value.
- Control-flow targets must identify instruction boundaries; byte offsets, indices and encoding particulars are private to the current bytecode format.
- Source `let`/`var`, `ref`/`inout`, and `consume` meaning must not be inferred from physical slot or stack operations alone.

### 2.1 Binary stability and cache headers

The v0 instruction encoding and serialized cache are **not stable public ABI**. Source and compiler semantic revisions can invalidate existing artifacts.

The historical ISA document described a module magic value `TMPR`, while [BYTECODE_CACHE.md](BYTECODE_CACHE.md) documents the persisted **cache-container** magic `TMBC` and bytecode format version 2. These are not interchangeable claims: the cache spec governs its actual serialized header. Do not hard-code `TMPR` as the current cache magic.

A cache hit must pass compatibility/trust validation **and** bytecode verification. A raw decoder is not itself an execution authorization. Introducing ownership metadata, new verification semantics or changed effect enforcement requires the appropriate semantic-revision/cache migration.

## 3. Operand notation

In tables, `[a, b] → [c]` shows the top-of-stack input suffix (left to right) and its replacement. `T` denotes a verified operand type. The display names are documented v0 opcode names; byte values, exact operands, encodings, availability of optimised variants and current arity must be taken from the implementation when emitting test vectors.

### 3.1 Constants — existing v0 families

| Opcode | Operand | Stack effect | Meaning |
|---|---|---|---|
| `ConstUnit` | — | `[] → [Unit]` | Unit value |
| `ConstBool` | Boolean | `[] → [Bool]` | Boolean value |
| `ConstI64` | `i64` | `[] → [Int64]` | Signed integer |
| `ConstU64` | `u64` | `[] → [UInt64]` | Unsigned integer |
| `ConstF64` | `f64` | `[] → [Float64]` | Floating-point value |
| `ConstString` | Constant index | `[] → [String]` | UTF-8 value from constant pool |

`UInt8` exists in the language/runtime but the original v0 ISA does **not** list a dedicated `ConstU8`; do not infer one from this document. Numeric source reachability and conversion support may be narrower than bytecode-level tests.

### 3.2 Locals and stack — existing v0 families

| Opcode | Operand | Stack effect | Meaning |
|---|---|---|---|
| `LoadLocal` | Local index | `[] → [T]` | Read local value |
| `StoreLocal` | Local index | `[T] → []` | Assign local from stack |
| `Pop` | — | `[T] → []` | Discard stack value |
| `Dup` | — | `[T] → [T, T]` | Duplicate stack operand |

**Ownership qualification:** these are *physical instruction descriptions*, not permissions to duplicate resources or implicitly turn a borrow into an owned value. Under v0.4:

- Loading a duplicable value may be lowered with a physical copy, reuse, or move if observationally equivalent.
- Loading or duplicating a nonduplicable resource cannot create two independent owners.
- A `consume` transfer invalidates the source **semantically**; the checker/verifier must reject later uses.
- `Pop`/slot overwrite/return/unwind must not leak or double-release owned resources.
- Existing opcodes alone do not establish these guarantees. An ownership-aware TIR/bytecode representation, metadata, or an equivalent verified protocol must supply them. **No new concrete opcode is mandated here.**

**A1 ownership-aware implementation contract:** `LoadLocal` reads a value at
the time of the instruction, including when the slot is an incoming scalar
borrow. It must not put a raw borrow handle on the operand stack. `StoreLocal`
assigns a value (through an exclusive incoming place when applicable); it cannot
bind or copy a raw handle. `BorrowLocal`, `BorrowLocalMut` and `BorrowSliceMut`
are the explicit handle producers. The verifier checks the matching capture or
direct-call consumer. The serialized format version is incremented when this
contract changes because old code may rely on implicit handle copying.

**A3.2a read-return contract (format 11):** a read-only slice uses the
`ReadSlice` type tag (10), distinct from `Any` and `ExclusiveSlice`. Function
records include an optional zero-based runtime parameter index after the return
type (presence byte 0 or 1, then a little-endian u32 when present). A defined
function returning `ReadSlice` must identify a non-exclusive read-slice
parameter. The verifier checks that each reachable return preserves that
origin through loads, stores, subviews and direct calls; CFG merges retain only
proven origins. Unknown aggregate projections and indirect return ABIs cannot
establish the proof. This gate does not yet establish all dynamic read-loan
lifetimes or change the physical `Rc`-backed runtime representation. Exclusive
slice return/downgrade remains unsupported. The instruction IDs are unchanged.

### 3.3 Arithmetic — existing v0 families

| Type | Add | Sub | Mul | Div |
|---|---|---|---|---|
| `Int64` | `AddI64` | `SubI64` | `MulI64` | `DivI64` |
| `UInt64` | `AddU64` | `SubU64` | `MulU64` | `DivU64` |
| `Float64` | `AddF64` | `SubF64` | `MulF64` | `DivF64` |

Each binary arithmetic instruction consumes two same-typed operands and produces one same-typed result.

**`Int64`:** checked add, subtract, multiply and divide. Overflow is a controlled error; division by zero is a separate controlled error; `Int64::MIN / -1` overflows. Unary signed negation must obey the checked contract, regardless of its lowering sequence. These operations conservatively require `Panic` until proven otherwise by an independently approved mechanism.

**`UInt64`:** checked addition/multiplication overflow, subtraction underflow, division by zero. Do not silently reinterpret them as signed integer operations or wrap.

**`Float64`:** IEEE 754 behaviour, including NaN, infinities, signed zero and division by zero. Arithmetic must not be rewritten into checked integer operations. `LtF64` and `GtF64` are ordered comparisons, both false if either operand is NaN.

The runtime must report controlled Tempra failures rather than expose Rust panics, undefined behaviour, incidental host traps or unchecked arithmetic.

### 3.4 Comparison and Boolean operations — existing v0 families

| Opcode | Stack effect | Meaning |
|---|---|---|
| `Eq` | `[T, T] → [Bool]` | Equality subject to type/operator rules |
| `LtI64`, `GtI64` | `[Int64, Int64] → [Bool]` | Signed comparisons |
| `LtU64`, `GtU64` | `[UInt64, UInt64] → [Bool]` | Unsigned comparisons |
| `LtF64`, `GtF64` | `[Float64, Float64] → [Bool]` | IEEE-ordered comparisons |
| `Not` | `[Bool] → [Bool]` | Negation |
| `And`, `Or` | `[Bool, Bool] → [Bool]` | Boolean combination |

`Eq` is not a licence to compare closure values, arbitrary raw addresses or noncomparable resources. The type checker determines comparability. Source-level `&&` and `||` must retain short-circuit evaluation; an eager `And` or `Or` opcode cannot replace it unless the operands have already been evaluated with equivalent control flow.

### 3.5 Branches, calls and returns — existing v0 families

| Opcode | Operand | Stack effect | Meaning |
|---|---|---|---|
| `Jump` | Target | No data result | Unconditional branch |
| `JumpIfFalse` | Target | `[Bool] → []` | Conditional branch |
| `Call` | Function index, arity | `[args…] → [ret?]` | Direct call |
| `Return` | — | `[ret?] → []` | Function return |
| `Unreachable` | — | No valid successor | Controlled execution failure if reached |

At joins, the stack shape and types must agree. Jumps cannot enter the middle of an operand encoding. Call arity and argument types must match the target function's verified signature; source-specialization IDs must map consistently to concrete bytecode function indices.

Error propagation through `?` is ordinary checked control flow over `Result` values; it is **not** exception unwinding.

### 3.6 Enums — existing documented v0 families

| Opcode | Operand | Stack effect | Meaning |
|---|---|---|---|
| `MakeEnum` | Enum, variant, count | `[payloads…] → [enum]` | Create variant |
| `Tag` | — | `[enum] → [tag]` | Inspect variant tag |
| `Unwrap` | Variant, index | `[enum] → [T]` | Checked payload extraction |

The tag's exact runtime representation is an implementation detail. Pattern matching must preserve arm order, guards, exhaustiveness and source-level ownership rules, even if a particular lowering stage does not yet support every pattern.

### 3.7 Arrays, tuples, slices and strings — existing documented v0 families

| Opcode | Operand | Stack effect | Meaning |
|---|---|---|---|
| `MakeArray` | Element count | `[elements…] → [Array]` | Own array value |
| `MakeTuple` | Element count | `[fields…] → [Tuple]` | Positional product |
| `GetIndex` | — | `[collection, index] → [T]` | Bounds-checked lookup |
| `Slice` | — | `[collection, start, end] → [Slice(T)]` | Borrowed view |
| `Len` | — | `[collection] → [Int]` | Element count / string scalar count |
| `Size` | — | `[String] → [Int]` | UTF-8 byte size |
| `Bytes` | — | `[String] → [Slice(UInt8)]` | Read-only bytes view |

The notation does **not** prescribe whether a collection operand is physically retained, borrowed, consumed or loaded via a helper: that distinction is semantic and depends on the verified contract.

Normative requirements from v0.4:

1. `Slice(T)` is a non-owning read-only view; `inout Slice(T)` is a checked exclusive mutable access, **not** a separate public `MutableSlice(T)`.
2. A borrowed slice does not keep its owner alive; every live use requires valid provenance.
3. `Range` is a value independent of a buffer. Slicing applies a half-open interval `[start, end)` and checks bounds at application.
4. `Bytes` must not expose writable string bytes through a read-only view.
5. No silent copy may repair an illegal escaping borrow.
6. Mutable access, views of temporary owners and data crossing suspension points require v0.4 checks. The `Slice` instruction alone does not encode a complete borrow proof.

`GetIndex` and `Slice` require checked indices. A malformed operand or wrong type must not trigger an uncontrolled host panic.

## 4. Structural bytecode verification

Before execution of a bytecode module the verifier must establish, within its currently supported feature surface:

1. **Instruction decoding:** valid opcodes, operands and constant/function/local indices.
2. **CFG integrity:** jump targets align with instruction boundaries; all reachable blocks have legal terminators.
3. **Stack discipline:** no underflow; consistent stack height/type on join; correct return shape; declared `max_stack` respected.
4. **Locals:** definite initialization before load; assignments meet slot/type constraints.
5. **Calls:** arity, argument type, return type and function identity are correct.
6. **Typed operations:** numeric opcode selection follows verified types; no accidental unsigned/float-to-signed rewriting.
7. **Failure containment:** invalid input or impossible reachable `Unreachable` is reported safely, not converted into undefined host behaviour.

Structural verification **is not equivalent** to having implemented the entire v0.4 ownership checker. A future verified bytecode boundary must also preserve ownership-acquisition and consumption, loan scopes, provenance, exclusive access, cleanup on every exit, nonduplicable types, capture modes and suspension restrictions. These requirements may be encoded by additional facts/instructions or by a checked lowering certificate; the representation remains **open**, not silently assumed present.

Untrusted or stale cache artifacts must never bypass these checks.

## 5. Capability and effects boundary

`@deny(...)`, `@pure`, and specialized effect restrictions are checked by the compiler on the supported path. Current frontend-only `check` or generic IR emission is **not automatically** a certificate that all runtime/capability contracts were enforced.

- `@pure` denies I/O, process, spawn and nondeterminism; it does **not** by itself deny allocation or panic.
- `@deny(alloc)` must apply a conservative, backend-independent allocation contract. A possible dynamic copy/compaction cannot be accepted merely because one optimizer may eliminate it.
- A bytecode cache hit cannot bypass a newly required semantic analysis. A compatible artifact must carry sufficient trust information, or compilation must rerun the relevant check.
- A JIT is **not** part of the target architecture.

## 6. Lowering and optimization invariants

The original stack lowering materializes TIR values in local slots, emits loads before consumers, stores results after producers and patches branch targets after block emission. See [TIR_TO_BYTECODE.md](TIR_TO_BYTECODE.md).

Existing optimisation ideas include stack preallocation, superinstructions, instruction dispatch improvements and peephole removal of redundant temporary `StoreLocal`/`LoadLocal` pairs.

Each optimisation must preserve:

- evaluation order and Result/controlled-error behaviour;
- observable value independence and copy effects;
- `consume` invalidation and correct ownership transfer;
- borrow provenance, exclusive access and last-use termination;
- cleanup on ordinary return, error and cancellation where applicable;
- effect contracts and deterministic compiler outputs.

An optimisation may avoid a *physical* copy only when the semantic operation remains valid. It cannot make invalid source ownership or `@deny(alloc)` contracts acceptable through a target-specific lucky outcome.

## 7. Profiling and performance — VM only

Call counts, instruction counts, backedge counts, opcode histograms and superinstruction statistics may remain useful for VM profiling, benchmarking and optimisation. **Hotness is telemetry**, not a trigger for JIT compilation.

Do not document JIT options, native promotion, JIT fallback or Cranelift-specific ABI as future ISA capabilities. Historical prototypes belong under `docs/archive/jit/`.

## 8. Comptime migration gate

Current remote documentation describes a K1 HIR-based `comptime` evaluator. Planned Tempra Script consolidation moves compile-time execution to the VM **only after** semantic parity is demonstrated.

The bytecode execution boundary must preserve selected-branch pruning, specializations, deterministic fuel/call-depth policies (or an explicitly approved revised policy), restricted host effects, error diagnostics and memory semantics. Counting raw bytecode instructions is not automatically equivalent to the existing HIR semantic-tick policy.

Comptime is an execution phase, **not** a public effect or permission to bypass borrowing.

## 9. Worked examples (illustrative, not serialized byte offsets)

### Arithmetic

```tempra
fn add(a: Int64, b: Int64) -> Int64 {
    a + b
}
```

```text
LoadLocal 0
LoadLocal 1
AddI64
Return
```

### Conditional

```tempra
fn max(a: Int64, b: Int64) -> Int64 {
    if a > b { a } else { b }
}
```

```text
LoadLocal 0
LoadLocal 1
GtI64
JumpIfFalse else
LoadLocal 0
Return
else:
LoadLocal 1
Return
```

Labels here illustrate CFG structure and are **not** literal bytecode operands.

## 10. Conformance and implementation ledger

| Case | Required result | Current guarantee? |
|---|---|---|
| Well-typed verified arithmetic | Correct typed value or controlled failure | Existing v0 contract on supported paths |
| Wrong numeric opcode/type | Rejected before execution | Existing structural/typed verification |
| Bad jump or uninitialized local | Rejected | Existing verification requirement |
| Cache artifact with stale semantics | Invalidated/recompiled | Existing cache contract |
| `Dup` of nonduplicable resource | Rejected | **v0.4 requirement; implementation not certified** |
| Borrow escaping an owner | Rejected | **v0.4 requirement; implementation not certified** |
| Mutating a live read-only slice | Rejected | **v0.4 requirement; implementation not certified** |
| Dynamic borrow through `.await()` | Rejected | **v0.4 requirement; async not certified** |
| Actor state isolated and handles cycle-safe | Per v0.4, handles deferred | **Not implemented/certified** |
| Comptime via bytecode VM | K1 parity gate passes before promotion | **Planned** |
| JIT/Cranelift runtime | No target path | **Legacy code removal separate** |

## 11. Implementation validation required before the documentation PR

Read the active VM branch and confirm actual instruction variants/operands, the precise structural verifier surface, source reachability, bytecode cache format, optimisation and profiling options. This document deliberately does **not** invent numeric opcode values, a final instruction count or completed ownership opcode definitions.

Future native Tempra Native/LLVM AOT must preserve the **same language semantics**, but is not required to share the bytecode encoding.

---

**Invariant:** *Bytecode may change; Tempra's checked semantics must not.*
