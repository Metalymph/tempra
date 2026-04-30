# Fero Pattern Matching Spec (v1.6)

## 1. Match Expression

`match` is an expression that evaluates an operand against a series of arms. Each arm consists of a pattern, an optional guard, and a body expression.

```fero
let x = match value {
    Some(v) => v
    None => default_val
}
```

Match arms are evaluated in order. The first arm whose pattern matches (and whose guard, if present, evaluates to `true`) wins.

---

## 2. Supported Patterns

### 2.1 Wildcard
```fero
_ => expr
```
Matches anything, binds nothing.

---

### 2.2 Binding (Catch-all)
```fero
x => expr
```
Matches anything, binds the value to `x`.

---

### 2.3 Literal
```fero
42 => expr
"hello" => expr
true => expr
```
Matches an exact value.

---

### 2.4 Enum Constructor
```fero
Some(v) => expr
None => expr
Ok(v) => expr
Err(e) => expr
Present(Data(n)) => expr   // nested payload
```
Matches a specific enum variant. Payload patterns are fully recursive.

---

### 2.5 Struct Patterns

Three forms are supported, all first-class:

#### Shorthand binding
```fero
User { name }
```
- Matches a struct of type `User`.
- Binds the value of field `name` to a local variable also named `name`.

#### Field renaming
```fero
User { name: n }
```
- Matches a struct of type `User`.
- Binds the value of field `name` to a local variable named `n`.

#### Field subpattern
```fero
User { meta: (age, _) }
User { inner: Data(v) }
```
- Matches a struct of type `User`.
- Validates the field value against the given pattern.
- Recursively extracts bindings from the subpattern.

#### Combinations
All three forms can be freely combined in a single struct pattern:
```fero
User { name: n, meta: (age, _), active }
```

---

### 2.6 Tuple Patterns

Tuple patterns are written as:

    (p1, p2, ..., pn)

Constraints:
- n ≥ 2
- single-element tuple patterns are not allowed
- empty tuple patterns are not supported
- parentheses do not mean grouping in pattern position

### 2.7 OR-Patterns
```fero
Pattern1 | Pattern2 => expr
```
- Supported in `match` arms only.
- All alternatives must bind the same set of names with identical types.
- Binding and Wildcard alternatives are forbidden in OR-patterns (v1).

---

## 3. Match Guards

    (a, b)
    (_, value)
    (User { name }, age)

Invalid:

    (a)
    (a,)
    ()

---

## 3. Match Guards

```fero
match x {
    Some(v) if v > 10 => 1
    Some(v) => 2
    None => 0
}
```

- The guard expression must be of type `Bool`.
- The guard sees the bindings of its arm.
- A guarded arm does **not** contribute to exhaustiveness coverage.
- A guarded catch-all does **not** make later arms unreachable.
- An unguarded catch-all **does** make later arms unreachable.

---

## 4. Diagnostics

| Situation | Error |
|-----------|-------|
| Guard not `Bool` | `match guard must be Bool, found <type>` |
| Unknown struct field | `struct 'X' does not have a field named 'f'` |
| Duplicate struct field | `duplicate field 'f' in struct pattern` |
| Wrong subpattern shape | `cannot match <type> with ...` |
| Tuple arity mismatch | `tuple pattern arity mismatch: expected N, found M` |
| Empty tuple pattern | `empty tuple patterns are not supported` |
| Single-element tuple pattern | `single-element tuple patterns are not allowed; use `a` instead` |
| Non-exhaustive match | `non-exhaustive match on enum 'X': missing arm for variant 'Y'` |
| Duplicate variant arm | `duplicate 'X' arm in match: this case was already covered` |
| Unreachable after catch-all | `unreachable match arm: a previous wildcard ...` |
| Guarded arm after same unguarded | `unreachable match arm` |

---

## 5. Exhaustiveness

Fero performs lightweight exhaustiveness analysis for:
- `Bool` — requires both `true` and `false`
- Enums (including `Option` and `Result`) — requires one unguarded arm per variant

### 5.1 Rules

- **Unknown Space Rule**: Guarded arms (e.g., `Some(x) if x > 0`) produce **Unknown** space and do NOT satisfy exhaustiveness for their variant.
- **OR-Pattern Coverage**: Unguarded OR-patterns contribute to coverage for all their alternatives. Guarded OR-patterns do not.
- **Divergent Match Rule**: Exhaustiveness is determined by case coverage, not successful value production. A match expression is exhaustive if all top-level cases are covered, even if every branch diverges (evaluates to `Never`).
- **Match on Never**: A `match` on a value of type `Never` is exhaustive with zero arms.
- **Tuple/Struct Domains**: Tuples and Structs are recognized as distinct domains but are not yet analyzed for partial exhaustiveness. They must include an unguarded catch-all (`_`) to be considered exhaustive.
- **Top-level Coverage**: Coverage is checked at the **top-level variant** only. Payload subpatterns do not currently affect variant coverage.

---

## 6. Unsupported (v1.8)

- Rest patterns: `User { name, .. }`
- Grouped patterns
- `is` struct patterns
- Full payload-sensitive exhaustiveness

---

## 7. Semantic Invariants

- Binding names introduced in a pattern are scoped to their arm only.
- Guards can only see bindings from their own arm.
- Failed guards do not leak bindings to subsequent arms.
- All three struct pattern forms (`shorthand`, `rename`, `subpattern`) are first-class. None is described as a fallback of another.
