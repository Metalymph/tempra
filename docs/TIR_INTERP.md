# Tempra — TIR Micro-Interpreter v0.2

**Status:** Limited internal TIR execution and regression-testing tool. Its documented Phase 12.5 instruction subset is **not** the full language runtime and not the future canonical `comptime` implementation.

**References:** [TIR_SPEC.md](TIR_SPEC.md), [TIR_LOWERING.md](TIR_LOWERING.md), [BYTECODE_VM.md](BYTECODE_VM.md), [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md).

## 1. Purpose and boundaries

`tempra-tir-interp` evaluates a verified **subset** of TIR instructions for development and differential validation. It helps test the frontend, typed TIR, CFG and arithmetic without involving bytecode generation. It is **not** the production Tempra Script stack VM, a fallback for unsupported bytecode, or a substitute for an ownership-aware verifier.

The user-facing scripting runtime target is the Tempra Script stack VM after parity. The existing K1 comptime evaluator uses HIR in the documented remote-main baseline; the **planned canonical comptime engine is the Tempra Script VM**, gated on equivalent semantics. This TIR micro-interpreter is not promoted to that role.

## 2. Execution model

The historical micro-interpreter is frame-based:

- `TirValueId → TirRuntimeValue`: typed computed-value table;
- `TirSlotId → TirRuntimeValue`: local mutable storage;
- program cursor: current function, block and instruction index;
- each completed block selects a `br`/`condbr` successor, returns, or fails on reachable `unreachable`;
- direct recursive intra-module function calls use their own frames.

An implementation must execute verified TIR, not arbitrary malformed instructions. It should report invalid/unsupported conditions through `TirInterpError` rather than panic on user-supplied source or IR.

## 3. Internal APIs (documented historical signatures)

```rust
interpret_function(
    module: &TirModule,
    function_name: &str,
    args: Vec<TirRuntimeValue>,
) -> Result<TirRuntimeValue, TirInterpError>
```

`verify_module` is invoked before execution; verifier failures produce `TirInterpError::VerifierFailed`.

The driver previously exposed an internal source-level integration hook:

```rust
interpret_tir_source(
    source: &str,
    function_name: &str,
    args: Vec<TirRuntimeValue>,
) -> Result<TirRuntimeValue, DriverError>
```

This hook exercises source → HIR/type checking → TIR lowering → verification → interpretation, but only for the supported subset. Current names/signatures should be checked on `feature/k5-compiler-timings` before being treated as a stable library API. Neither hook is advertised as the normal `tempra run` backend.

## 4. Historical instruction coverage (Phase 12.5)

| Instructions | Documented status |
|---|---|
| `const` | Primitive constants |
| `slot`, `load`, `store` | Supported local slots |
| `add`, `sub`, `mul`, `div` | `Int64`, `UInt64`, `Float64` |
| `eq`, `lt`, `gt` | Primitive compatible types |
| `and`, `or`, `not` | Bool |
| `call` | Intra-module calls |
| `ret` | Value or Unit |
| `br`, `condbr` | Basic-block control flow |
| `unreachable` | Returns controlled error |
| `make`, `get`, `set` | Historically `UnsupportedInstr` |
| `tag`, `unwrap` | Historically `UnsupportedInstr` |

This table describes **the micro-interpreter**, not the bytecode VM. Later TIR/VM documentation added ADTs, arrays, slices, `Result` and methods in the bytecode pipeline; their availability does **not** imply that this narrower interpreter gained the same operations.

`get_index`, `slice`, `len`, `size`, `bytes`, closures, async, native externs and structural ownership checking should not be asserted as implemented without active code/tests. Unsupported instructions must fail closed.

## 5. Runtime values

The documented `TirRuntimeValue` cases are:

```rust
TirRuntimeValue::Unit
TirRuntimeValue::Bool(bool)
TirRuntimeValue::Int64(i64)
TirRuntimeValue::UInt64(u64)
TirRuntimeValue::Float64(f64)
TirRuntimeValue::String(String)
```

This value table is distinct from the VM's richer `VmValue`. It does not define Tempra's language-level ownership semantics, and the absence of an aggregate case in the micro-interpreter does not ban that aggregate from Tempra.

## 6. Arithmetic and controlled failures

Historically, integer operations use the common checked Tempra runtime kernel:

