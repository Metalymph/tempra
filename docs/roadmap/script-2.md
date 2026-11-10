# Tempra Script 2.0 — canonical language ergonomics

**Status:** PRE-AOT requirements, not a second Tempra dialect or shipped feature set.

## Scripting prelude

Provide an explicitly versioned, deterministic import prelude for scripts:
selected `print`/I/O, `Path` and filesystem operations, typed `Command`/`cmd`,
strings and collections. Names reference actual stdlib declarations, never
parallel implementations. Ambiguous names must be diagnosed deterministically;
explicit imports can select between conflicting APIs. Keep the prelude small.

**Auto-import is not auto-permission:** `@deny(...)`/`@pure` capability checks,
Result obligations and ownership apply unchanged. Host-OS availability,
filesystem policy and external executables are not made portable by default.

## Top-level code

Script `.tmpr` code runs without a mandatory `main` on the ordinary VM path.
Preserve normal typing, effects and ownership. Specify top-level `?`, uncaught
fallible results, exit codes and how a failing script maps to a diagnostic.
Existing HIR interpreter support is not evidence of VM parity.

## Single-expression functions

Target grammar, subject to parser/type-checker integration:

```tempra
fn double(x: Int64) = x * 2
fn double_explicit(x: Int64) -> Int64 = x * 2
```

The body uses the existing expression type checking, effects, HIR/TIR and VM,
with no second lowering. Define optional return inference consistently with
existing function APIs, recursion and specialization. Braced functions remain
canonical. No arrow-function alternative, implicit call parentheses, or
indentation-sensitive block form. This form is **planned**, not implemented.

## Portable scripts and subprocesses

`.tmpr` scripts are always canonical Tempra; `tempra run` and an optional shebang
must not depend on Python/Node/Bun/Bash for *interpretation*. External commands
used by the program may of course be required. Use typed `Command` and `cmd(...)`
for argument arrays, environment, cwd and pipeline execution. Never build an
implicit shell command from unsanitized concatenated strings.

## Acceptance gate

- Prelude identity/resolution stable across machines, module graphs and caches.
- Top-level functions, imports, effects, Result handling and errors agree on VM
  and `check`/LSP; no silent fallback to HIR after semantic failure.
- Expression-body function type/effect and source span tests equal braced-body
  equivalents; unsupported syntax fails cleanly.
- Portable scripts run without installed scripting language runtimes.
- End-to-end example/benchmark fixtures cover Python, JS/Node/Bun and TS modes
  with equivalent behavior, pinned versions and explicit build-cost accounting.
