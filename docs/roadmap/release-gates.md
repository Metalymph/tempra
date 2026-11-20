# Tempra Script — Release Gates

**Revision:** 2026-11-16. These gates define the promised supported surface; they do not certify that any release has shipped.

No Tempra Script v1.0 alpha/beta should be dispatched without a coherent install-to-edit-to-execution workflow, deterministic diagnostics and reproducible evidence. Tempra Native is a later LLVM AOT distribution, not a dependency of this release.

| Gate | Alpha requirement | Beta expansion |
| --- | --- | --- |
| **Runtime** | Verified stack VM is default for declared scripting subset; no hidden HIR fallback after errors | Broader end-to-end parity and stronger regression evidence |
| **Memory** | Explicitly disclose current Memory v0.4 conformance limits; reject unsupported ownership forms | Expand M/M5 evidence; no unearned cycle/borrow safety claims |
| **Effects / Result** | Enforced on shipped supported execution paths; no silent Result discard | Broader native/stdlib, module and diagnostic coverage |
| **Stdlib** | File/path/process/strings/collections and documented failures on the declared subset | More APIs; tested cross-platform behavior |
| **CLI / REPL** | `run`, `check`, `repl`, exit codes, diagnostics | Cache, package and interactive-state reliability |
| **Formatter** | Deterministic, idempotent, comments and `--check` | Stress tests, LSP edit agreement |
| **Linter** | Stable baseline diagnostics and actionable fixes | Configurable and expanded rules |
| **LSP** | Completion, hover, definitions, references, rename, formatting, diagnostics | Large-workspace and latency hardening |
| **MCP / agent workflow** | Compiler-backed inspection and controlled actions | Agent evaluation and retrieval-quality baselines |
| **Distribution** | macOS/Linux/Windows install/uninstall smoke; relocatable stdlib | Repeatable packaging and upgrade path |
| **Documentation** | Language/stdlib references and executable examples | Migration and compatibility policy |
| **Benchmarks** | K5 environment and raw measurements published | K6 budgets; reproducible CPython/Node/Bun/TypeScript comparisons |
| **Formal evidence** | Claimed scope and open obligations disclosed | Additional model/implementation-linked work where feasible |

## Mandatory nonclaims

- Existing crate scaffolds do **not** prove an AOT compiler, a general VM-supported source surface, actor safety or an RC-free memory implementation.
- The VM may not silently hide missing support behind a second runtime.
- `comptime` VM migration must pass semantic and deterministic-budget parity before replacing the historical kernel.
- Bytecode caches are untrusted until version/fingerprint/source checks and verification; the format is not a stable ABI.
- No historical JIT/Cranelift feature may be used as a Tempra Script product promise.
- Any release states its exact supported syntax, libraries, targets and test/build commit.

## Evidence to retain

Record exact commit, working-tree state, toolchain, target OS/architecture, native and bytecode versions, commands, fixtures, stdout/stderr/exit behavior, relevant environment, failures, and benchmarks with raw samples. A release is blocked by an unverified hard gate even if the marketing site or editor tooling is ready.

See the [Script plan](script.md), [roadmap status](status.md), [K5 protocol](k5.md), and [VM coverage](../VM_COVERAGE.md).
