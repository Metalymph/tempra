# Tempra Surface Syntax v0.1

## 1. Design Goals

Tempra v0.1 focuses on:

- minimal and readable syntax
- no implicit coercions
- explicit semantics
- strong scripting ergonomics
- no unnecessary symbols or keywords
- Zig-style generics with specialization-oriented design
- explicit type instantiation for generic constructors
- no exception system
- clear path from script to native binary

The language is influenced by Swift, Rust, Go, and MoonBit, but aims to remain simpler than all of them.

---

## 2. Core Rules

- immutability by default
- `var` for mutability
- no exceptions, only explicit errors
- only one loop construct: `for`
- `if` and `match` are expressions
- `for` can be a statement or an expression
- primary compound types:
  - `struct`
  - `enum`
- structural static interfaces (future)
- script mode has no required `main`
- app mode requires `main`

---

## 3. Bindings

### Immutable

```tempra
let name = "Lorenzo"
```

### Mutable

```tempra
var count = 0
```

Rules:

- `let` = immutable binding
- `var` = mutable binding
- no `let mut`

---

## 4. Functions

```tempra
fn add(a: UInt64, b: UInt64) -> UInt64 {
    a + b
}
```

Explicit return:

```tempra
fn add(a: UInt64, b: UInt64) -> UInt64 {
    return a + b
}
```

The last expression of a block is returned if `return` is not used.

### Generic Functions

Tempra supports specialization-oriented generics using `type` parameters.

```tempra
fn identity(type T, value: T) -> T {
    value
}

let a = identity(1)            // inferred: T = Int
let b = identity[Int](1)       // explicit specialization
```

Rules:
- Generic parameters use the `type T` syntax in the parameter list.
- Explicit specialization uses `foo[Int](...)` syntax.
- Inference is strict; no implicit coercions across generic boundaries.
- Substitution is structural and recursive.

---

## 5. Comptime

Tempra supports explicit compile-time evaluation via `comptime`. This allows deterministic expressions to be evaluated during type checking and materialized as primitive constants.

See [COMPTIME_SPEC.md](./COMPTIME_SPEC.md) for the full specification.

### Comptime Expression
```tempra
let x = comptime fib(10)
```

### Comptime Block
```tempra
let table = comptime {
    let a = 1
    let b = 2
    a + b
}
```

### Comptime Parameters (Phase 2)

Named functions can declare `comptime` parameters. These must be compile-time-known at the call site and participate in specialization identity.

```tempra
fn add_n(comptime n: Int, x: Int) -> Int {
    x + n
}

fn main() {
    let result = add_n(3, 4)   // ok: literal is comptime-known
    expect(result == 7)
}
```

`comptime` parameters are immutable and rejected on closures. Runtime locals cannot be passed as `comptime` arguments regardless of how they were initialized.

### Type Values and Type Equality (Phase 3)

Generic type parameters are available as values. Type equality uses `==` / `!=`.

```tempra
fn is_int(type T) -> Bool {
    T == Int
}

fn main() {
    expect(is_int[Int]())       // true
    expect(!is_int[String]())   // true
}
```

### Compile-time Branching (Phase 4)

`if comptime` allows branch selection during type checking / specialization. Only the selected branch is semantically checked.

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

Rules:
- The condition must be evaluable by the restricted comptime interpreter.
- Unselected branches must be syntactically valid but are not semantically checked.
- `compile_error` is allowed within an `if comptime` branch.

### Not Yet Implemented

- `comptime match` (compile-time pattern matching)
- Generated HIR specialization / runtime erasure of comptime parameters
- Comptime IO, resources, or build configuration

---

## 5. Blocks

```tempra
{
    let x = 10
    let y = 20
    x + y
}
```

Blocks are expressions.

---

## 6. If Expression

```tempra
let result = if ok {
    "done"
} else {
    "failed"
}
```

---

## 7. Match Expression

```tempra
let text = match result {
    Ok(value) => value
    Err(err) => err.message
}
```

### Supported Patterns

