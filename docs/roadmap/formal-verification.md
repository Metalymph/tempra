# Incremental formal specifications and `.PROOF` milestones

A test suite, even one with 100% passing tests, is not a soundness proof.
The proof specification must define the *property*, assumptions,
supported Tempra subset, semantic model and trusted components.

## Rules

1. Attach `<canonical milestone>.PROOF` where a universal property is
   meaningful (R.PROOF, M.PROOF, E.PROOF, VM.PROOF, ...).
2. Distinguish **model proved** from **Rust implementation linked**.
   A sound Lean/Rocq model does not prove a Rust compiler implements it.
3. Version the source syntax, typing rules, operational semantics and
   abstract interpretation domains. Do not infer new rules from tests.
4. No theorem with uninspected axioms, `sorry`/`admit` or trusted external
   assumptions may be advertised as fully proved.
5. Preserve proof assets for future self-hosted compiler work, but prove
   a new compiler-to-model relation for the Tempra implementation.

## First obligations

**R.PROOF.1:** semantics of Result creation, alias, handling, propagation,
control-flow merge and repeated dynamic instances.
**R.PROOF.2:** prove abstract transfer/join/fixed-point soundness and
safe acceptance against silent-drop property for explicitly scoped core.
**R.PROOF.3:** establish that the relevant Rust analyzer implements the
model (or a verified certificate checker) for that core.

Memory: M.PROOF (acyclic ownership, borrow exclusivity, reclamation).
Effects: E.PROOF (sound capability enforcement on shipped paths).
VM: VM.PROOF (verified bytecode, execution preservation).
These are not claimed completed.

## Tool qualification

Compare one independent proof assistant (Lean 4 or Rocq), one Rust
verifier (Verus or Creusot), and optionally hax/Aeneas on a *small*
finite-domain Result-flow model. Measure proof scope, assumptions,
Rust refactoring, CI setup, toolchain size, maintainability and reuse
under self-hosting. Do **not** start with a custom verifier.

`formal/manifest.toml` is a status registry, not a machine-checked proof.
