# Fero Comptime Phase 1

`comptime` is an explicit compile-time evaluation form. Phase 1 keeps the model small: it evaluates deterministic expressions during type checking and materializes only primitive values back into the HIR.

## Implemented in Phase 1

- `comptime expr`
- `comptime { ... }`
- Primitive materialization: `Int`, `Float`, `Bool`, `String`, and `Unit`
- Restricted evaluator mode for compile-time execution
- Rejection of runtime locals and parameters inside comptime expressions
- Rejection of obvious IO and process operations during comptime evaluation
- `compile_error("message")` in comptime contexts
- Evaluation fuel for comptime evaluation

## Syntax

```fero
let x = comptime 1 + 2
```

```fero
let y = comptime {
    let a = 1
    let b = 2
    a + b
}
```

The block form follows normal block expression semantics: the final expression is the value of the block.

## Restrictions

Comptime code may only reference values known during the comptime expression itself. Runtime parameters and runtime locals are rejected before evaluation.

```fero
fn f(x: Int) {
    let y = comptime x + 1 // error
}
```

The restricted evaluator rejects filesystem, environment, OS, and command/process operations during comptime evaluation.

## `compile_error`

`compile_error("message")` aborts compile-time evaluation with the provided diagnostic message. It is only valid inside a comptime context.

```fero
let x = comptime compile_error("unsupported value")
```

Using `compile_error` at runtime is an error.

## Not Yet Implemented

- `if comptime`
- `comptime match`
- `comptime` parameters
- Type equality
- Type values
- Specialization keys with comptime arguments
- General constant IR or non-primitive materialization