- **Wildcard:** `_` matches anything without binding.
- **Binding:** Lowercase identifiers bind the full matched value (e.g., `x => ...`).
- **Literal:** Match by value equality (e.g., `0`, `"ok"`, `true`).
- **Tuple Pattern:** `(a, b)` destructures any tuple. Must have arity >= 2.
- **Struct Pattern:** `User { name }`, `User { name: n }`, `User { field: pattern }`.
- **Constructor:** `Some(v)`, `None`, `Ok(v)`, `Err(e)`. Fully recursive.
- **OR-Patterns:** `Pattern1 | Pattern2`. Supported in `match` only. Alternatives must bind same names with same types. Binding/Wildcard alternatives are forbidden in OR-patterns.

### Guards

Match arms support boolean guards:

```tempra
match x {
    Some(v) if v > 10 => 1
    _ => 0
}
```

### Exhaustiveness

Tempra performs exhaustive checking for `Bool` and `enum` types (including `Option`/`Result`).
- Guarded arms do **not** contribute to coverage.
- Struct and Tuple patterns are recognized but require a wildcard `_` for exhaustiveness in v1.
- `match` on `Never` requires no arms.

---

## 8. For

### Statement

```tempra
for x in items {
    io.println(x)
}
```

### With filter

```tempra
for x in items if x > 0 {
    io.println(x)
}
```

### Expression

```tempra
let ids = for user in users if user.active {
    user.id
}
```

Rules:

- `for` statement iterates
- `for` expression collects results into a dynamic array
- the optional `if` in the header filters iterations

---

## 9. Struct

### Definition

```tempra
struct User {
    name: String
    var bank_value: UInt64
}
```

### Generic Structs

Generic structs use Zig-style type constructors:

```tempra
struct Box(type T) {
    value: T
}

let b: Box(Int) = Box(Int) { value: 1 }
```

Rules:
- Generic types are instantiated using function-call syntax: `Box(Int)`.
- Instantiation is strict; `Box` without arguments is invalid if `Box` is generic.
- Type arguments are part of the type identity.

### Default values

```tempra
struct User {
    name: String = "ciao!"
    var bank_value: UInt64 = 34567
}
```

Rules:

- fields are immutable by default
- `var` marks mutable fields
- `let` is not used in fields

### Initialization

```tempra
let user = User {
    name: "Lorenzo",
    bank_value: 50000,
}
```

### Struct update

```tempra
let rich_user = User {
    bank_value: 99999,
    ..user
}
```

---

## 10. Enum

```tempra
enum Result {
    Ok(String)
    Err(String)
}
```

### Generic Enums

```tempra
enum Maybe(type T) {
    None
    Some(T)
}

let x: Maybe(Int) = Some(1)
```

Rules:
- Generic enums use type-constructor syntax: `Maybe(Int)`.
- Generic enum constructors (`Some`, `Ok`, etc.) require an **expected type context** to resolve.
- Ambiguous generic constructors without expected type are errors.
- Tempra v1 does not infer the full generic enum type from the constructor alone (no `let x = Some(1)`).

Usage:

```tempra
let r = Ok("done")
```

Rules:

- **Construction**: Enum constructors are resolved contextually.
- **Lexical Priority**: If a name matches a variable or function in scope, it is NOT treated as a constructor. Shadowing is explicit.
- **Ambiguity**: If multiple enums share a variant name, the `expected` type (from function signatures or explicit context) is used to disambiguate. If ambiguity remains, it is a type error.
- **Payload Arity**: v1 supports 0 or 1 positional payload components.
- **Pattern Matching**: Enum variants can be matched in `match` and `is`. Payloads are bound to identifiers or matched with nested patterns.

---

## 11. Tuple

```tempra
let pair = (name, age)
let (name, age) = get_user()
```

Rules:

- tuples are positional only
- no named tuples
- tuples do not replace structs

---

## 12. Arrays and Slices

### Dynamic array

```tempra
let xs = [1, 2, 3]
```

### Static array

```tempra
let xs: [UInt64; 4]
```

### Slice

```tempra
let part = xs[1..3]
```

Rules:

- core collection forms are:
  - dynamic arrays
  - static arrays
  - slices
- slices are **zero-copy views** over arrays or other slices
- arrays and slices share identical **read-only** semantics:
  - `.len()` → `Int`
  - `xs[i]` → element type
  - `xs[a..b]` → `Slice(T)`
  - `for x in xs { ... }` — iteration
  - `print(xs)` / `dbg(xs)` — identical human-readable formatting `[1, 2, 3]`
- slices are **read-only** at the language level in v0.1 (no write-through)
- no large family of built-in container types in v0

---

