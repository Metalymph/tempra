# Agent enablement (planned product gate)

The goal is compiler-grounded usage by coding agents, not an assumption
that language models have already been fine-tuned for Tempra.

Future `knowledge/` contains versioned, compiler-tested language and
stdlib references; `skills/` contains workflows; `evals/` contains
tasks that compile/run and check exact outputs and diagnostics.
`tempra-mcp` should expose structured documents and read-only compiler
inspection first; filesystem writes and execution require deliberate
permission boundaries. Maintain an agent success/failure baseline.

Canonical instructions: root `AGENTS.md`. Do not duplicate the parser
or typechecker in the MCP or LSP.
