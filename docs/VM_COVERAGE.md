# Tempra Script VM — Coverage and Parity Ledger v0.2

**Status:** documentation-backed inventory, **not** a fresh CI attestation.
**Baseline:** available `Metalymph/tempra` GitHub `main`; local `feature/k5-compiler-timings` must be checked before merge.
**Memory authority:** [Tempra Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md).
**Related:** [BYTECODE_VM.md](BYTECODE_VM.md), [TIR_SPEC.md](TIR_SPEC.md), [TIR_LOWERING.md](TIR_LOWERING.md), [BYTECODE_CACHE.md](BYTECODE_CACHE.md).

## 1. Purpose

Coverage is tracked **end to end**, not inferred from a parser rule, TIR opcode, native intrinsic or isolated VM unit test. A language feature is covered only when a supported source program can be typechecked, lowered, verified and executed on Tempra Script with expected values, failures, effects and ownership semantics.

The historical HIR interpreter is a differential reference, **not** an approved long-term second scripting runtime. The plan is VM-first Tempra Script following source parity. JIT/Cranelift are legacy features being removed and are not VM coverage requirements. Tempra Native LLVM AOT is an independent future parity target.

## 2. Evidence categories

| Label | Meaning |
|---|---|
| Documented supported | Historical `VM_COVERAGE.md` asserted tests for its bounded subset; not freshly rerun |
| Documented gap | Historical `VM_COVERAGE.md` recorded missing or partial support |
| Normative pending | Required by v0.4 or another current spec; no completion evidence provided |
| Candidate for verification | Documentation is incomplete or contradictory; inspect source and run tests |
| Verified on branch | **Use only** after recording commit, command, platform, output and specific passing cases |

The May 12, 2026 report said **19 differential tests and cache lifecycle tests passed**. That is historical test evidence only. It does not establish complete VM parity in November 2026, nor memory-safety compliance.

## 3. Documented language surface

| Feature | Historical evidence on `main` documentation | Necessary parity check |
|---|---|---|
| `Int64`, `Bool`, basic arithmetic/logic | Documented supported | Checked overflow, short-circuit logic, diagnostics |
| `UInt64`, `Float64` at TIR/VM boundary | Numeric parity recorded in later K3 documents | Source reachability, IEEE NaN/signed zero, exact errors |
| `let`, `var`, assignments | Documented supported via VM/TIR docs | Assignment legality, value aliasing, ownership |
| `if`/`else`, `is` narrowing | Documented supported | Branch merge, shadowing, borrow last-use |
| `for` over Array/Slice | Documented supported | Filter, return/break/continue, invalidation and cleanup |
| `for` expressions | Documented supported | Collected array values and `Alloc` accounting |
| Direct function calls, recursion | Documented supported, except some closure cases | Deep recursion, identity, error propagation |
| Struct create/get/mutable field set | Documented supported | Field mutability, copies, borrowed fields |
| `extend` methods and `self` | Documented basic support | `inout self`, generic methods, receiver ownership |
| Option/Result, `?` | Documented supported | Early return, Result discard, cleanup on error |
| `match` core literal/wildcard/variant | Documented supported | Exhaustiveness, order, guards and nesting |
| Arrays, slices and string metadata | Documented supported | Bounds, UTF-8, read-only views, owner provenance |
| Persistent bytecode cache | Documented supported bounded single-source path | Invalid/corrupt/mismatch handling and fresh path parity |
| Generic functions and specialization | Other docs record supported subsets | Multi-module identity, dispatch and deterministic output |
| `comptime` expressions and pruning | Supported in HIR evaluator; VM migration planned | VM comptime evaluator **not** yet assumed |
| Capability contracts and specialized denials | E/K4 documentation records enforcement on supported VM path | No bypass via cache, unsupported imports/externs |

No line in this table claims the full new Surface Spec, Grammar or v0.4 is implemented by the current VM.

## 4. Historically reported limitations

The earlier VM coverage report listed:

1. Complex/recursive closure captures: incomplete or known to fail in TIR lowering.
2. `match` guards: not supported by the then-reported VM lowering.
3. Nested patterns: not supported by the then-reported VM lowering.
4. String iteration with `for`: not supported by the then-reported VM lowering.
5. Generic methods: only simple fully resolved/concrete instances supported.
6. CLI arguments to `main`: not passed by that VM runner.
7. HIR recursive closure tests: reported stack overflow on some cases (`closure_tests.tmpr`); this is a reference issue, not proof of VM failure.