## 13. String

```tempra
let text: String = "hello"
```

Rules:

- a single public string type: `String`
- UTF-8 internal representation
- `s.len()` returns the number of **Unicode scalar values**
- `s.size()` returns the number of **UTF-8 bytes**
- `s.bytes()` returns a read-only `Slice(UInt8)` view of the UTF-8 bytes
- `for ch in s` iterates over **Unicode scalar values** (binding `ch` as a single-scalar `String`)
- `for b in s.bytes()` iterates over raw UTF-8 bytes (`b` as `UInt8`)
- **string indexing `s[i]` is intentionally unsupported**
- **string slicing `s[a..b]` is intentionally unsupported**
- no implicit byte-to-string conversions

### Option (v0)

Tempra supports an explicit optional model:

- semantic type: `Option[T]`
- surface sugar in type position: `T?` (equivalent to `Option[T]`)
- constructors in expression position:
  - `Some(value)`
  - `None`

Examples:

```tempra
fn first(xs: [Int]) -> Int? {
    None
}

let x = Some(1)
```

Rules:

- `T?` is type-position sugar only
- `Some(value)` constructs `Option[T]` from `value: T`
- `None` is the empty option value
- this step does not add `if let`, `guard`, or Option helper methods
- `expr?` propagation syntax is unchanged and is not extended to Option in v0

### Result Sugar (v0)

Tempra supports concise type-position sugar for `Result`:

- `T!E` is sugar for `Result[T, E]`
- `T!` is sugar for `Result[T, Error]`

Examples:

```tempra
fn load() -> String! {
    Ok("x")
}

fn load2() -> String!IoError {
    Err(IoError {})
}

fn main()! {
    print("ok")
}
```

Meaning family (disambiguated by position):

- `T?` in type position: `Option[T]`
- `T!E` / `T!` in type position: `Result[...]`
- `expr?` in expression position: Result propagation
- `expr!` in expression position: unwrap/crash behavior

## 13. Never Type

`Never` is the bottom type in Tempra. It represents computations that never produce a value (e.g., those that crash or exit).

Rules:
- No runtime values of type `Never`.
- `crash(...)` and `exit(...)` return `Never`.
- `Never` is compatible with any expected type.
- Branch unification: `T + Never => T`.
- A `match` where all arms return `Never` is itself type `Never`.

## 14. is Operator and Narrowing

Tempra does not have `if let`. Instead, the `is` operator is used for type/pattern tests and branch-local narrowing.

```tempra
if value is Some(v) {
    // v is bound here
}
```

Rules:
- Result type is always `Bool`.
- **Narrowing:** If the tested expression is a simple variable, it is rebound/narrowed in the true branch.
- **Bindings:** Pattern bindings do not leak outside the branch.
- **Negative Narrowing:** Not implemented in v1.
- OR-patterns and wildcards are **not** supported in `is`.
- Naked bindings (e.g., `if x is y`) are forbidden.

---

## 15. Numeric Types

Supported numeric types:

- `Int8`
- `Int16`
- `Int32`
- `Int64`
- `UInt8`
- `UInt16`
- `UInt32`
- `UInt64`
- `Float32`
- `Float64`

Rules:

- no `Double`
- no `Float`
- no alias like `UInt = UInt64`

---

## 16. Inout

```tempra
fn increment(inout value: UInt64) {
    value = value + 1
}
```

Rules:

- values are passed immutably by default
- `inout` enables explicit mutable passing
- no `&mut` syntax in surface syntax

---

## 17. Methods and Extend

### Methods inside a type

```tempra
struct User {
    name: String
    var bank_value: UInt64

    fn display(self) -> String {
        self.name
    }
}
```

### Extend blocks

```tempra
extend User {
    fn is_rich(self) -> Bool {
        self.bank_value > 100000
    }
}
```

---

## 18. Errors

Tempra has no exceptions.

```tempra
match fs.read("file.txt") {
    Ok(text) => io.println(text)
    Err(err) => {
        io.println(err.message)
        exit(1)
    }
}
```

Errors are explicit values.

---

## 19. Conversion and Casting

### Simple explicit conversion

```tempra
let x = UInt64(value)
let y = Float64(count)
let text = String(number)
```

### Parsing from text

```tempra
let n = UInt64.parse(text)
let f = Float64.parse(text)
```

### Fallible conversion

