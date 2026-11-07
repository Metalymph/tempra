# Tempra coding-agent guidance

Tempra is experimental. Do not invent syntax, stdlib symbols, compiler
flags or runtime guarantees; verify against the current compiler and
`docs/TEMPRA_SURFACE_SPEC_v0.1.md`, `docs/GRAMMAR.md`, `docs/STDLIB.md`.

- Use `just check`, `just test` and `just tooling-check` as appropriate.
- Use `just roadmap-check` when editing roadmap or proof status.
- Do not replace the planned stack-VM default with a generic HIR fallback
  or embed a Wasm engine. Experimental JIT is frozen pending product scope.
- Treat `Result` obligations and capability contracts as semantic rules,
  not style warnings. Diagnostic locations and deterministic order matter.
- Before publishing examples, execute them with the exact documented
  Tempra version and compare output with reference tests.
- Do not claim an `.PROOF` complete from passing tests or model-only proof.
- Give existing APIs and semantics precedence over external model guesses.

Agent resources/evals will live under `agents/` when they are implemented.
