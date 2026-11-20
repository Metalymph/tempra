# Tempra Script VM — Performance Baseline and K5 Measurement Protocol v0.2

**Status:** historical data plus reproducible measurement protocol; **not** a current benchmark report.
**Source baseline:** GitHub `main`, historical Phase 14.15–14.20 VM work and [roadmap/k5.md](roadmap/k5.md).
**Memory authority:** [Tempra Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md).
**Target runtime:** Tempra Script stack VM without JIT. Tempra Native LLVM AOT is a separate future benchmark subject.

## 1. Scope and comparison rules

Measure whole-process scripting usability separately from VM execution efficiency. Cold compiler startup, cache reuse, instruction dispatch, memory consumption, code size and developer responsiveness are distinct metrics. No optimization invalidates memory semantics, controlled arithmetic, `@deny`/`@pure` or Result propagation.

**Do not infer November 2026 performance from old Phase 14 samples.** No current timing run was performed for this documentation update. The active local `feature/k5-compiler-timings` branch must be measured before publishing results.

JIT/Cranelift phases and their speedups are **historical**. Their code is designated for removal, not for further tuning. Historical design/benchmarks belong in `docs/archive/` if preservation is desired. Retain VM-only work such as peephole passes, stack/frame tuning and superinstructions, subject to updated tests.

## 2. Historical Phase 14.15 sample (not a contemporary result)

The prior document reported averages over **20 iterations following 2 warmups**, using `crates/tempra-driver/tests/perf_baseline.rs`. It did **not** provide a reproducible full machine/profile/source fingerprint for these samples, so figures below are historical context only.

| Case | HIR avg | VM miss avg | VM hit avg | VM exec | BC size | Instructions | Functions | Cache file |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `arithmetic_loop_1000` | 2.27 ms | 5.86 ms | 4.17 ms | 3.57 ms | 25 KB | 3072 | 1 | 25 KB |
| `for_expr_collect_1000` | 1.54 ms | 15.13 ms | 14.13 ms | 13.98 ms | 25 KB | 3086 | 1 | 25 KB |
| `struct_access_1000` | 3.90 ms | 7.54 ms | 5.81 ms | 5.09 ms | 25 KB | 3087 | 1 | 25 KB |
| `string_basics_repeat` | 73 µs | 259 µs | 97 µs | 58 µs | 826 B | 118 | 1 | 834 B |

Columns reproduce the original source even where they are not strictly consistent (for example `BC size` versus `Cache file` for the string case). The difference is **not** silently corrected or explained without raw artifacts.

### 2.1 Historical Phase 14 VM-only tuning (not independently rerun)

The previous report described these optimizations and provisional micro-benchmark observations:

| Phase | VM-only change | Historical observation |
|---|---|---|
| 14.16 | Peephole elimination of redundant temporary `StoreLocal/LoadLocal` | ~15–25% fewer instructions; ~5–10% runtime improvement in microbenchmarks |
| 14.17 | Frame/call-stack and operand-stack preallocation; direct local initialization | ~10–15% runtime reduction in reported cases |
| 14.18 | `AddI64LL` and `AddI64LC` superinstructions | ~10–12% execution-time reduction in arithmetic-heavy cases |
| 14.19 | Optional call/instruction/backedge profiling | Low disabled-path overhead reported |
| 14.20 | Opcode histograms and per-function profiles | Diagnosed load/store density and superinstruction use |

These percentages are not automatically additive, do not establish today's throughput, and may not apply after memory-model changes. Remeasure under K5.

## 3. Canonical K5 measurement contract

Record **before** measurements:

- Full git commit, dirty state and branch; compiler build/version/profile, semantic revision and bytecode format.
- OS, architecture, CPU, memory, host/target, binary install path, tool versions and environment variables that affect execution.
- Complete fixture source/input hash and module/stdlib resolution; configuration, runtime selection, cache root and warmed/cold state.
- Warmup count, sample count, sample order, method for median and tails, stdout/stderr digest and exit code.
- Exact measurement command and raw artifacts (CSV/JSON/text); CPU governor/power configuration where relevant.

**K5 measures distinctly:** process-start-to-exit wall time, compiler stage timings, cold/warm VM cache path, VM execution-only time, CPU, peak RSS and (when measured) editor/check/LSP responsiveness. Filesystem-page-cache coldness, absent Tempra cache, compiler-process cold start and warm in-process VM are **different experimental conditions**; label them separately.

The initial `scripts/k5_bench.py` is documented as measuring **new-process wall time only**. It does not by itself measure peak RSS, compiler stages, or physical cold filesystem cache. Its harness self-test is not a Tempra benchmark.

