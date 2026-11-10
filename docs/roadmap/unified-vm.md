# Unified non-AOT VM and staged `comptime` migration

**Target:** one stack bytecode VM for scripts, tests, REPL, Shell profile and
`comptime`; **not** a VM written in Tempra (self-hosting is a later program).

## Current vs target

Current default `run` and REPL use the HIR interpreter; the TIR/bytecode VM is
experimental and narrower. K1 established the HIR evaluator as the canonical
`comptime` semantics for the presently supported surface. The TIR interpreter
also exists for testing/validation. Treat these as authoritative *current facts*,
not extra runtimes to preserve in the final product.

## Transition gates

| Gate | Deliverable | Required evidence |
| --- | --- | --- |
| UV1 | VM coverage and typed TIR mapping for script language | Feature matrix, differential success/error diagnostics, cache tests |
| UV2 | Single VM runtime entrypoint for `run` / tests / REPL | No hidden HIR fallback, verified bytecode, effect enforcement |
| UV3 | Staged typed fragments for compiler evaluation | Dependency graph for type-required values, deterministic incremental lowering |
| UV4 | VM comptime execution context | Fuel/call-depth, no forbidden host effects, isolation and state rollback |
| UV5 | Old/new comptime differential parity | Same results/errors/side effects, modules, generics, nested evaluation, cache |
| UV6 | Remove both general-purpose interpreter implementations | `cargo tree`/feature audit, no runtime refs, entire workspace gate |

## Breaking the compiler staging cycle

Sometimes the typechecker needs a computed `comptime` value before it can finish
typing a declaration. A fully typed whole-module TIR cannot be a prerequisite
for that value. Use a dependency-directed staged pipeline:

1. Identify a compile-time expression's free names, available types, and
   module/specialization dependencies.
2. Lower only a **sufficiently typed dependency closure** to verified TIR,
   preserving stable names, facts, locations and deterministic order.
3. Lower that TIR to verified bytecode; execute it with a VM configured for
   `comptime` and explicitly restricted capabilities, fuel and state.
4. Reify checked results into typed compiler constants, resolve dependent
   declarations, and continue regular typechecking/lowering.
5. Diagnose cycles, unavailable types or unsupported operations explicitly;
   never manufacture a type/value to make the pipeline continue.

Ensure pure compiler constant folding never grows into a competing general
interpreter. Validate source span provenance and Result/ownership semantics
through staged evaluation. Reproducibility and caches must include all relevant
compiler/semantic inputs. No `comptime` source may silently gain filesystem,
process or nondeterministic privileges from the runtime VM's broader host API.

## JIT removal order

Audit `tempra-jit-cranelift`, `tempra-backend-cranelift`, CLI experimental flags,
Cargo features, `justfile` jobs, integration tests and docs. Separate the
**JIT implementation** from historical native backend scaffolds; no blanket
delete before impact analysis. Remove JIT code/dependencies and revise coverage
in a dedicated PR; keep the available VM execution path valid throughout.

## Acceptance

One runtime authority for executable non-AOT bytecode; no general HIR/TIR
interpreter required by normal execution or `comptime`; parity tests on every
supported source construct; no unintended capability escalation, lifetime
change or Result discard; cold start, memory and performance measured against K5.

`VM.PROOF` shall specify bytecode validity, sound transfer through staged TIR
and the relation between compilation-time and runtime VM states, without
advertising unproved implementation properties.
