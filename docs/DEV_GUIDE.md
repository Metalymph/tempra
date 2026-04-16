# Fero Development Guide

This guide explains the end-to-end process of adding a new feature to the Fero language.

## High-Level Architecture

Fero follows a traditional compiler pipeline:
1. **Lexer** (`fero-lexer`): Source string → Tokens.
2. **Parser** (`fero-parser`): Tokens → AST (`fero-ast`).
3. **Lowering** (`fero-hir`): AST → HIR.
4. **Type Checker** (`fero-typeck`): HIR → HIR (with type validation).
5. **Interpreter** (`fero-interpreter`): HIR evaluation.

---

## 1. Adding a New Keyword or Symbol

### Step 1.1: Lexer
Add the new token to `crates/fero-lexer/src/token.rs`.
```rust
pub enum TokenKind {
    // ...
    MyKeyword,
    // ...
}
```
Update `crates/fero-lexer/src/lexer.rs` to recognize the string or symbol.

### Step 1.2: Parser
Update `crates/fero-ast/src/lib.rs` to add a new node if necessary.
Update `crates/fero-parser/src/lib.rs` to parse the new construct. Use `peek()` and `consume()` to handle tokens.

---

## 2. Implementing Logic (HIR & Typeck)

### Step 2.1: HIR
Add the new node to `crates/fero-hir/src/hir.rs`.
Implement the conversion from AST to HIR in `crates/fero-hir/src/lower.rs`.

### Step 2.2: Type Checker
Define validation rules in `crates/fero-typeck/src/check_expr.rs` or `check_stmt.rs`.
Ensure errors are reported via `ctx.error(msg, span)`.

---

## 3. Runtime Implementation (Interpreter)

Update `crates/fero-interpreter/src/eval_expr.rs` or `eval_stmt.rs`.
- Handle the new HIR variant in the `match` arms.
- Return `Result<ControlFlow, String>`.
- For variables, use `env.get()`, `env.insert()`, or `env.update()`.

---

## 4. Testing

### 4.1 Unit & Integration Tests
Each core crate has a `tests/` directory.
- `crates/fero-typeck/tests/typeck_tests.rs`: Use `expect_ok()` or `expect_err()`.
- `crates/fero-interpreter/tests/interpreter_tests.rs`: Use `assert_runs()` and compare against `Value` variants.

Commands:
```bash
# Run all tests
cargo test

# Run specific integration tests
cargo test --test typeck_tests
cargo test --test interpreter_tests
```

### 4.2 Manual Verification
Create a file in `examples/feature_name.fero` and run it:
```bash
cargo run -- run examples/feature_name.fero
```

---

## Useful Information

- **Spans**: Always preserve `Span` information for better diagnostics.
- **Value Semantics**: Fero is value-oriented. Use `Rc` inside `Value` (like in `Value::Array`) for shared storage in slices.
- **Control Flow**: The interpreter uses `ControlFlow` to handle `Return`, `Break`, `Continue`, `Exit`, and `Crash`. Ensure these are propagated correctly.
- **Lexical Scoping**: Use `env.push_scope()` and `env.pop_scope()` in blocks and loops.
