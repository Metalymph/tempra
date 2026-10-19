# Tempra

> **From scripts to software. Without switching languages.**

Tempra is a statically typed, script-first programming language designed to grow
from small scripts into efficient native software without switching languages.
It combines a compact, expression-oriented syntax with static type checking,
explicit error handling, compile-time evaluation, and capability contracts.

**Status: Experimental — actively developed.** Tempra is not production-ready.
The language, CLI, and internal APIs may change.

Native AOT output is a core design goal, not a current feature. Today Tempra
runs on its default HIR interpreter, with an opt-in bytecode VM and Cranelift
JIT available for experimentation.

---

## Why Tempra?

- **One language from start to finish.** Begin with top-level statements, add
  types and a `main` entry point when you need them, and follow the same
  language toward future native builds.
- **Static types without ceremony.** Types are checked across variables,
  functions, generics, and interfaces, with inference where it helps and no
  implicit coercions.
- **Failure is part of the type system.** There are no exceptions. `Option`,
  `Result`, `?`, and exhaustive `match` make failure visible and local.
- **Effects belong in signatures.** `@deny(...)` and `@pure` let functions
  declare the capabilities they do not accept, with tooling that explains
  violations.
- **Values by default.** `let` is immutable, `var` is explicit, and `inout`
  marks mutation at call sites. Arrays and slices are values; slices are views
  over shared storage.
- **A small, coherent surface.** One loop form (`for`), expressions everywhere,
  structural static interfaces, and a deliberately compact standard library.

---

## A Taste of Tempra

Run any example with `tempra run <file>`.

### A script with no boilerplate

```tempra
let languages = ["Rust", "Swift", "Go"]

for language in languages {
    print(language)
}
```

Top-level code executes directly in script mode; a `main` function is optional.

### Types and pattern matching

```tempra
struct Language {
    name: String
    year: Int
}

enum Status {
    Active
    Retired
}

fn describe(lang: Language, status: Status) -> String {
    let state = match status {
        Active => "active"
        Retired => "retired"
    }
    return lang.name + " (" + state + ")"
}

let languages = [
    Language { name: "Rust", year: 2010 },
    Language { name: "Swift", year: 2014 },
]

for lang in languages {
    print(describe(lang, Active))
}
```

Structs and enums are core data types, and `if` and `match` are expressions.

### Errors as values

```tempra
fn read_config(path: String) -> String! {
    let text = fs.read(path)?
    return Ok(text)
}

fn main()! {
    let config = read_config("examples/sample.txt")?
    print(config)
}
```

`String!` is shorthand for a fallible result, and `?` propagates an error to the
caller. A `main` function can itself be fallible.

---

## Design Philosophy

Tempra aims to keep the language small while allowing the compiler to become
more capable underneath it.

- **Script first, not script only.** Direct feedback for small programs, with a
  typed path to larger applications and future native backends.
- **Explicit over implicit.** Mutation, fallibility, conversions, and effects
  are visible in source code.
- **Types as documentation.** Static checks catch mistakes early without
  requiring a heavy type-system vocabulary.
- **One obvious construct per job.** `for` covers iteration, `if` and `match`
  produce values, and errors are ordinary values.
- **Safety before optimization.** Memory-model guarantees come before aggressive
  ARC optimization or concurrency.
- **Tooling is part of the language.** Structured diagnostics, symbol
  information, capability inspection, and LSP support are first-class concerns.

---

## Current Capabilities

### Language

- Static type checking for bindings, functions, methods, generics, and
  structural interfaces.
- `struct`, `enum`, tuples, `Option`, `Result`, arrays, and slices.
- Pattern matching with exhaustiveness checking, guards, struct/tuple patterns,
  and `is` narrowing.
- `if`, `match`, and blocks as expressions; `for` as a statement or expression.
- Named functions, explicit-capture closures, generic specialization, `inout`
  parameters, methods, and `extend` blocks.
- UTF-8 strings where `.len()` counts Unicode scalar values and `.size()` counts
  bytes; string indexing and slicing are intentionally unsupported.
- Value-oriented collections with eager helpers such as `map`, `filter`, `find`,
  `reduce`, `fold`, `enumerate`, `zip`, `take`, `skip`, and `chain`.
- Checked `Int64` arithmetic: overflow and division by zero produce controlled
  errors instead of wrapping or trapping.

### Explicit errors

There is no exception system. Fallible operations return `Result`, optional
values use `Option`, and `?` propagates errors. `match` is exhaustive for
`Bool`, enums, `Option`, and `Result`.

