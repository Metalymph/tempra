# Unified REPL and optional Shell profile

**Status:** design requirements. Existing REPL uses the HIR interpreter;
`tempra repl --shell` and `tempra shell` are not yet exposed CLI modes.

## Entrypoints

- `tempra repl`: canonical Tempra syntax.
- `tempra repl --shell`: identical session and VM plus interactive shell frontend.
- `tempra shell`: alias for the previous mode, not a second product/runtime.

One session engine owns incremental typed definitions, bytecode fragments,
values, module graph and environment overlay. Keep committed state independent
from failed typecheck/execution transactions; never replay whole history.

## REPL acceptance

Persistent values/functions and modules, multiline editing, undo/correction
policy, history, completion, syntax/semantic diagnostics, result rendering,
and incremental cached compilation. Result rendering is the REPL's presentation
layer, **not** an implicit Tempra `print` statement that discharges an effect or
silently handles a pending Result. Define Result visibility and session lifetime.

## Shell frontend

Command elision is an interactive syntax **only within the explicitly active
Shell profile**. An interactive input containing `for`, `if` or function bodies
may include command forms accepted by a documented, deterministic shell-input
parser. The parser lowers these forms to typed canonical `Command` operations
with source locations and effect facts. It must not introduce a macro system or
silently permit those forms in persisted `.tmpr` scripts; show/offer canonical
Tempra equivalents for saving scripts.

Input disambiguation must not rely on an ad hoc 'try parsing Tempra, else
execute it as a shell command' fallback, which can execute mistyped code.
Specify precedence, escaping, quoting, interpolation, pipes and redirects.
Always pass argument vectors structurally; no default `sh -c` or `cmd /c` for
interpolated strings. Nonzero exits and pipeline failures require an explicit
Result/exit-status policy, not a silent acceptance of errors.

## Session and OS responsibilities

Persist `cwd`, environment overlay, foreground/background jobs and process
status; implement `cd`, `pwd`, Ctrl+C, Ctrl+Z, suspension/resume and TTY
integration where supported. Use abstract OS process/job APIs with distinct
Unix process-group/terminal semantics and Windows process/console support.
Document unsupported controls rather than pretending POSIX parity.

Completion unifies Tempra symbols, process executables, filesystem paths and
current command context. Job control belongs to REPL runtime services, not
another general-purpose shell/interpreter. No promise of full Bash/Zsh/POSIX
compatibility.

## Release gates

**Alpha:** usable VM-backed REPL, diagnostics, history/multiline/completion and
explicitly documented Shell-preview subset if exposed; baseline fmt/lint/LSP/MCP
remain hard gates. **Beta:** broader shell parsing, subprocess/result handling,
terminal integration and cross-platform stress. **Full PRE-AOT Tempra Script exit:**
Shell mode offers documented job control on supported platforms, TTY and signals
where supported, with cross-platform fallback behavior and tested security
boundaries. Do not market a stub alias as feature complete.
