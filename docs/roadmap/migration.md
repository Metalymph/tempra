# Milestone identity and migration map

This file prevents collisions between historical compiler milestone IDs and the
scripting-first release program. `docs/roadmap.md` remains the roadmap entrypoint.
Do not silently renumber completed work or rewrite historical status as a proof.

## Canonical IDs (existing roadmap)

| Canonical ID | Meaning | Status / handling |
| --- | --- | --- |
| K4 | Specialized-effect semantics and enforcement | Historical **implemented** track; **not** Result Hardening |
| K5 | Phase timings and latency baseline | Next active measurement milestone |
| K6 | Responsiveness regression budgets | After K5 measurements |
| R | Result/value-use hardening | R1–R3 implementation gate test-green on supported scope; R4/R5 remain separately open |
| M / M5 | Acyclic ownership and memory stabilization | Required safety gate before actor concurrency implementation |
| C | Structured concurrency / actors | Planned; implementation gated by M5 |
| A | Native AOT | Deferred until scripting release maturity; target LLVM (new decision) |
| V | Source-level `@verified` functions | Deferred; separate from roadmap `.PROOF` obligations |

Earlier conversations called the Result work **"K4 Result/Error Hardening"**.
Its canonical repository identity is **R1–R3**, *not* a second K4.
The corresponding formal obligation is **R.PROOF**; any `K4-result`
reference is only a historical alias. The original `K4` keeps its
specialized-effect meaning and may have its own `K4.PROOF` in the future.

## Status language

- **IMPLEMENTATION_GATE_PASS**: tests/audits passed for the declared scope;
  no mathematical proof is implied.
- **DRAFT / SPECIFIED**: proof obligation not yet proved.
- **MODEL_PROVEN**: theorem checked in the selected formal tool,
  for a clearly named model and assumptions.
- **IMPLEMENTATION_LINKED**: the implementation-to-model relation has
  separately been justified or proved.
- **FORMALLY_VERIFIED**: all declared proof obligations for the stated
  scope are discharged; never apply to a broader language surface.

R1–R3 implementation changes have been merged following a user-reported
workspace-green gate; this does not close R4/R5 or establish formal proof.
In particular, R4 error compatibility, R5 backend/API parity and aggregate
completeness must not be marked finished by inference from R3 tests.

## Roadmap hierarchy

`S1`–`S4` are *product gates*, not replacements for K/R/M/C/A.
A phase's formal obligations use `<canonical-id>.PROOF` when there is a
meaningful general property to establish. Performance measurements,
websites and packaging do not receive decorative proof milestones.

`docs/roadmap/status.md` records the implementation gates; this document
records identity and migration; `formal/manifest.toml` tracks formal
obligations independently of both.

## 2026-11-10 PRE-AOT clarification

No new K IDs are assigned to Script 2.0, VM unification, REPL/Shell,
TIR Graph or dependency reduction: see [PRE-AOT](pre-aot.md).
'K5.1/K5.2 = Result hardening' in parallel design notes is **not canonical**:
K5.1 is measurement contract and K5.2 is phase timing; Result belongs to R.
K4 always denotes specialized effects in this repository. M/M5 is the ARC
acyclic memory correctness gate preceding C; O is advanced memory optimization.
The future LLVM/Nimonic decision replaces the earlier planned Cranelift AOT
backend only **prospectively**, without rewriting historical phase records.
