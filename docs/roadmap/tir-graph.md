# TIR Graph — versioned semantic inspection

**Target CLI:** `tempra graph` (exact subcommand/flags and schema version TBD).
This is static compiler inspection, not a live program profiler.

## Queries

- Functions, blocks, instructions, type/ownership summaries and source links.
- CFG edges, call graph, dependency edges and specialization provenance.
- Capability/effect witnesses, `@deny` violations and their call path.
- Result production/handling/forwarding, joined origins and error flow.
- Backtracking from diagnostics to original HIR/source spans for developers,
  tools, IDE and agents. No second semantic IR or duplicate effect analyzer.

## Representation contract

TIR/semantic facts remain compiler authoritative. A lowerer-owned source map
links `TirLocation` to module identity and valid original source span. Graph
nodes carry stable **within-compilation** IDs, with explicit semantic/schema
revision and source identity. Cross-version stable identifiers are **not**
claimed until designed and tested. Prefer deterministic first-request / ordered
ID allocation over HashMap iteration order.

Machine-readable JSON is the initial format; optional DOT/Graphviz is a later
export. Define pagination/limits, query errors and redaction for sensitive
paths/data; MCP must reuse this output and must not expose host files by default.

## Acceptance

Versioned query schema with golden fixtures and determinism across processes,
original working directories and equivalent source roots; no program execution
for static queries; source mapping valid for supported TIR instructions;
capability and Result witnesses match existing compiler diagnostics; unknown
or HIR-only operations return explicit *unavailable* responses rather than
invented nodes or facts. VM/TIR coverage gaps remain visible.

Graph results do not by themselves constitute a mathematical proof; they are
structured evidence that can be used by `.PROOF` work and external tools.
