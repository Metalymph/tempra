# Roadmap status snapshot

This is a planning snapshot, not a CI attestation or a release certificate.
Update it only after recording the exact test commands and git commit.

| Workstream | Status | Outstanding gate |
| --- | --- | --- |
| Historical K4 specialized effects | Implemented on documented bounded surface | Formal proof not claimed |
| R1–R3 Result flow/provenance | **Implementation gate passed (user-run tests)** | Full workspace gate, R4/R5 and R.PROOF not closed |
| K5 compiler latency baseline | **NEXT: K5.1** | Measurement contract + actual data |
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