- `Int64`: checked overflow and division by zero, including `MIN / -1`;
- `UInt64`: checked overflow, subtraction underflow, division by zero;
- `Float64`: IEEE-754 arithmetic including NaN, infinities and signed zero; division by floating zero is not an integer-style division-by-zero error.

The micro-interpreter must agree with Tempra Script VM and future LLVM AOT on shared supported operations. It must never allow Rust overflow panics, unchecked host traps or host-dependent wrapping as observable Tempra outcomes.

### Error categories from the historical API

- `VerifierFailed` — malformed or invalid TIR;
- `Int64Arithmetic`, `UInt64Arithmetic` — checked integer failures;
- `DivisionByZero` — historical compatibility error, numeric division now uses typed error variants;
- `UnreachableExecuted` — a reachable impossible path;
- `UnsupportedInstr` — unavailable operation;
- `WrongArity`, `UnknownFunction` — invalid calls.

Exact Rust enum shape is version-specific; this document records the API names found in the remote-main documentation.

## 7. Compared with Tempra Script bytecode VM

| Aspect | TIR micro-interpreter | Tempra Script bytecode VM |
|---|---|---|
| Input | Verified TIR module | Verified bytecode module |
| Dispatch | TIR instruction enum/CFG | Stack-machine opcode stream |
| Primary use | Developer validation | Planned default script execution |
| Value coverage | Primitive subset | Struct, enum, array, tuple, Slice and others on supported paths |
| Native stdlib | Not covered by the micro-interpreter contract | Supported subset through runtime native bridge |
| Caching | Not a bytecode runtime | Bytecode cache with semantic trust checks |
| Memory v0.4 | Normative requirement for any admitted operations, not certified | Normative requirement, implementation gate pending |
| Comptime | Not selected as canonical engine | Planned after parity |

The VM cannot silently fall back to the TIR micro-interpreter on failed lowering, capability verification or runtime errors. Test or developer use of this tool is explicit.

## 8. Memory & Ownership v0.4 boundary

All Tempra execution must eventually conform to Memory v0.4; this small interpreter is **not yet a proof** of the following:

- independent by-value copies and valid physical reuse;
- explicit `consume` source invalidation;
- nonduplicable resources and exactly-once destruction;
- `ref`/`inout` exclusivity, provenance and non-escaping borrowed Slice;
- borrowed aggregates, closure captures, actor isolation;
- cleanup on Result error and cancellation;
- no dynamic borrows over `.await()`;
- consistent effect contracts for potentially allocating copies.

Because these operations are not within its documented supported subset, the micro-interpreter must reject unsupported TIR. If any of the operations are added, the corresponding verifier, execution semantics, test cases and cache implications must be designed first. **Do not introduce a second runtime-specific ownership model for convenience.**

## 9. Effects and comptime

Raw TIR interpretation does not reconstruct source `@deny`/`@pure` declarations or a complete capability-analysis product. Source TIR interpretation was documented as rejecting annotated HIR rather than silently bypassing enforcement. Frontend-only `check` and TIR inspection similarly do not certify effect restrictions.

K1's HIR comptime policy allows deterministic local computation and temporary allocation but rejects runtime host-observable I/O/process/environment operations, with a deterministic semantic fuel/call-depth policy. Migrating to the VM requires proving the same policy, not extending this micro-interpreter by default.

## 10. Verification and regression use

Recommended explicit test categories:

1. Arithmetic and comparisons on supported primitive types.
2. Valid and invalid CFG edges, parameter mapping and recursion.
3. Typed failure categories versus host panics.
4. Rejected `make`/`tag`/other unsupported TIR instructions.
5. Agreement with the bytecode VM for the **intersection** of supported operations.
6. No unsafe source contract bypass through internal hooks.
7. Deterministic behavior with fixed module, inputs and compiler build.

The specific active-branch state must be confirmed before claiming these tests passed. Keep this file as documentation of an **internal validation tool**, not a second user-facing evaluator.

## 11. Deferred topics

Full heap/object semantics, standard library and I/O execution, closure environments, actors/tasks, FFI, global heap lifetime management, complete v0.4 verifier, direct source execution and canonical comptime VM evaluation do not become micro-interpreter requirements by default.

**Rule:** A useful small TIR interpreter is a testing tool; a safe Tempra Script runtime requires the VM's separate verified execution path.
