# Tempra Comptime Specification

`comptime` is an explicit compile-time evaluation form. The model is small, typed, and deterministic.

---

## Phase 1 — Implemented

- `comptime expr`
- `comptime { ... }`
- Primitive materialization: `Int`, `Float`, `Bool`, `String`, `Unit`
- Restricted evaluator mode (deterministic, no IO)
- Rejection of runtime locals and parameters inside `comptime` expressions
- Rejection of filesystem, environment, OS, and process operations during `comptime` evaluation
- `compile_error("message")` in `comptime` contexts
- Evaluation fuel to prevent unbounded compile-time loops

### Syntax

```tempra
let x = comptime 1 + 2
```

```tempra
let y = comptime {
    let a = 1
    let b = 2
    a + b
}
```

The block form follows normal block expression semantics: the final expression is the value of the block.

### Restrictions

`comptime` code may only reference values known at compile time. Runtime parameters and runtime locals are rejected before evaluation regardless of how they were initialized.

```tempra
fn f(x: Int) {
    let y = comptime x + 1 // error: runtime variable 'x' cannot be used in comptime context
}
```

```tempra
fn f(x: Int) {
    let n = 3
    let y = comptime n + 1 // error: runtime variable 'n' cannot be used in comptime context
}
```

The restricted evaluator rejects filesystem, environment, OS, and command/process operations during `comptime` evaluation.

### `compile_error`

`compile_error("message")` aborts compile-time evaluation with the provided diagnostic message. It is only valid inside a `comptime` context.

```tempra
let x = comptime compile_error("unsupported value")
```

Using `compile_error` at runtime is an error.

---

## Phase 2 — Implemented

- `comptime` value parameters for named functions: `fn f(comptime n: Int, x: Int)`
- Compile-time argument validation at call sites
- Runtime variable rejection for `comptime` arguments (`let n = 3; f(n, x)` → error)
- `comptime expr` as a `comptime` argument (`f(comptime 1 + 2, x)` → ok)
- `comptime` parameters bound as immutable locals inside the function body
- `comptime` parameters rejected on closures (named functions only in this phase)
- `SpecializationKey` bookkeeping: function name + type args + `comptime` args uniquely identify a call-site instantiation

### Syntax

```tempra
fn add_n(comptime n: Int, x: Int) -> Int {
    x + n
}

fn main() {
    let result = add_n(3, 4)   // ok — literal is comptime-known
    expect(result == 7)
}
```

### Semantics

A `comptime` parameter:
- is passed at the call site like a normal argument
- must be comptime-known at the call site (literal, `comptime` expr, or another `comptime` param in scope)
- is available inside the function body as a compile-time constant
- is part of the function specialization key
- is immutable; assignment inside the body is an error
- is not supported on closures in this phase

### SpecializationKey

`SpecializationKey` records a unique function instantiation:

```
SpecializationKey {
    function_name: String,  // temporary — will be replaced with FunctionId in a future phase
    type_args: Vec<Type>,
    comptime_args: Vec<ComptimeValue>,
}
```

> **Note:** `function_name` is a temporary identifier used for Phase 2 bookkeeping. The final model must use a stable `FunctionId`. Do not treat `function_name` as a stable identity.

> **Note:** Phase 2 tracks specialization identity semantically via `SpecializationKey`, but runtime erasure of `comptime` parameters and generation of specialized HIR function bodies are **not implemented** in this phase. `comptime` parameters remain as normal runtime parameters in the current interpreter. Full specialization is deferred to a future phase.

### Error examples

```tempra
fn f(comptime n: Int, x: Int) -> Int { x + n }

fn main() {
    let n = 3
    let y = f(n, 4)   // error: runtime variable 'n' cannot be used in comptime context
}
```

```tempra
let g = fn(comptime n: Int) -> Int { n }
// error: comptime parameters are only supported on named functions in this phase
```

```tempra
fn f(comptime n: Int) {
    n = 10   // error: cannot assign to immutable value 'n'
}
```

---

## Phase 3 — Implemented

- `Value::Type(Type)` — type values at runtime and in comptime contexts
- Generic type parameters (`type T`) resolved to `Value::Type` in the interpreter
- Type equality: `T == Int` evaluates to a `Bool` at runtime / comptime
- Primitive type names (`Int`, `Bool`, `String`, …) resolve to `Value::Type` when referenced as expressions
- `Eq` and `NotEq` operators unified to use structural equality across all `Value` variants

### Syntax

```tempra
fn is_int(type T) -> Bool {
    T == Int
}

fn main() {
    expect(is_int[Int]())       // true
    expect(!is_int[String]())   // true
}
```

---

## Not Yet Implemented

- `if comptime` — branch selection at compile time
- `comptime match` — pattern match at compile time (unselected arms not evaluated)
- Type equality in `comptime` blocks (deferred; requires late-binding evaluation or specialization cloning)
- Non-primitive `comptime` materialization
- Full HIR specialization: cloning function bodies, substituting `comptime` constants, removing `comptime` params from runtime signatures
- General constant IR / `FunctionId`-based specialization identity
