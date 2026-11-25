# Tempra — Typed HIR to TIR Lowering v0.2

**Status:** Consolidation of historical bootstrap, Result/ADT and loop lowering. This document distinguishes past implementation evidence from future v0.4 ownership requirements; it does not certify whole-language lowering or the current local branch.

**References:** [TIR_SPEC.md](TIR_SPEC.md), [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md), [Surface Syntax v0.2](TEMPRA_SURFACE_SPEC_v0.2.md), [GRAMMAR.md](GRAMMAR.md), [TIR_TO_BYTECODE.md](TIR_TO_BYTECODE.md).

## 1. Responsibility and input

The lowering consumes typed HIR and a resolved `TypeContext`, then emits typed TIR function definitions and explicit CFG. It must preserve source evaluation order and control-flow semantics; it must diagnose operations not representable in TIR rather than silently deleting them.

```text
Source → AST → HIR → type checking / comptime selection / specialization
       → HIR→TIR lowerer → structural/typed TIR verifier
       → effects / ownership facts → bytecode lowering → Tempra Script VM
```

The remote-main runtime still documented HIR execution as the default; the VM-first product direction is **planned** until parity passes. The future `comptime` migration is likewise separately gated. Neither a JIT nor Cranelift forms part of the target pipeline.

## 2. Historical baseline and scope

The original file mixed Phase 12.2 bootstrap, Phase 14.4 ADT/Result expansion and Phase 14.7 indexed `for-in` lowering. They should be understood chronologically, not as one unchanging feature list.

| Feature | Documented historical work | Qualification |
|---|---|---|
| Return checking | Non-Unit missing-return detection | Source expression-tail semantics must still agree with type checker |
| Slots/locals | Parameters in entry slots, `let`/`var` mutability | Physical storage is not semantic ownership |
| Direct calls | Resolved signatures and arity checks | Generic/specialized ID mapping must remain correct |
| Conditionals | Terminated branches avoid unnecessary join | Joining expression results requires correct value semantics |
| Builder | Reject append after terminator | Does not establish full ownership soundness |
| `Result` `?` | Explicit tagged early-return flow | Cleanup on error path is a future v0.4 obligation |
| ADTs | Constructor/tag/unwrap expansion | Exact active support needs branch tests |
| `for-in` | Indexed Array/Slice loops | Does not prove general Iterable protocol parity |

The older statement “closures are not implemented” was a **Phase 12.2 non-goal** and is not a reliable current status declaration. The exact closure-capture support requires active-branch regression verification.

## 3. Parameters, locals, results and mutability

- Each function has an entry block with parameter values mapped to declared slots as needed.
- Source `let` is immutable; source `var` permits reassignment. Assignment errors should be diagnosed before invalid TIR is emitted.
- Named-function calls require a resolved target and a valid argument count/type sequence.
- A non-Unit function cannot fall off a reachable path without a result. If all branches terminate, do not insert an artificial join.
- TIR slot `load`/`store` are implementation operations. They do **not** imply a source move, copy or legal borrow by themselves.
- Parameters without `ref`/`inout` use value semantics. Explicit `consume` invalidates the source; a compiler-optimized last-use move does not change source legality.

### Illustrative CFG

```text
if condition {
    return left
} else {
    return right
}
```

```text
entry → condbr condition, then, else
then  → ret left
else  → ret right
(no reachable join required)
```

This is CFG pseudocode, not literal TIR serialization.

## 4. Types and arithmetic

The historical lowerer documented `Unit`, `Bool`, `Int64`, `UInt64`, `Float64`, `String`, `Never`, `Option(T)` and `Result(T,E)`; later TIR also represents aggregates. Source support and TIR type representation must not be conflated.

Typed operations must preserve:

- checked `Int64` arithmetic, including signed division overflow;
- checked `UInt64` arithmetic, including subtraction underflow;
- `Float64` IEEE 754 behavior, NaN, signed zeros and ordered comparisons;
- strict rejection of an unresolved or unsupported numeric operation rather than defaulting to signed `Int64`;
- correct effect facts for may-fail arithmetic (`Panic`) and potential runtime allocation (`Alloc`).

Unary `Float64` negation historically lowered via multiply by `-1.0`; any replacement must retain IEEE-754 signed-zero/NaN semantics. `UInt8` arithmetic reachability remains limited in the documented older lowering and must be verified explicitly before claiming parity.

## 5. Result/Option construction and `?`

The documented Result propagation uses `tag`, equality, `condbr`, `unwrap` and an early `ret`. Semantically:

```text
value = evaluate expression once
if value is Err(error):
    return Err(error)
else:
    continue with Ok payload
```

This is a conceptual algorithm, not a source-language exception. An error branch must preserve discriminants, payload types and ownership; when deterministic cleanup is implemented, resources that do not escape must be released exactly once on both success and failure paths. Dropping or duplicating an `Err` payload to simplify CFG is forbidden.

Nested pattern/guard lowering, if-expression values and full ADT structural checks must be tracked through actual tests, not inferred from these basic forms.

## 6. `for-in` on Arrays and Slices

The Phase 14.7 implementation lowered a collection loop to private indexed state:

```tempra
for item in items {
    use(item)
}
```

**Conceptual lowering:**