### 3.1 Recommended benchmark matrix

| Mode | Required boundaries | Primary reported metric |
|---|---|---|
| HIR reference (historical only) | Same source semantics, no VM | End-to-end and execution-only for differential context |
| VM cold compiler / absent artifact | Start, parse, typecheck, TIR, checks, lower, verify, write, run | Wall time and per-stage breakdown |
| VM compatible cache hit | Start, read, validate, verify, run | Wall time, cache time, execution time |
| VM in-process repeated execution | Already verified module, known inputs | Execution latency and instruction counters |
| VM stale/corrupt cache | Incompatible/invalid artifact followed by safe rebuild | Failure containment, rebuild latency |
| CLI check/edit workflow | Only documented frontend/analysis steps | Check and responsiveness latency |

Do not compare process startup for one runtime with only execution time for another. If `cargo run` is used, distinguish Cargo build overhead from installed-binary startup.

## 4. Compiler phase timings

The planned K5.2 instrumented pipeline reports lexer, parser, AST/HIR, typechecker including `comptime`, TIR lowering, verification, effect analysis/contracts, bytecode lowering, bytecode verification and cache read/write. The selection/pruning of compile-time branches must not be double-counted as both compiler and VM execution work without clear labeling.

Timers should not alter semantic behavior or create hidden inputs that change deterministic compilation. Profiling allocation itself should be measured or excluded consistently when interpreting `@deny(alloc)` cases.

## 5. Memory and correctness before performance

v0.4 makes value semantics, `consume` invalidation, resource cleanup, nonescaping borrows and `@deny(alloc)` normative. Optimizations cannot rely on unsafe alias sharing, omit a required copy solely to make an invalid contract pass, or leak owners during early returns.

Track at least:

- Peak RSS and allocation counts, clearly distinguished from VM value-copy counts.
- Repeated Array/String copies, Slice creation/lifetime, closure capture, and Resource transfer where implemented.
- Error paths (`Result` propagation, overflow, bounds errors), verifier-rejected artifacts and cold-cache failures.
- Impact of nonduplicable types and borrow provenance enforcement when implemented.
- Determinism of generated bytecode and successful cache reuse under the K2.5 bounded reproducibility guarantee.

No claim of ARC NO-CYCLE completion, zero runtime reference counting in Tempra Native, or actor concurrency performance is made by this file.

## 6. VM optimization backlog (JIT-free)

The following are **candidates**, not promised wins:

1. Reduce redundant temporary loads/stores while preserving verification and ownership events.
2. Retune local/operand-stack and frame capacity using representative workloads.
3. Profile superinstructions and adjust only after counting actual opcode distribution.
4. Reduce verifier/cache work safely without skipping semantic trust checks.
5. Investigate stack scheduling and typed dispatch only with reliable K5/K6 regressions.
6. Benchmark allocation hot spots and buffer operations under value semantics independently of AOT elimination.

Use K6 to establish regression budgets **after** K5 publishes a reproducible baseline. Avoid replacing a sound verifier with an unverified fast path.

## 7. Comparative language benchmarking

For public comparisons, pin the exact versions of CPython, Node.js, Bun and any TypeScript compiler/transpiler. Document whether source is TypeScript or JavaScript, whether transpilation is included, and whether runtime cold startup is included.

Compare equivalent output, input size, error handling and observable work first. Run stable identical fixtures and validate output hashes; report throughput/latency separately from code complexity or developer experience. Do not invent rankings or describe the historical Phase 14 VM numbers as current Python/Node/Bun results.

## 8. Reporting template

```text
Benchmark / source hash:
Commit / dirty state / semantic revision:
Host / OS / CPU / RAM / compiler profile:
Runtime path:
Command / tool versions:
Cache condition (absent / valid / stale / corrupt):
Warmups / samples:
Correct result hash, stdout/stderr, exit:
Median / p95 / min / max / spread:
Peak RSS / CPU (if collected):
Compiler phases (if collected):
Raw artifact path / reproducibility notes:
```

Record separate time units consistently (µs/ms/s), preserve raw samples, and prefer medians and dispersion over standalone rounded means.

## 9. Next review and historical references

Before the final PR, compare this file with the active K5 implementation and the **actual tests** in the branch. Decide whether the historical Phase 14 sample table remains useful here or is better moved to `docs/archive/performance/` once a current measured K5 baseline exists.

The old Phase 15–17 Cranelift performance narrative is deliberately excluded from the active VM performance plan. Its technical history should remain available under `docs/archive/jit/`, not blended with Tempra Script's future benchmark path.

**Principle:** performance measurements are evidence; optimizations never redefine semantics.
