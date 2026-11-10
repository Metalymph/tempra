# PRE-AOT integration — Tempra Ergal Script 2.0

**Decision date:** 2026-11-10. **Status:** adopted direction / pending implementation.

This planning integration extends [the canonical roadmap](../roadmap.md), [Ergal](ergal.md),
[release gates](release-gates.md), [formal work](formal-verification.md), and
[milestone IDs](migration.md). It does **not** supersede historical records or normative
language specifications. Work that exists only as a scaffold remains experimental.

## Invariants

- One Tempra language with one type system, value semantics, Result rules and effect contracts.
- One **stack-based bytecode VM** for all non-AOT execution: scripts, REPL, interactive
  Shell profile, tests and eventual `comptime`. The compile-time context differs in
  allowed effects, fuel, access to state and staging; there is no second general VM.
- A central **typed, verified TIR** with provenance and typed semantic facts. Source
  lowering may be staged for `comptime` but uses the same semantics and bytecode VM.
- **No JIT in the Ergal target.** Remove experimental Cranelift JIT safely after
  dependency, feature and regression audits; do not remove a working fallback first.
- Retire the general HIR runtime interpreter and the general TIR interpreter **after**
  compiled VM coverage and compile-time parity gates. A small compiler constant
  folder is not a second general interpreter.
- **LLVM** is the first Nimonic native AOT backend; LLVM-based Wasm is the first
  option to evaluate. Do not ship an embedded Wasm VM or promise Wasm support now.
- No second language dialect, general macro system or Python indentation syntax.
  Canonical `.tmpr` uses `{}` blocks everywhere.
- ARC value ownership is *designed* acyclic; safe no-cycle/`ref`/`inout`/capture rules
  must be implemented and validated in M/M5, with M.PROOF independent. Perceus-style
  reuse, uniqueness and escape optimization belong to O and do not gate correctness.
- Full actor/task/channel implementation remains blocked by M5. Scheduler design may
  proceed earlier. No source-level locks or `async fn` coloring are introduced.

## Canonical identities — no K renumbering

| Identity | Responsibility | Dependency / state |
| --- | --- | --- |
| E / K4 | Implemented effect semantics / specialized-effect track | Existing bounded claims remain |
| R | Result/value-use semantics | R1–R3 tests merged; R4/R5 and R.PROOF independently open |
| K5.1 | Benchmark protocol and self-test harness | Existing; **not** Result hardening |
| K5.2 | Instrumented compiler phase timings | Next implementation task |
| K5.3–5.5 / K6 | Startup/RSS, workloads, measured baselines / regression gates | After measurements |
| M / M5 | Acyclic ARC, ownership, `ref`/`inout`, stabilization | Before actor concurrency |
| C | Structured concurrency, actor and scheduler model | Implementation after M5 |
| O | ARC optimization, reuse, escape, uniqueness | Does not gate M5 correctness |
| P | Compiler optimization, staged VM comptime and responsiveness | K5 evidence + semantic prerequisites |
| S1–S4 | Ergal product gates, site, developer tooling, distribution | Product sequencing, not new K IDs |
| A | Nimonic LLVM AOT | After Ergal scripting maturity |
| H (planning) | Self-hosting | After language/AOT maturity; not a historical canonical K ID |

`K4 Result` and `K5 Result` in earlier conversational notes are historical
misnomers: canonical Result is **R**, and canonical K5 is **measurement**.

## Recommended dependency-constrained sequence

This is a **partial order**, not a promise that every large feature finishes before
unrelated smaller work can begin. Parallel tracks are shown deliberately.

1. **K5.1 complete the real baseline protocol, then K5.2 instrumentation.** The
   initial process-harness self-test is not a Tempra performance baseline.
2. **R4/R5 semantic closure on the declared scope** and VM coverage/per-path
   consistency. R.PROOF begins as specification work, not a release claim.
3. **JIT retirement audit**, then remove runtime path, CLI flag, Cargo features,
   tests/docs and dependencies in controlled commits. Keep VM and comptime tests green.
4. **M1–M5 memory correctness** (including acyclic ARC and captures); only after
   M5 may concurrency claim safe actor/channel/task boundaries.
5. **VM feature parity, staged lowering and unified `comptime`** with the same
   bytecode engine. Run old/new semantic differential gates *before* interpreter
   retirement. This work can overlap M but cannot bypass ownership/effects rules.
6. **Script 2.0 language ergonomics, prelude and stdlib** according to
   [Script 2.0](script-2.md). The top-level experience must work on the VM.
7. **REPL/Shell profile**, [TIR Graph](tir-graph.md), formatter/linter/LSP/MCP
   and agent knowledge packs. Parallelize where their semantic APIs are ready.
8. **Ergal release gates:** alpha requires coherent editing-to-execution tooling;
   beta broadens coverage; full PRE-AOT distribution gates include documented
   interactive Shell/job support and VM-only execution.
9. **Nimonic/LLVM** and then self-hosting, each with explicit semantic/parity gates.

**Dependency reduction** is cross-cutting from the beginning, with measured
changes staged in isolated PRs. See [audit procedure](dependency-reduction.md).

## Authority and migration rules

Current executable behavior is described in the existing architecture and pipeline
files; these target decisions do not retroactively assert that VM, `comptime`, Shell,
or Graph already satisfy them. Historical Cranelift AOT/Wasm design notes are retained
as history but **do not override** the forward target of LLVM and unified VM.

Proof assets follow the `R.PROOF`, `M.PROOF`, `E.PROOF`, `VM.PROOF` registry and
make no formal claim until actual checked evidence and implementation linkage exist.