1. Evaluate `items` exactly once into an inaccessible `$iter_obj` slot.
2. Initialize private `$idx = 0` and `$len = len($iter_obj)` slots.
3. Branch to a condition block; if `$idx < $len`, load `get_index($iter_obj,$idx)` and execute the body, otherwise exit.
4. Increment `$idx`, branch back, and maintain correct `continue`/`break` destinations.
5. Limit the source loop binding's scope to the body.

Every access must be bounds-safe. Loop optimizations must not extend a dynamic Slice past the validity of its owner or accidentally copy a nonduplicable element. Physical private slots do not justify a second owner. This indexed strategy does not itself imply that arbitrary user-defined Iterator/Iterable implementations are supported by the same path.

## 7. Branches, patterns and expression values

The lowering must preserve:

- `if` and `match` expression result types;
- first matching pattern/guard semantics and ordering;
- `is` narrowing and binding scope;
- exhaustiveness and discarded-Result restrictions from typechecking;
- appropriate termination of every generated block;
- accurate source spans/diagnostics on unsupported constructs.

Do not confuse the *grammar's* supported syntax with the active lowerer's execution coverage. Branch-produced values may be implemented through slots or future block parameters; no new representation is frozen by this document.

## 8. Generics, comptime and modules

Generic type/value specialization and compile-time branches are resolved by the frontend before emitting concrete executable TIR where applicable. Unselected `if comptime` branches and `comptime match` arms must be pruned before effects analysis; semantic pruning must not depend on later optimizer luck.

`TirFunctionId` is not the physical bytecode vector index. For multiple modules/specializations, preserve canonical deterministic request order, function identity and explicit call mappings. The lowerer must not lose source capability contracts, even if a method or template is not emitted.

**Current comptime status:** HIR evaluator under K1 in the remote-main baseline. **Future:** VM evaluator after parity for fuel, errors, allowed operations, specialization, determinism and v0.4 memory invariants. TIR lowering cannot assume comptime needs a fully formed runtime module during type checking.

## 9. Closure and method semantics

The approved language semantics include explicit `fn[ref x]`, `fn[inout x]`, `fn[consume x]`, recursive `fn[self]`, and methods with `self` / `inout self`. No implicit captures are allowed. `consume` invalidates the source. Borrowed dynamic closure environments must remain non-escaping; owned escaping environments are allowed only under v0.4 acyclic and copyability rules.

The actual lowering of closure environments, recursive self identity, receiver modes and escaping behavior requires a concrete representation and tests. **Do not equate conceptual support in the Surface Spec with proved VM parity.** `ref self`/`consume self` spelling remains subject to surface/parser alignment.

## 10. Ownership-aware lowering obligations (approved; not certified)

| Contract | Obligatory treatment |
|---|---|
| Value copy | Semantically independent; may be physically reused only if equivalent |
| `consume` | Source invalidation and unique ownership transfer |
| Unique resource | Cannot duplicate via slot loads, calls, tuples or captures |
| `ref` | Non-owning shared read, valid owner/provenance |
| `inout` | Exclusive mutability, no competing loans |
| `Slice(T)` | Borrowed read-only; `inout Slice(T)` exclusive writable |
| `Range` | Independent owned half-open descriptor; check on application |
| Non-escaping struct with Slice | Aggregate inherits field provenance |
| CFG/loops | NLL-like last uses and conservative joins/fixed-point checks |
| `.await()` | No live dynamic borrow across suspension |
| Error/return/cancel | Correct cleanup exactly once |
| `@deny(alloc)` | No optimizer-dependent acceptance of potentially allocating copy |

Borrow summaries and other ownership facts must reach TIR verification and consumers. The final set of ownership opcodes, metadata or normalized facts remains open. Lowering must never insert implicit copies to repair illegal escaping borrow lifetimes.

## 11. Verifier and error policy

`tempra-tir-lower` should report a controlled lowering error for unsupported constructs, bad arity, invalid assignment targets or missing returns. Builder/verifier failures are structured internal compiler errors, not source-language traps. Verify generated TIR before analysis/execution; verify bytecode again after lowering and optimizations.

Lowering must preserve deterministic diagnostic ordering and source association where known. A lowering-owned side table can associate `TirLocation` with module identity and an existing source span; the legacy core TIR lacks full source mapping.

## 12. Developer inspection: `emit-tir`

Historical CLI:

```bash
tempra emit-tir path/to/file.tmpr
```

The command parses/checks, lowers, verifies and prints TIR without executing the program. It is a diagnostic/inspection path, **not** certification of `@deny`/`@pure` enforcement and not a license to bypass memory verification. Exact CLI behavior must be checked against the active feature branch.

`interpret_tir_source` is a separate limited validation hook, documented in [TIR_INTERP.md](TIR_INTERP.md). It is not the default VM and does not silently substitute for it.

## 13. Tests and acceptance gate

Before declaring full lowering parity, test: all CFG exits/joins; `Result` early-return; nested patterns/guards; loops and `continue`; methods, recursive/escaping closures, generics, independent copies, `consume`, unique-resource cleanup, borrowed field provenance, `.await()` rejection, allocation contracts and cache parity. Mark each behavior as implemented, rejected safely, or deferred based on executable tests on the active branch.

**Rule:** HIR→TIR lowering preserves Tempra semantics; it never decides new ownership rules.