```tempra
let x = UInt8.try(value)
```

Rules:

- `Type(value)` is the standard explicit conversion form
- `Type.parse(text)` is for parsing text into values
- `Type.try(value)` is for fallible conversions such as narrowing

---

## 20. Operator Overloading

Tempra allows limited operator overloading.

### Supported for user-defined types

- `+`
- `-`
- `*`
- `/`

Rules:

- no custom operators
- no custom precedence
- no unrestricted operator overloading

---

## 21. Boolean Operators

Tempra uses:

- `&&`
- `||`
- `!`

Tempra does not use:

- `and`
- `or`
- `not`

---

## 22. Closures

Tempra supports explicit-capture closures as first-class callable values.

### Syntax

```tempra
let add = fn(x: Int) -> Int {
    x + 1
}

let base = 10
let add_base = fn[ref base](x: Int) -> Int {
    base + x
}
```

### Function Item vs Closure Value

Tempra distinguishes between **Function Items** and **Closure Values**:

- **Function Item** (`fn name(...) { ... }`):
  - Named and static.
  - No lexical environment (no captures).
  - Can be recursive.
- **Closure Value** (`let name = fn[...]() { ... }`):
  - First-class callable value.
  - Can have an explicit lexical environment via captures.
  - **Recursion**: Supported via explicit `self` capture: `fn[self](...) { ... self(...) ... }`.

### Syntax Grammar

```
CaptureList
  = Capture ("," Capture)*
  | "self" ("," Capture)*

Capture
  = CaptureMode Identifier
```

### Rules and Semantics

- **Explicit Captures**: No implicit capture of outer variables. Every captured value must be explicitly listed in `[...]`.
- **Recursive closures**: Explicitly captured via `[self]`.
  - `fn[self]` provides a stable recursive self-reference within the closure body.
  - Recursive identity is preserved across calls and through value passing.
  - `self` behaves as an immutable local binding of the closure's function type.
  - `self` is only available if explicitly listed in the capture list as `self` (without modifiers).
  - `self` is reserved inside the closure body and cannot be shadowed by parameters or local bindings.
- **Nested Closure Isolation**: Recursive `self` does not automatically propagate to nested closures. To use recursion in a nested closure, it must be explicitly captured (e.g., `fn[ref self](...)`).
- **Implicit recursion rejected**: `let fact = fn(n: Int) -> Int { fact(n - 1) }` is invalid (missing explicit capture).
- **Capture Modes**:
  - `ref`: Read-only capture. Body cannot assign to it.
  - `inout`: Mutable capture (requires source to be `var`). Mutation persists across calls.
  - `consume`: Transfers ownership into the closure. The variable is no longer usable in the outer scope after closure creation.
- **Empty Environment**: `fn()` (without brackets) is sugar for a closure with a guaranteed empty environment.
- **No `fn[]`**: Empty capture lists `fn[]` are forbidden. Use `fn()` instead.
- **Shadowing**: Captured names (including `self`) cannot be shadowed by parameters or local variables inside the closure body.
- **Equality**: Closures are not comparable.

### Storage and Escaping

- **Storage**: The closure storage strategy and runtime representation are implementation-defined. The language does not guarantee specific memory layouts or allocation strategies (e.g., heap vs stack).
- **Escaping**: Closures may be stored and passed as values. Lifetime and escape restrictions are implementation-defined in v1 and will be hardened in future versions with ownership/borrow analysis.

> [!NOTE]
> The runtime representation of recursive closures is implementation-defined and must not be relied upon by user code.

> [!NOTE]
> **Tail-Call Optimization (TCO)**: may be implemented in the future but is not currently guaranteed.

### Closure Types

```tempra
let f: fn(Int) -> Int = ...
```

Rules:
- Closure types are written as `fn(params...) -> Return`.
- Function items (defined with `fn name(...)`) are not automatically closure values, but can be converted or used where a closure is expected if they match the signature and have no environment.

---

## 23. Commands

Commands are ordinary values of type `Command`.

```tempra
let c = cmd("git", ["status"])
```

### Supported command operators

- `|`
- `&&`
- `||`

---

## 24. Imports

Supported import forms:

```tempra
import std/fs
import std/process
import ./util
import pkg/json
```

Rules:

- `std/...` for standard library modules
- `./...` for relative imports
- `pkg/...` for package imports
