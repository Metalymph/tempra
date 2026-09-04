# Tempra Development Guide

This guide explains the end-to-end process of adding a new feature to the Tempra language.

## High-Level Architecture

Tempra follows a traditional compiler pipeline:
1. **Lexer** (`tempra-lexer`): Source string → Tokens.
2. **Parser** (`tempra-parser`): Tokens → AST (`tempra-ast`).
3. **Lowering** (`tempra-hir`): AST → HIR.
4. **Type Checker** (`tempra-typeck`): HIR → HIR (with type validation).
5. **Interpreter** (`tempra-interpreter`): HIR evaluation.

This is the default execution path. The experimental VM continues from checked HIR through `tempra-tir-lower`, TIR verification, bytecode lowering, bytecode verification, and `tempra-vm`. MIR, native AOT, and dedicated Wasm output are not part of the default path.

---

## 1. Adding a New Keyword or Symbol

### Step 1.1: Lexer
Add the new token to `crates/tempra-lexer/src/token.rs`.
```rust
pub enum TokenKind {
    // ...
    MyKeyword,
    // ...
}
```
Update `crates/tempra-lexer/src/lexer.rs` to recognize the string or symbol.

### Step 1.2: Parser
Update `crates/tempra-ast/src/lib.rs` to add a new node if necessary.
Update `crates/tempra-parser/src/lib.rs` to parse the new construct. Use `peek()` and `consume()` to handle tokens.

---

## 2. Implementing Logic (HIR & Typeck)

### Step 2.1: HIR
Add the new node to `crates/tempra-hir/src/hir.rs`.
Implement the conversion from AST to HIR in `crates/tempra-hir/src/lower.rs`.

### Step 2.2: Type Checker
Define validation rules in `crates/tempra-typeck/src/check_expr.rs` or `check_stmt.rs`.
Ensure errors are reported via `ctx.error(msg, span)`.

---

## 3. Runtime Implementation (Interpreter)

Update `crates/tempra-interpreter/src/eval_expr.rs` or `eval_stmt.rs`.
- Handle the new HIR variant in the `match` arms.
- Return `Result<ControlFlow, String>`.
- For variables, use `env.get()`, `env.insert()`, or `env.update()`.

---

## 4. Testing

### 4.1 Unit & Integration Tests
Each core crate has a `tests/` directory.
- `crates/tempra-typeck/tests/typeck_tests.rs`: Use `expect_ok()` or `expect_err()`.
- `crates/tempra-interpreter/tests/interpreter_tests.rs`: Use `assert_runs()` and compare against `Value` variants.

Commands:
```bash
# Run all tests
cargo test

# Run specific integration tests
cargo test --test typeck_tests
cargo test --test interpreter_tests
```

### 4.2 Manual Verification
Create a file in `examples/feature_name.tempra` and run it:
```bash
cargo run -- run examples/feature_name.tempra
```

---

## Useful Information

- **Spans**: Always preserve `Span` information for better diagnostics.
- **Value Semantics**: Tempra is value-oriented. Use `Rc` inside `Value` (like in `Value::Array`) for shared storage in slices.
- **Collection Helpers**: The v0.1 Array/Slice helpers are eager. They lower through the existing iterator protocol, `Option`, loops, slots, and `ArrayPush`; do not add lazy adapter hierarchy, new TIR instructions, or VM opcodes for these helpers.
- **Control Flow**: The interpreter uses `ControlFlow` to handle `Return`, `Break`, `Continue`, `Exit`, and `Crash`. Ensure these are propagated correctly.
- **Lexical Scoping**: Use `env.push_scope()` and `env.pop_scope()` in blocks and loops.

---

## 5. Module System & Resolution

Tempra employs a filesystem-driven, dot-separated module system.
- **Resolver**: The dependency graph is resolved via Depth-First Search (DFS) in `crates/tempra-driver/src/module_graph.rs`.
- **Cyclic Imports**: Cyclic dependencies are explicitly forbidden and detected during resolution.
- **Visibility**: Items are private by default; `pub` visibility modifier is required for cross-module access.
- **Type Checking Namespace Qualification**: During type checking, namespaces and types from other modules are resolved dynamically via the dot-separated prefix matching (`TypeContext::is_module`).
- **Relative Imports**: Imports starting with a dot (e.g. `import .helper`) are resolved relative to the directory of the current source file.

---

## 6. Native Bridge Architecture

The Tempra standard library relies on a native bridge to communicate with Rust implementations.
- **Signatures**: Extern functions are declared in `.tmpr` files inside the `/std` directory (e.g., `pub extern fn exists(path: String) -> Bool`).
- **Rust Implementation**: The `tempra-native` crate provides the actual logic for these standard library functions.
- **VM Extern Dispatch**: During execution, the VM resolves `extern` calls by routing them to the native registry, which dispatches the call to the corresponding Rust function without leaving the VM cleanly.

---

## 7. Adding Semantic Analyses

Cross-cutting semantic analyses consume verified TIR and publish reusable per-function facts. E1 established this pattern in `tempra-effects`, which still depends only on `tempra-tir`. E2 integrates it in fresh VM compilations via immutable `VmCompilation`; cache hits, TIR emission/interpreter and HIR paths remain independent. See the [pipeline boundaries](architecture/PIPELINE.md).

- Do not add a parallel HIR capability analyzer to compensate for unsupported lowering.
- Keep analysis results separate from `TirModule` and bytecode serialization.
- Retain structural provenance using function IDs, block IDs, instruction indices, and call edges.
- Treat unknown extern declarations as harmless until called; a called unclassified extern is an analysis error.
- Keep source spans and rendered diagnostics in later integration layers.