**These limitations are unverified against the active branch**; don't mechanically carry them forward as still-open defects without checking implementation. Conversely, don't mark them fixed because later language specifications describe them.

## 5. Memory/ownership v0.4 mandatory gate

The compiler/VM must demonstrate:

| Gate | Positive and negative evidence required | Status |
|---|---|---|
| By-value copy | Independent mutations for copyable composites; no hidden identity sharing | Normative pending |
| `consume` | Successful transfer; compile error on later source use | Normative pending |
| Affine/nonduplicable resource | Exactly one owner, no implicit clone or accidental second cleanup | Normative pending |
| `ref` and `inout` | Valid shared reads; exclusive mutable access; branch/loop loan-end analysis | Normative pending |
| Slice and Range | Non-owning read-only slice, range `[start,end)` bounds and owner lifetime | Normative pending |
| Borrowed temporary aggregates | Allowed local use and rejected escape/suspension | Normative pending |
| Closure capture | Explicit `ref`/`inout`/`consume`, escape constraints and `fn[self]` acyclicity | Normative pending |
| Resource cleanup | Normal return, `?`/error propagation and cancellation after safe completion | Normative pending |
| Async/actors | Dynamic borrow not live over `.await`; actor isolation; handles deferred post NO-CYCLE | Future feature/gate |
| `@deny(alloc)` | Conservative static effect facts; no backend-sensitive elision loophole | Normative pending |
| FFI | Foreign raw pointers not granted safe provenance without verified wrapper | Deferred boundary |

These rows **are not a statement of failed tests**. They are required evidence with no active-branch result available in this review. Future Tempra Native AOT is bound to the same memory semantics.

## 6. Effects and result flow

`@deny` and `@pure` must be enforced on the supported source compilation path, including transitively required operations and emitted specializations. Cache hits must represent code previously compiled under equivalent semantics. Paths unable to enforce a contract must reject explicitly rather than execute unchecked.

`Result` must not silently disappear on the supported source surface; `?` must preserve propagation and cleanup. Comptime-pruned branches do not contribute runtime effects; mere optimisation hopes do not remove required capabilities.

## 7. Promotion gate: HIR → Tempra Script default VM

Before removing the default HIR runtime for general scripting, record and pass:

- Positive/negative differential fixtures for *every declared supported language construct*, including source/module graph and stdlib, equality of output/exit code and controlled diagnostics.
- Full numeric/error behaviour, specialization identity, Result propagation and deterministic compilation.
- Fresh compilation/cache hit/stale artifact/corrupt artifact consistency; no effects or ownership bypass through cache.
- Compatibility with the accepted Surface/Grammar and v0.4 memory semantics for the **shipped subset**; unsupported constructs produce reliable errors.
- Cold process launch, warm cache and memory baselines with the K5 protocol; no unmeasured performance claims.
- Clear CLI `run`/`check`/REPL policy; no hidden fall-through to HIR after a contract failure.
- Exact commit, test commands, host/target, exit codes, counts and failing tests in the release ledger.

Passing this gate does not imply actors, zero-RC native code, Level 4 reproducibility, Wasm emission, or full formal proof. Actors remain gated by M5; Tempra Native LLVM AOT is later.

## 8. Tests and reporting template

For each feature, store:

```text
Feature:
Source fixture(s):
Source compiler + options:
Commit and dirty state:
Host/target:
HIR expected/reference behavior:
VM actual value/stdout/stderr/exit:
TIR lowering / bytecode verifier outcome:
Cache cold/hit/stale/corrupt outcomes:
Ownership/effect negative fixtures:
Result: VERIFIED | PARTIAL | UNSUPPORTED | FAILED
```

Do not use `VERIFIED` without a recorded run. Refer to [VM_PERF_BASELINE.md](VM_PERF_BASELINE.md) for measurable performance and [roadmap/k5.md](roadmap/k5.md) for timing methodology.

## 9. Next reconciliation

The actual branch `feature/k5-compiler-timings` was not available from the remote during this documentation pass. This is a **pre-PR status ledger**; reconcile every historical assertion with the branch's parser/typechecker, TIR lowerer, VM, tests and CLI before treating it as authoritative execution coverage.
