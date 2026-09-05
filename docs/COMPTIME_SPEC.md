# Tempra Comptime Specification

`comptime` is an explicit compile-time evaluation form. The intended model is
small, typed, deterministic and independent of the selected runtime backend.

## K1 implementation status

The K1 audit selected the existing HIR interpreter as Tempra's canonical
comptime semantic kernel. K1.1 adds one documented evaluation facade: it owns
entry into comptime mode, installs the shared one-million expression-step
budget, normalizes the value/error/control-flow boundary, and restores the
interpreter's previous mode and fuel after every ordinary return. The
typechecker remains responsible for converting accepted primitive values into
HIR literals.

Nested `comptime` nodes reuse the active session and its remaining fuel instead
of opening a second session. Resolved values and selected branches are committed
to HIR before TIR lowering; TIR and its backends do not evaluate comptime.

K1.2 extends the same authority to ordinary multi-module and file-based
typechecking by giving the interpreter a module-aware lookup adapter. Module HIR
files remain separate and imports retain their qualified identities. The
analysis/LSP and REPL typechecking paths remain intentionally deferred.

K1.3 makes the shared HIR integer-arithmetic kernel panic-free for supported
operations. `Int64` addition, subtraction and multiplication report overflow;
division reports zero and signed `MIN / -1` separately; unary negation reports
overflow. `UInt8` division by zero is controlled. Existing `UInt8`
addition/subtraction/multiplication wrapping behavior is unchanged by K1.3 and
is not made a normative language rule here. Float arithmetic, including IEEE
division by zero, is unchanged.

Remainder/modulo is not current source syntax. Right shift is parsed but rejected
by the typechecker, so neither is part of the supported K1.3 arithmetic boundary.

K1.4 rejects calls to functions without executable HIR bodies in the shared
interpreter dispatch. This includes source `extern` declarations during
comptime: the evaluator reports a controlled error instead of synthesizing
`Unit`. Normal empty function bodies still evaluate to `Unit`, and VM-native
extern dispatch remains a separate runtime path. K1.4 does not add native
execution to comptime or validate native signatures.

K1 remains in progress. Complete effect restrictions, recursion limits and
specialization correctness are separate later slices.

---

## Phase 1 — Implemented

- `comptime expr`
- `comptime { ... }`
- Primitive materialization: `Int`, `Float`, `Bool`, `String`, `Unit`
- Canonical restricted-evaluator entrypoint with shared mode/fuel setup
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

## Phase 4 — Implemented

- `if comptime` — branch selection at compile time
- Type-directed branching: using type equality to select implementation branches
- `compile_error` support within selected `if comptime` branches
- Deferred typechecking: unselected branches are parsed/lowered but not semantically checked

### Syntax

```tempra
fn default_value(type T) -> T {
    if comptime T == Int {
        0
    } else if comptime T == Bool {
        false
    } else {
        compile_error("unsupported type")
    }
}
```

### Compile-Time Branch Selection

`if comptime condition { then } else { else }`

Rules:
- The `condition` MUST be evaluated at compile-time. If it contains runtime variables, it is a compile error: "if comptime condition must be known at compile time".
- The `condition` MUST evaluate to a `Bool`.
- **Deferred Typechecking**: During the definition of a generic function, evaluation of the condition is deferred ONLY if it depends on a generic `type T` parameter. During specialization, the condition MUST resolve and select exactly one branch.
- **Strict Branching**: ONLY the selected branch is typechecked. The resulting type of the `if comptime` expression is strictly the type of the selected branch.
- **Unselected Branches**: The non-selected branch MUST NOT be typechecked at all. It must only be syntactically valid. It can contain type errors or `compile_error` calls without failing the build.
- **`compile_error` behavior**: `compile_error("...")` behaves as a compiler abort. It will fail compilation ONLY if the branch containing it is selected and typechecked.
- **No Internal Leakage**: The behavior is purely derived from whether the value is known and whether it is a generic type. It explicitly forbids relying on internal flags or implementation details.

---

## Phase 5 — Implemented

- `comptime match` — pattern match at compile time
- Branch selection based on compile-time known scrutinee
- Support for literals and type names in patterns
- Mandatory arm pruning in HIR: unselected arms are erased
- Exhaustiveness checking via compile-time evaluation

### Syntax

```tempra
fn default_value(type T) -> T {
    comptime match T {
        Int => 0
        Bool => false
        String => ""
        else => compile_error("unsupported type")
    }
}
```

### Semantics

A `comptime match` works like compile-time branch selection:

1. **Scrutinee Evaluation**: The scrutinee is evaluated at compile time.
2. **Generic Deferral**: If the scrutinee depends on a generic `type` parameter or a `comptime` parameter, checking is deferred until specialization.
3. **Arm Selection**: Patterns are checked in order. Currently supported: literal patterns and type identifier patterns.
4. **Arm Pruning**: ONLY the selected arm is typechecked. Unselected arms are erased from the HIR.
5. **Exhaustiveness**: At specialization time, at least one arm MUST match (or an `else` arm must be provided).

### Capability-analysis boundary

Selected `if comptime` branches and `comptime match` arms are retained in HIR; unselected alternatives are pruned before TIR lowering. The existing TIR capability analysis therefore observes only the selected semantic branch when specialization has resolved it. This is semantic pruning, not a promise that later optimization, inlining, or dead-code elimination will remove capability requirements.

Comptime is an execution phase, not an effect or an unsafe authorization. Its
restrictions are implemented by the HIR evaluator and frontend checks, not by E1.
All currently supported single-source and ordinary multi-module/file-check
evaluation entrypoints now enter the HIR kernel through the K1.1 facade; K1.2
supplies module-aware lookup without flattening HIR. This centralizes execution
setup but does not yet harden the policy itself.
The evaluator has no dispatch to the VM native registry. K1.4 explicitly rejects
unsupported/bodyless calls rather than yielding and materializing synthetic
`Unit`. This rejection is not support for comptime native execution; native
signature integrity remains a separate debt in the
[audit record](E1_CAPABILITY_ANALYSIS.md). No `@unsafe` interaction is implemented.

---

## Not Yet Implemented

- Non-primitive `comptime` materialization
- Full HIR specialization: cloning function bodies, substituting `comptime` constants, removing `comptime` params from runtime signatures
- General constant IR / `FunctionId`-based specialization identity
- Complex patterns in `comptime match` (constructor patterns, tuple patterns, etc.)
- `comptime` IO/resources/config
