# JIT Burn-in & Stability Validation

This document describes the stability validation corpus and stress tests for the Cranelift JIT prototype (Phase 15.7).

## Objective

The goal of the burn-in phase is to ensure the JIT implementation is stable, handles complex control flow correctly, and gracefully falls back to the interpreter when encountering unsupported features.

## Burn-in Corpus

The following scripts are located in `examples/jit_burnin/`:

| Script | Coverage | Expected Behavior |
|---|---|---|
| `int64_call_chain.tmpr` | Deep function calls | Helper functions JIT-compiled; `main` rejected (Unit local), runs via VM |
| `int64_nested_loops.tmpr` | Multi-level loops | Native backedge handling in `inner_loop`; `main` rejected |
| `int64_branch_heavy.tmpr` | Complex `if/else` | `classify` compiled and executed natively |
| `jit_burnin_bool_logic_chain` (inline) | `&&`, `\|\|`, `!` | Result parity; function rejected by JIT v1 |
| `jit_burnin_deep_nested_branches` (inline) | 4+ levels of `if/else` | Stable block termination and join; native execution |
| `int64_mixed_fallback.tmpr` | Strings in eligible fn | Caller rejected, executes via VM |
| `int64_rejection_struct.tmpr` | Struct usage | Explicit rejection (eligibility check) |
| `int64_rejection_string.tmpr` | String usage | `main` rejected (Unit local); `process_string` accepted via read-only heap subset |

## Differential Regression Gate

The differential gate pins HIR/VM/JIT semantic parity for the supported shared
subset. It reuses the existing suites rather than introducing a new framework.

### Standard Gate (no JIT feature)

```bash
cargo test -p tempra-driver --test differential_tests
cargo test --workspace
```

`differential_tests.rs` compares normalized HIR and experimental VM results for
values and asserts checked `Int64` failures by semantic category.

### JIT Gate

```bash
cargo test -p tempra-driver --test jit_burnin_tests --features jit-cranelift
cargo test -p tempra-driver --features jit-cranelift
cargo test --workspace --features jit-cranelift
```

The JIT gate runs the burn-in corpus plus `jit_integration_tests`,
`heap_jit_source_tests`, `entry_result_tests`, `multi_module_vm_tests` and
`tir_correctness_tests` with Cranelift enabled. It asserts:

- HIR reference parity with the VM/JIT result.
- Native compilation *and* native execution for JIT-required programs
  (`jit_compile_successes > 0` and `jit_runtime_calls > 0`).
- Native runtime failures propagate without falling back, while pre-execution
  rejections execute safely in the interpreter.

## Supported vs Unsupported Backends

| Subset | Evidence |
|---|---|
| `Int64` arithmetic, control flow, calls | `jit_compile_successes > 0` and `jit_runtime_calls > 0` |
| Read-only heap (`String`, `[Int64]`) reads | `heap_jit_source_tests`, `jit_heap_runtime_calls > 0` |
| Structs, `Bool` ABI, heap mutation, closures | `jit_is_permanently_rejected`, `jit_runtime_calls == 0` |
| Reachable explicit `Unreachable` | Rejected by JIT v1 (diagnostic) |

## Known Diagnostic Limitations

- Checked `Int64` failures use different error shapes per backend. HIR reports
  `RuntimeError("integer overflow")` / `RuntimeError("integer division by zero")`;
  the VM reports `VmError("Int64Arithmetic(Overflow)")` /
  `VmError("Int64Arithmetic(DivisionByZero)")`. The gate compares the semantic
  category, not the raw strings.
- `Unreachable` terminators that remain reachable after TIR lowering are not
  accepted by JIT v1; genuinely dead joins are pruned during TIR lowering.
- Unsupported functions are permanently rejected and run through the
  interpreter; caller functions may also be rejected transitively.
- `int64_rejection_string.tmpr`'s `process_string` is accepted by the read-only
  heap subset; the corpus entry's rejection assertion is satisfied by its `main`
  function (Unit local).

## How to Run

### Automatic Tests

Run the dedicated driver test suite:

```bash
just jit-burnin
```

This executes `crates/tempra-driver/tests/jit_burnin_tests.rs`, which validates:
- Correctness (result comparison with HIR reference).
- JIT compilation success for eligible functions.
- Proper rejection/fallback for unsupported features.
- Profile summary transparency.

### Manual Verification

You can run individual scripts using the CLI:

```bash
cargo run --features jit-cranelift -- run examples/jit_burnin/int64_call_chain.tmpr --experimental-vm --experimental-jit
```

## Stress Testing

The suite includes:
1. **Repeated Execution**: Running the same JIT-enabled script 20+ times to ensure no memory corruption or state leakage.
2. **Warm-run Validation**: Verifying that once a function is compiled, it is reused for subsequent calls without re-compilation.
3. **Differential Micro-tests**: A collection of small, edge-case programs to ensure semantic parity between JIT and VM.

## What is NOT covered

- **New features**: This phase does NOT expand the JIT subset.
- **Production readiness**: JIT is still strictly opt-in and experimental.
- **Struct values and heap mutation**: Struct fields, mutable heap writes and
  closures remain unsupported in JIT-compiled code. Read-only heap reads
  (`String`, `[Int64]`) are supported by the heap-aware subset — see
  [JIT_HEAP_BOUNDARY.md](JIT_HEAP_BOUNDARY.md).
