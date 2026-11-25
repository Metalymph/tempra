# Tempra Script — Scripting-first Product Plan

**Revision:** 2026-11-16
**Status:** target product sequence, not an alpha release announcement.

## Goal and positioning

Tempra Script is the lightweight, statically checked scripting distribution of **Tempra**. It should be practical for small scripts, automation, CLI tools and day-to-day development without requiring LLVM, a JIT, a bundled Wasm engine or a native compilation toolchain for ordinary execution.

The target runtime is a **verified stack-bytecode VM**, with bytecode caching and controlled failures. The historical default HIR interpreter remains an implementation/reference path until VM coverage and diagnostic parity justify switching the default. It must not become a permanent second general scripting runtime.

Tempra Script and Tempra Native share one language and one [memory specification](../Tempra_Memory_Ownership_v0.4.md). Script's intended owning-value implementation is **ARC NO-CYCLE**; the absence of general safe strong-reference cycles and sound borrowing must be validated before a safety claim is shipped. Native's future LLVM AOT representation may differ physically while preserving the same observable semantics.

## S1 — Scripting achievement

### Runtime and language coverage

Establish an explicit, end-to-end supported subset: top-level scripts, functions, modules/imports, generics, `comptime`, `Option`/`Result`, pattern matching, `for`, closures, collections, capabilities and documented value semantics. Acceptance requires source-to-VM execution, not just parser or isolated opcode support.

Switch default execution only after [VM coverage](../VM_COVERAGE.md) and [release gates](release-gates.md) demonstrate parity on supported programs, errors and diagnostics. Unsupported lowering must fail with a controlled diagnostic, never silently fall back to HIR. The existing HIR-based `comptime` kernel remains authoritative until the [VM migration parity gate](../COMPTIME_SPEC.md) is explicitly passed.

### Standard library

Prioritize filesystem/path/stream I/O; process/exit/environment; UTF-8 strings, bytes and collections; structured data/JSON; time; and practical scripting utilities. Each public API needs a documented failure model, platform constraints, effect classification and tests. An API listed here is **not** automatically implemented.

### CLI and module workflow

Provide script execution, checking, REPL, reliable module/import resolution, package/dependency workflow, deterministic diagnostics and trustworthy bytecode cache. Installed stdlib discovery must be explicit and relocatable. Source/typecheck-only commands must not imply that all capability or memory contracts have been certified.

### Safety and performance

Do not silently discard `Result` obligations on the supported surface. Preserve deterministic compiler output and rejected capability violations. Measure process startup, peak RSS, compiler phases and cache hits/misses; do not equate old microbenchmarks with validated product performance. K5 captures baselines, K6 defines regression budgets.

Actors are **not** a first-alpha prerequisite. Their implementation and safety claims remain blocked by M5. Tempra Native AOT is likewise not a Script runtime prerequisite.

## S2 — Public website and documentation

The planned site uses the Metalymph Astro Tone fork when implementation begins. Required content: installation and removal, release version, language reference, stdlib/API stability, error handling, unsupported features, examples that execute in CI, troubleshooting, and transparent comparisons with Python, JavaScript and TypeScript.

No benchmark figures or “faster than” claims without equivalent workloads, pinned tools and published raw measurements.

## S3 — Developer experience (hard pre-alpha gate)

- **Formatter:** deterministic, idempotent, comment-preserving, check mode, LSP edit parity.
- **Linter:** useful, stable diagnostics with explainable severities and fixes; no duplicate typechecker.
- **LSP:** diagnostics, completion, hover, definition, references, rename, formatting and code actions on a declared subset.
- **MCP:** compiler-backed read-only inspection/resources by default; controlled file mutation/execution, reproducible schemas and bounded host access.
- **Agent workflows:** examples, commands and editor integration tested against real supported Tempra programs.

Target CLI verbs (not an implementation claim): `tempra run`, `check`, `fmt`, `lint`, `repl`, `lsp`, `mcp`.

## S4 — Release gates

**Tempra Script v1.0-alpha** requires representative scripts running on the default VM; verified module/stdlib resolution and installation smoke on macOS/Linux/Windows; K5 raw baseline; safe controlled failures; documentation and example tests; and usable formatter, linter, LSP and MCP workflows for the declared scope.

**Tempra Script v1.0-beta** adds wider tested coverage, K6 responsiveness budgets, package and cache compatibility guidance, cross-platform stability, security-boundary checks, repeatable distribution upgrades and reproducible language comparisons.

See [release-gates.md](release-gates.md). A release must explicitly disclose gaps. Neither alpha nor beta implies actor safety, RC-free native execution, a stable bytecode ABI, or a formal proof of the whole language.

## Follow-on — Tempra Native

After Script's product and semantic maturity, implement an LLVM-based ahead-of-time backend with ownership-aware code generation, native ABI, linking, standalone executables, effect enforcement, parity and measured performance. Native is not a separate language or a JIT mode. See [migration.md](migration.md) and the A milestone in [roadmap.md](../roadmap.md).
