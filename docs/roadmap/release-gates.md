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
