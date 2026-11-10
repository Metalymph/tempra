# Ergal release gates

No Ergal v1.0-alpha/beta release should be dispatched without a coherent
editing-to-execution workflow and installable distribution.

| Gate | Alpha | Beta |
| --- | --- | --- |
| Runtime | VM default for declared scripting subset; no hidden interpreter fallback | Broader VM parity and regression budgets |
| Stdlib | File/path/process/strings/collections + documented failure model | Broader scripting APIs, cross-platform behavior verified |
| CLI / REPL | run/check/repl/diagnostics and exit codes | Cache/package reliability and compatibility |
| Formatter | Deterministic, idempotent, comments and --check | Stress and editor edit consistency |
| Linter | Stable baseline rules, actionable diagnostics | Configuration and broader rule coverage |
| LSP | Completion/hover/definition/references/rename/formatting/diagnostics | Performance and larger workspace robustness |
| MCP / agents | Compiler-backed resources/tools, controlled execution, supported schemas | Agent evals and docs retrieval quality baselines |
| Distribution | macOS/Linux/Windows install smoke + relocatable stdlib | Repeatable packaging and upgrade path |
| Documentation | Language + stdlib reference, recipes, examples tested | Compatibility policy and migration docs |
| Benchmarks | K5 environment and raw data published | K6 budgets; Python/Node/Bun/TS comparisons reproducible |
| Proof | Scope/status disclosed, no false formal claims | Further `.PROOF` milestones where feasible; not a substitute for release tests |

Do not label *experimental* crates as production-grade because they
exist in Cargo workspace. Release notes must list unsupported surfaces.

## Additional PRE-AOT gate: unified Script 2.0 distribution

The alpha/beta table above is retained. Do not label unfinished Shell/job
control or graph queries as shipping features in those releases. Before full
PRE-AOT Ergal distribution and the start of main Nimonic AOT work:

- No JIT, no general-purpose HIR or TIR interpreter in the shipped engine;
  `run`, REPL, Shell profile, tests and `comptime` use the same verified
  bytecode VM with their respective capability budgets and session policies.
- Complete the staged comptime differential/parity gates, no hidden fallback,
  Result/ownership/effects consistency, reproducible cache and diagnostics.
- Stable scripting prelude/top-level behavior, expression-body functions and
  usable documented I/O/process/Path/collection APIs.
- REPL with incremental committed state; Shell mode documented on supported
  platforms with command elision, pipes/redirects, TTY, signals and job control
  including explicit limitations on Windows vs Unix.
- TIR Graph with versioned deterministic JSON, query bounds, source mapping
  and compiler-sourced effects/Result provenance.
- Packaging macOS/Linux/Windows; process startup, RSS, binary size and
  dependency footprint measured; Python/JS/TS comparisons reproducible.
- No formal-proof claim beyond the actually discharged `.PROOF` scope.

`tempra repl --shell` and `tempra shell` are aliases to one REPL service,
not separate language runtimes. See [REPL/Shell](repl-shell.md).
