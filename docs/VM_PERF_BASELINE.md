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

### Phase 15.3 — JIT Int64 Calls v1
Expanded JIT support to allow static calls between eligible Int64 functions.
- **Dependency Compilation**: The VM automatically resolves and pre-compiles `Call` instruction targets, maintaining the JIT-cache without VM-to-JIT callbacks.
- **Recursive Safety**: Cleanly rejects cyclical call graphs to prevent compilation loops.
- **Diagnostics**: Rejection reasons (e.g. nested calls to string functions) correctly propagate up the call chain for full profile visibility.

### Phase 15.2 — JIT Observability & Guardrails
Added performance tracking and safe fallbacks for the JIT integration.
- **Metrics**: `VmProfile` now tracks `jit_compile_attempts`, `jit_compile_successes`, `jit_runtime_calls`, and `jit_fallback_calls`.
- **Diagnostics**: `JitEligibility` provides clear reasons when a function cannot be compiled natively.
- **Rejection Caching**: The VM caches rejection reasons to avoid redundant compilation attempts of unsupported functions.
- **Verification**: Fully tested via driver integration to ensure accurate JIT statistics without semantic changes.

### Phase 15.1 — JIT/VM Integration Hook
Integrated JIT into the VM dispatch loop.
- **Opt-in**: JIT is disabled by default, ensuring stability.
- **Transparent Fallback**: Unsupported functions or compilation failures revert to the interpreter.
- **Hotness Driven**: JIT triggers based on `hot_call_threshold` and `hot_backedge_threshold`.
- **Verified**: Integration tests confirm 100% result parity between JIT and interpreter.

### Phase 15.0 — Cranelift JIT Prototype
Introduced `tempra-jit-cranelift`, a prototype JIT compiler supporting hot `Int64` functions. 

**Observations:**
- Preliminary tests show native execution of simple arithmetic loops is significantly faster than interpreted execution.
- Successful lowering of stack-based bytecode to Cranelift SSA.
- Ready for future integration with VM hot function profiling.

### Phase 14.20 — VM Profiling & Trace Visibility
Extended profiling to include global and per-function opcode execution counts. Added `VmProfile::summary()` for human-readable execution reports.

**Observations:**
- Opcode distribution reveals high density of `LoadLocal/StoreLocal`, suggesting further stack-to-register optimization potential.
- Superinstructions like `AddI64LC` are clearly visible in the trace, confirming the effectiveness of the peephole optimizer.
- Minimal overhead confirmed for the disabled path.

### Phase 14.19 — Hot Function Counters
Added opt-in profiling infrastructure to track call counts, instruction counts, and loop backedges. This allows the VM to identify "hot" functions and loops based on configurable thresholds.

**Observations:**
- Minimal overhead when disabled (verified via performance baseline).
- Provides actionable metrics for future JIT/Cranelift integration.
- Correctness verified via dedicated profiling tests.

### Phase 14.18 — Superinstructions v1
Introduced conservative superinstructions `AddI64LL` (Local + Local) and `AddI64LC` (Local + Constant) to reduce dispatch overhead.

**Observations:**
- Further performance improvement: ~10% to 12% reduction in execution time for arithmetic-heavy code.
- Instruction count reduced in hot loops.
- Peephole optimizer enhanced with multi-instruction fusion patterns to identify these candidates.

### Phase 14.17 — VM Stack/Frame Tuning
Optimized VM runtime by pre-allocating call stack and operand stacks based on `max_stack` metadata. Refined the function call path to avoid temporary allocations.

**Observations:**
- Measurable performance improvement: ~10% to 15% reduction in execution time.
- Fewer allocations during hot loops (especially for function calls).
- Cleaner instruction dispatch with reduced borrow churn.

### Phase 14.16 — Peephole Lowering v1
Added a conservative post-pass peephole optimizer that eliminates redundant `StoreLocal/LoadLocal` pairs for single-use temporary variables.

**Observations:**
- Reduction in instruction counts: ~15% to 25% for simple expressions and string metadata access.
- VM Execution time improved slightly (~5-10% in micro-benchmarks).
- Verifier overhead remains negligible.

## Lowering Optimizations

The TIR-to-Bytecode lowerer includes a peephole optimization pass:
1. **Conservative Peephole**: Identifies redundant `StoreLocal/LoadLocal` sequences for temporary locals.
2. **Jump Fixups**: Automatically adjusts jump offsets when instructions are removed.
3. **Safety Verification**: Each optimized function is re-verified by the bytecode verifier; if verification fails, the unoptimized version is used.
4. **Direct Local Initialization**: Function calls pop arguments directly into the callee's local slots, bypassing intermediate vector allocations.
5. **Efficient Dispatch**: The instruction loop minimizes redundant call stack lookups and employs inlined stack helpers.
6. **Superinstructions**: High-frequency instruction sequences (like `LoadLocal` + `AddI64`) are fused into single opcodes to reduce dispatch overhead.
7. **Frame Allocation**: Reuse or pre-allocate call frames to reduce heap pressure.
8. **Cranelift JIT**: Direct machine code generation for hot functions.

## Caveats

- The VM is currently unoptimized and focuses on correctness and safety (hardening).
- The baseline includes all hardening checks (verifier, runtime bounds checks).
- Inlining is not yet implemented in the TIR lowerer, which impacts method call performance.
