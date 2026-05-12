# VM Performance Baseline (Phase 14.15)

This document records the performance characteristics of the experimental Tempra VM (stack-based v0) compared to the reference HIR interpreter.

## Methodology

Benchmarks are executed via a dedicated performance harness in `crates/tempra-driver/tests/perf_baseline.rs`.

- **HIR Avg**: Average execution time using the reference HIR interpreter.
- **VM Miss Avg**: Average time for a full cold run (Compile + Cache Write + Execute).
- **VM Hit Avg**: Average time for a warm run (Cache Read + Verify + Execute).
- **BC Size**: Approximate size of the bytecode artifact (including headers).
- **Instrs / Fns**: Count of bytecode instructions and functions generated.

Measurements are averaged over 20 iterations after 2 warmup runs.

## Baseline Results (Sample)

| Case | HIR Avg | VM Miss Avg | VM Hit Avg | VM Exec | BC Size | Instrs | Fns | Cache File |
|------|---------|-------------|------------|---------|---------|--------|-----|------------|
| arithmetic_loop_1000 | 2.27ms  | 5.86ms      | 4.17ms     | 3.57ms  | 25 KB   | 3072   | 1   | 25 KB      |
| for_expr_collect_1000| 1.54ms  | 15.13ms     | 14.13ms    | 13.98ms | 25 KB   | 3086   | 1   | 25 KB      |
| struct_access_1000   | 3.90ms  | 7.54ms      | 5.81ms     | 5.09ms  | 25 KB   | 3087   | 1   | 25 KB      |
| string_basics_repeat | 73µs    | 259µs       | 97µs       | 58µs    | 826 B   | 118    | 1   | 834 B      |

*Note: Results vary significantly depending on hardware and system load.*

## Candidates for Optimization

### Phase 14.16 — Peephole Lowering v1
Added a conservative post-pass peephole optimizer that eliminates redundant `StoreLocal/LoadLocal` pairs for single-use temporary variables.

**Observations:**
- Reduction in instruction counts: ~15% to 25% for simple expressions and string metadata access.
- VM Execution time improved slightly (~5-10% in micro-benchmarks).
- Verifier overhead remains negligible.

### Lowering Optimizations

The TIR-to-Bytecode lowerer includes a peephole optimization pass:
1. **Conservative Peephole**: Identifies redundant `StoreLocal/LoadLocal` sequences for temporary locals.
2. **Jump Fixups**: Automatically adjusts jump offsets when instructions are removed.
3. **Safety Verification**: Each optimized function is re-verified by the bytecode verifier; if verification fails, the unoptimized version is used.
4. **Interpreter Dispatch**: Current `match` based dispatch in the VM is a bottleneck.
5. **Stack Caching**: Frequent push/pop operations can be reduced by caching the top of the stack in registers.
6. **Frame Allocation**: Reuse or pre-allocate call frames to reduce heap pressure.
7. **Cranelift JIT**: Direct machine code generation for hot functions.

## Caveats

- The VM is currently unoptimized and focuses on correctness and safety (hardening).
- The baseline includes all hardening checks (verifier, runtime bounds checks).
- Inlining is not yet implemented in the TIR lowerer, which impacts method call performance.