### Comptime

`comptime` expressions and blocks are evaluated during type checking and
materialized as constants. Tempra also supports `comptime` parameters,
`if comptime`, `comptime match`, and type values in generic code. Evaluation is
deterministic, bounded by evaluator fuel and a call-depth limit, and rejects
host effects such as I/O.

Full specialization and runtime erasure for every comptime form remain under
development.

### Capability contracts

Functions can declare capability contracts with `@deny(...)` and `@pure`:

```tempra
@pure
pub fn add(a: Int, b: Int) -> Int {
    return a + b
}
```

`@pure` means `@deny(io, process, spawn, nondeterminism)`; pure code may still
allocate or fail with a controlled panic. Contracts are checked transitively
through calls.

Capability enforcement currently runs during fresh compilation on the
experimental VM path and can be inspected with `tempra effects`. The default HIR
interpreter, REPL, and test runner reject annotated programs rather than execute
them without enforcement. `tempra check` validates frontend types only; it does
not certify capability contracts.

### Runtime and tooling

- Default HIR interpreter with script mode and an interactive REPL.
- Experimental TIR and bytecode VM with verified bytecode and a compilation
  cache.
- Opt-in Cranelift JIT for eligible `Int64` functions and a read-only heap
  subset.
- CLI commands for running, checking, testing, diagnostics, symbols, effects,
  and TIR emission.
- LSP support for diagnostics, completion, hover, definitions, document symbols,
  semantic tokens, references, highlights, and rename.
- MVP native standard library: `std.fs`, `std.path`, `std.strings`, and
  `std.io`, plus the collection helpers above. Module coverage is still growing
  and differs between the default interpreter and experimental VM.

---

## Getting Started

### Prerequisites

- A stable Rust toolchain (see `rust-toolchain.toml`).
- `just` is optional; the equivalent Cargo commands work without it.

### Build and run

```bash
cargo build --release -p tempra-cli
./target/release/tempra run examples/hello.tmpr
```

Output:

```text
hello
```

For a development build:

```bash
cargo run -p tempra-cli -- run examples/hello.tmpr
```

To run the test suite:

```bash
cargo test --workspace
```

or, with `just`:

```bash
just test
```

---

## CLI Overview

The examples below use `tempra` for brevity. From a local build, substitute
`./target/release/tempra` or `./target/debug/tempra`.

```text
tempra run <file> [args...]        Execute with the default HIR interpreter
tempra check <file> [--json]       Parse and type-check
tempra diagnostics <file> [--json] Emit structured diagnostics
tempra symbols <file> [--json]     Print a structural symbol outline
tempra effects <file> [--json]     Inspect capability facts from fresh VM compilation
tempra test <file>                 Run a script and its expect assertions
tempra repl                        Start the interactive REPL
tempra emit-tir <file>             Print TIR for supported programs (experimental)
```

`check`, `diagnostics`, `symbols`, and `effects` accept `--json`. `emit-tir`
works only for programs supported by the current TIR lowering.

### Experimental VM and JIT

The bytecode VM is opt-in and requires a `main` function. It supports a smaller
language subset than the default interpreter.

```bash
tempra run --experimental-vm examples/jit_heap/string_len.tmpr
```

The Cranelift JIT must be compiled in and enabled on top of the VM:

```bash
cargo build -p tempra-cli --features jit-cranelift
tempra run --experimental-vm --experimental-jit examples/jit_heap/string_len.tmpr
```

The JIT targets hot `Int64` functions and a read-only heap subset, including
`String.len`, `String.size`, `Array.len`, and `Array.get_i64`. Functions outside
that subset run in the VM. `--experimental-jit` requires `--experimental-vm` and
a JIT-enabled build.

---

## Compiler Architecture

Tempra currently has two execution paths. The default path favors language
coverage and direct execution; the experimental path adds typed backend IR,
verification, and bytecode.

```text
Default interpreter
source → lexer/parser → AST → HIR → type checking (and comptime) → HIR interpreter

Experimental VM
source → lexer/parser → AST → HIR → type checking (and comptime)
       → TIR lowering → TIR verification → capability analysis
       → contract enforcement → bytecode lowering → bytecode verification
       → VM → optional Cranelift JIT
```

- **HIR** is the typed high-level representation interpreted by default.
- **TIR** is the strongly typed, verifiable backend-facing representation used
  by the experimental VM and capability analysis.
