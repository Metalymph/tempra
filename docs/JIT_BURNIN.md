# JIT Burn-in & Stability Validation

This document describes the stability validation corpus and stress tests for the Cranelift JIT prototype (Phase 15.7).

## Objective

The goal of the burn-in phase is to ensure the JIT implementation is stable, handles complex control flow correctly, and gracefully falls back to the interpreter when encountering unsupported features.

## Burn-in Corpus

The following scripts are located in `examples/jit_burnin/`:

| Script | Coverage | Expected Behavior |
|---|---|---|
| `int64_call_chain.tmpr` | Deep function calls | All levels JIT-compiled |
| `int64_nested_loops.tmpr` | Multi-level loops | Efficient native backedge handling |
| `int64_branch_heavy.tmpr` | Complex `if/else` | Correct SSA branch translation |
| `int64_mixed_fallback.tmpr` | Strings in eligible fn | Caller rejected, executes via VM |
| `int64_rejection_struct.tmpr` | Struct usage | Explicit rejection (eligibility check) |
| `int64_rejection_string.tmpr` | String usage | Explicit rejection (eligibility check) |

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
- **Heap values**: Strings, arrays, and structs remain unsupported in JIT-compiled code.
