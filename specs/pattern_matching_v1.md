# Fero Pattern Matching Design (Minimal & Future-Proof)

## 1. Syntax Proposal (v1)

### The `match` Expression
`match` evaluates an operand against a series of patterns. It is an expression that yields a value.

```fero
let x = match value {
    Some(v) => v
    None => default_val
}
```

### The `is` Condition (Pattern Matching in `if`)
Fero avoids introducing a separate `if let` construct. Instead, we unify the existing `is` operator with destructuring behavior from the pattern syntax.

```fero
if value is Some(x) {
    print(x)
}
```

### Forbidden Syntax in `if is`
After `is`, Fero accepts only:
- a **type** (`value is String`)
- an **explicit constructor pattern** (`Some(x)`, `None`, `Ok(v)`, `Err(e)`)

Naked binding patterns like `value is x` are intentionally forbidden to keep syntax clear and prevent confusing lower-cased type names with full wild bindings.


### Supported Patterns

*   **Literals:** Exact value matches.
    ```fero
    match n {
        1 => "one"
        "hello" => "greeting"
        true => "yes"
        _ => "other"
    }
    ```
*   **Wildcard:** `_` matches anything without binding it.
*   **Binding:** Identifiers bind the matched value.
    ```fero
    match state {
        x => fmt("State is: ", x)
    }
    ```
*   **Option / Result Types:** Destructuring for builtin monadic types.
    ```fero
    match res {
        Ok(v) => v
        Err(e) => crash(e)
    }
    ```
*   **Basic Structs:** Flat destructuring by field name.
    ```fero
    match user {
        User { name } => name
        _ => "Anonymous"
    }
    ```

---

## 2. Examples

### Option Handling
```fero
let config = match get_config() {
    Some(c) => c
    None => default_config()
}

// Unified with `if`
if fetch_data() is Ok(data) {
    process(data)
}
```

### Struct Matching
```fero
struct Point { x: Int, y: Int }

let description = match pt {
    Point { x } => fmt("X is ", x) // Binds x
}
```

---

## 3. Tradeoffs vs. Other Languages

### Fero vs. Rust
*   **Tradeoff (Fero advantage):** No separate `if let`. Fero's `if value is Pattern` seamlessly mirrors `match`, removing a learning hurdle for beginners.
*   **Tradeoff (Rust advantage):** Rust requires exhaustive patterns from Day 1. Fero defers exhaustiveness, which simplifies initial parser and type-checking design. Fero also cleanly prevents deep verbosity complexity (e.g., auto-ref/deref `&` or `ref mut` complexities).

### Fero vs. Swift
*   **Tradeoff (Fero advantage):** No `guard let` or `if case let` madness. Swift's pattern matching syntax across `switch` and `if` is notoriously fragmented. Fero unifies `is` semantics explicitly.
*   **Tradeoff (Swift advantage):** Swift's pattern ecosystem has powerful overlapping semantics like `where` clauses (guards) natively baked in. Fero defers guards to later revisions to prioritize minimal initial footprint.

### Fero vs. Roc
*   **Tradeoff (Fero advantage):** Familiar C/Rust-family syntax (`=>`).
*   **Tradeoff (Roc advantage):** Roc specializes in highly optimized data-driven pattern constraints leveraging structural typing to guarantee absolute totality. Fero relies more on nominal typing and deferred v2 exhaustiveness checking.

---

## 4. Evolution & Separation (v1 vs Future)

### v1 (Current Scope)
*   **Core matching**: Flat pattern matches against Literals, Bindings, `_` wildcard, `Some`/`None`, `Ok`/`Err`, and flat Struct fields.
*   **Integration**: Introduction of `if value is Pattern` (reusing the v1 parser for `match` patterns).
*   **Typing**: Identifiers introduce rigidly scoped bindings valid only on the right side of `=>` or inside the `if` block. Each match arm evaluates to a unified compatible type.
*   **Excluded**: Exhaustiveness checks, nested destructuring (`Some(Ok(x))`), guards, OR patterns (`1 | 2 =>`).

### v2 (Safety & Capability)
*   **Nested Patterns:** Allowing deep matching like `Some(User { name })`.
*   **Guards:** Syntax like `Some(x) if x > 10 =>`.
*   **Exhaustiveness Check:** The compiler strictly enforces covering every possible data variant, optionally warning or failing compilation if patterns are missing.

### v3 (Advanced & Ecosystem)
*   **Traits / Interfaces:** Extensible matching for user-defined ADTs or polymorphic traits.
*   **OR Patterns:** Matching multiple distinct patterns to a single arm.
