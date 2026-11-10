# Roadmap status snapshot

This is a planning snapshot, not a CI attestation or a release certificate.
Update it only after recording the exact test commands and git commit.

| Workstream | Status | Outstanding gate |
| --- | --- | --- |
| Historical K4 specialized effects | Implemented on documented bounded surface | Formal proof not claimed |
| R1–R3 Result flow/provenance | **Merged; user-reported workspace gate green** | R4/R5 and R.PROOF not closed |
| K5 compiler latency baseline | **K5.1 harness validated; K5.2 next** | Actual baseline data and phase instrumentation |
| K6 responsiveness budgets | Planned | K5 baseline |
| S1 Ergal scripting | In progress | VM coverage, stdlib, CLI/distribution |
| S2 Website (Astro Tone) | Planned | Real public docs and install flow |
| S3 Tooling and AI workflows | Planned gate before alpha | Formatter, linter, LSP, MCP, agent evaluation |
| S4 Ergal v1.0 alpha / beta | Not released | All release gates |
| R.PROOF | Draft/not proved | Tool qualification, formal semantics, proof and Rust linkage |
| Nimonic LLVM AOT | Future | Ergal scripting release maturity |
| Self-hosting | Future | Validated Ergal and stable language semantics |

The HIR interpreter is currently the default execution path in the
historical architecture documentation; making the stack VM the normal
Ergal runtime is a **target**, not a completed migration. The existing
Cranelift JIT remains experimental/frozen after Result hardening.

## PRE-AOT implementation truth (2026-11-10)

The Result R1–R3 implementation tests and full workspace test gate were
reported green and the branch was merged before this planning update. This
is evidence for that tested surface, not R4/R5 closure or `R.PROOF` proof.
K5.1's measurement harness self-test passed; **actual Tempra performance
baseline not yet published**, and K5.2 timing instrumentation is next.

**Implemented / existing:** HIR script execution, experimental stack VM/TIR,
HIR-driven REPL, basic stdlib/native bridge, effect analysis, type checking,
Result analyzer and current structured diagnostics/LSP baseline.

**Planned / not represented as shipped:** unified VM for `comptime`, JIT removal,
retirement of general interpreters, Shell command elision and job management,
Script 2.0 expression-body functions, versioned TIR Graph, unified scripting
prelude, full formatter/linter/MCP and release distribution coverage.
