# Ergal scripting-first product plan

**Goal:** a small-startup, low-memory, statically checked scripting language
that is usable without invoking LLVM or bundling a Wasm engine.

## S1: Scripting achievement

1. **Semantic coverage:** top-level scripts, modules/imports, functions,
   generics, Option/Result, patterns, `for`, closures, collections,
   `comptime`, contracts/effects and documented value semantics.
2. **Default execution:** move ordinary scripts from the broad HIR
   interpreter to the stack bytecode VM only after supported-surface
   parity and failure/diagnostic checks. No silent fallback after a
   semantic or capability failure. Keep the existing comptime authority
   until the planned TIR-comptime transition passes parity verification.
3. **Scripting stdlib:** file/path/stream I/O; process/exit/environment;
   strings/UTF-8, bytes, collections, structured data/JSON, time and
   practical scripting utilities. Specify each API with failures,
   platform differences and tests; missing APIs are not shipped claims.
4. **CLI/package:** `run`, `check`, REPL, package/module workflow,
   deterministic diagnostics and bytecode cache. Installed stdlib
   discovery must be explicit and relocatable.
5. **Safety & performance:** no silent Result discard on the supported
   surface, deterministic compiler output, process startup and RSS
   baselines, and current effect-contract enforcement on shipped paths.

Actor concurrency is not an alpha prerequisite and cannot be presented
as safe until the mandatory M5 memory gate has passed. AOT/LLVM is not an
Ergal runtime prerequisite. Do not ship an extra general interpreter.

## S2: Public site

`site/` will use the Metalymph Astro Tone fork when implementation begins.
Required content: installation, versioned language reference, stdlib
reference, API stability, error handling, troubleshooting and **honest**
Python/JavaScript/TypeScript vs Tempra comparisons. No placeholder
performance claims or fabricated benchmark figures.

## S3: Developer experience (hard pre-alpha gate)

Formatter: deterministic and idempotent; comments preserved; check mode;
LSP text edits agree with CLI formatting. Linter: useful diagnostics and
severities, no duplicate type-checker, explainable rules and fixes.
LSP: diagnostics, completion, hover, go-to-definition, references,
rename, formatting and code actions on declared supported scope.
MCP: read-only inspection/resources by default, compiler-backed
validation tools, controlled file mutation/execution, reproducible
schemas and no unbounded host access. Agent packs and editor integrations
must be tested against published, executable examples.

Target CLI verbs (not all asserted implemented):
`tempra run`, `check`, `fmt`, `lint`, `repl`, `lsp`, `mcp`.

## S4: Release gates

**Ergal v1.0-alpha:** representative scripts work on default VM;
install/uninstall and stdlib resolution smoke on macOS/Linux/Windows;
K5 baseline recorded; diagnostics, format/lint/LSP/MCP and reference
docs usable; failures are controlled; unsupported syntax documented.

**Ergal v1.0-beta:** regression budgets (K6), broader VM/stdlib
compatibility, package migration story, security boundaries, stable
command output schemas where promised, docs and examples in CI, and
validated scripting comparison results.

Keep release status, supported language subset, and proven claims distinct.