- **Bytecode** is verified before execution and cached for repeat runs.
- **Capability analysis** computes direct and transitive capability requirements
  over verified TIR.

The workspace is split into focused crates: frontend (`tempra-lexer`,
`tempra-parser`, `tempra-ast`), semantic layers (`tempra-hir`,
`tempra-typeck`), execution (`tempra-interpreter`, `tempra-tir*`,
`tempra-bytecode*`, `tempra-vm`, `tempra-jit-cranelift`), native bindings
(`tempra-native`, `std/`), and tooling (`tempra-driver`, `tempra-diagnostics`,
`tempra-analysis`, `tempra-lsp`, `tempra-cli`, `tempra-repl`).

MIR, native AOT, complete Wasm emission, the linker, and related backend crates
contain scaffolds or prototypes. They are not part of a working output pipeline
today.

---

## Development Status

### Implemented

- Lexer, parser, AST, HIR lowering, type checker, and default HIR interpreter.
- Script mode without `main`, plus app mode with a `main` entry point.
- Core language surface: structs, enums, tuples, `Option`, `Result`, arrays,
  slices, pattern matching, generics, structural interfaces, closures, methods,
  and `inout`.
- Checked `Int64` arithmetic.
- Comptime core: expressions, blocks, parameters, `if comptime`, and
  `comptime match`.
- MVP standard library and eager collection helpers.
- CLI, structured diagnostics, symbol extraction, REPL, and LSP tooling.

### Experimental

- TIR lowering, verification, and interpretation.
- Bytecode lowering, verification, VM execution, and the bytecode cache; the VM
  requires `main` and supports a narrower language subset.
- Capability analysis, `@deny`/`@pure` enforcement, and `tempra effects` on the
  fresh VM compilation path.
- Cranelift JIT for eligible `Int64` functions and a read-only heap subset.
- `tempra emit-tir`.
- Wasm, native backend, MIR/IR, optimization, and linker crates in scaffold or
  prototype form.

### Planned

- Native AOT compilation.
- A complete Wasm backend and Wasm hosting path.
- Pluggable actor concurrency.
- `@verified` functions with stronger static guarantees.
- A complete acyclic ownership model and ARC-cycle prevention.
- Advanced ARC optimizations such as escape analysis, stack promotion, and
  retain/release elimination.
- Full comptime specialization and broader TIR coverage.
- Package management, incremental builds, and deeper compiler performance work.

---

## Roadmap Overview

The current focus is comptime consistency and runtime parity across execution
paths. The remaining language work follows a deliberate dependency order:

1. Result and value-use hardening.
2. Acyclic ownership and complete memory-model guarantees.
3. Pluggable actor concurrency.
4. Verified functions (`@verified`).
5. Deeper memory optimization.
6. Faster compilation and incremental builds.
7. A conditional unsafe boundary, only if a real low-level primitive requires it.

Native AOT, Wasm output, package management, and a richer TIR graph are parallel
engineering tracks. See the [roadmap](docs/roadmap.md) for details.

---

## Documentation

- [Surface syntax specification](docs/TEMPRA_SURFACE_SPEC_v0.1.md)
- [Grammar](docs/GRAMMAR.md)
- [Comptime specification](docs/COMPTIME_SPEC.md)
- [Capability contracts](docs/specs/effects-v0.1.md)
- [Architecture](docs/architecture/ARCHITECTURE.md)
- [Compiler pipeline](docs/architecture/PIPELINE.md)
- [Standard library](docs/STDLIB.md)
- [TIR specification](docs/TIR_SPEC.md)
- [Bytecode VM](docs/BYTECODE_VM.md)
- [Cranelift JIT](docs/CRANELIFT_JIT.md)
- [Tooling and diagnostics](docs/TOOLING.md)
- [Developer guide](docs/DEV_GUIDE.md)
- [Roadmap](docs/roadmap.md)

The `examples/` directory contains runnable programs covering scripts, types,
collections, errors, and experimental JIT cases.

---

## Contributing and License

Contributions, bug reports, and language-design discussions are welcome. Tempra
is experimental, so expect breaking changes and open an issue before starting a
large change.

Before submitting a pull request, run the workspace tests and formatting check:

```bash
cargo fmt --all -- --check
cargo test --workspace
```

The `justfile` contains additional workspace automation such as `just test`,
`just check`, and `just fmt`.

Tempra is licensed under the Apache License 2.0. See [LICENSE](LICENSE) for the
full text.
