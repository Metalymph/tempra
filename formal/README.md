# Tempra formal verification workspace

The directory is intentionally tool-neutral until qualification of a
proof assistant and a Rust-verification workflow. There are **no
machine-checked Tempra soundness proofs here yet**.

- `manifest.toml`: declared proof obligations and honest status.
- `obligations/result.md`: first R.PROOF questions and semantic boundary.
- `docs/roadmap/formal-verification.md`: proof policy and qualification.

No `lake`, Rocq, Verus or custom verification tooling is introduced into
the regular Cargo build. Version-pin tools once selected and run proofs
in dedicated CI jobs.
