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

### Compile-time Branching (Phase 4 — Implemented)

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

### Compile-time Pattern Matching (Phase 5 — Implemented)

`comptime match` allows pattern-based branch selection during type checking.

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

Rules:
- Scrutinee must be evaluable at compile time.
- Generic deferral: checking is deferred if scrutinee depends on generic parameters.
- Arm pruning: unselected arms are erased from the HIR and not semantically checked.
- Exhaustiveness is checked during specialization.

### Not Yet Implemented

- Generated HIR specialization / runtime erasure of comptime parameters
- Comptime IO, resources, or build configuration
- Complex patterns in `comptime match` (constructors, tuples, structs)

### Static Duck Interfaces (Phase 6 — Implemented)

Tempra supports static structural interfaces to constrain generic types during specialization.

```tempra
interface Display {
    fn display(self) -> String
}

fn show(type T: Display, value: T) -> String {
    value.display()
}
```

Rules:
- Interfaces are strictly static constraints used for generics `type T: Interface`.
- They cannot be used as runtime variable types (no vtables).
- Interface satisfaction is structural: a type satisfies an interface if it has all the methods with matching signatures.
- Methods can be on structs/enums or in `extend` blocks.
- `Self` inside an interface represents the implementing type.
- Constraints on `struct` or `enum` generic parameters are not supported in v1.
- Associated types can be declared with `type Name` inside the interface.
- Associated types can appear in method params and return types.
- Associated type inference is interface-wide. Missing/conflicting associated type inference is an error.
- Associated types are visible outside interfaces via projection syntax `T.Output`.
- Projection resolves during specialization to concrete types.
- **Projection is type-position only.** It is valid in: return types, parameter types, variable annotations, and compound type arguments (e.g. `Array(T.Item)`).
- **Projection in expression position is a type error.** `let x = T.Item` and `print(T.Item)` are rejected with a diagnostic: *associated type projection is only valid in type positions*.
- Nested projection is supported in compound types (e.g. `Array(T.Item)`).
- Cyclic associated type inference is rejected.

### Iterator Protocol

Tempra defines a built-in static interface for iteration:

```tempra
interface Iterator {
    type Item
    fn next(inout self) -> Item?
}
```

Rules:
- `Iterator` is a static-only constraint. It cannot be used as a runtime type.
- The `next` method MUST be `inout self` as it mutates the iterator state.
- Built-in collections provide `.iter()` which returns internal iterator types satisfying `Iterator`.
- Custom types can satisfy `Iterator` by providing a matching `next` method.


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

### Dynamic array (canonical)

```tempra
let xs: Array(Int) = [1, 2, 3]
let ys: [Int] = [1, 2, 3] // sugar for Array(Int)
```

### Slice (canonical)

```tempra
let part: Slice(Int) = xs[1..3]
```

Rules:

- core collection forms are:
  - dynamic arrays: `Array(T)`
  - slices: `Slice(T)`
- slices are **contiguous views** over arrays or other slices. `Slice(T)` is a view, not a copy. Modifying a slice element modifies the underlying array.
- runtime representation of `Slice(T)` is implementation-defined, but it behaves as a view for read and write access.
- arrays and slices share identical read methods:
  - `.len()` → `Int`
  - `.is_empty()` → `Bool`
- arrays and slices share identical read/write semantics:
  - `xs[i]` → element type (read/write)
  - `xs[a..b]` → `Slice(T)` (read/write)
- slice assignment `xs[a..b] = ys` requires matching lengths at runtime.
- **Strict Inference**:
  - `[]` requires an expected type context (e.g., `let xs: Array(Int) = []`).
  - `[crash("msg")]` requires an expected type context to determine element type.
  - elements must have compatible types.
- no large family of built-in container types in v0.1.

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

Current runtime semantics for `Int64` addition, subtraction, multiplication and
unary negation are checked. Results outside the signed 64-bit range fail with a
controlled integer-overflow error. Division by zero fails with a distinct
controlled error, while `Int64::MIN / -1` is overflow. These failures require
the conservative `Panic` capability. Rust panics, native traps and accidental
wrapping are not Tempra semantics. This rule does not define `UInt` or floating
point overflow behavior. The future `@verified` design may prove a particular
operation safe, but is not the authority for this current runtime contract.

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
- **Escaping**: Closures may be stored and passed as values. The formal escape and ownership restrictions are not yet specified.

The next ownership phase will define an acyclic invariant for safe Tempra across `ref`, `inout`, `consume`, explicit captures, recursive `self`, structs, enums, and escaping values. The current runtime representation is not the normative ownership model and must not be used to infer that strong cycles are already prevented. This work will not introduce Rust-style lifetime syntax or Pony-style reference capabilities.

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
import std.fs
import std.process
import .util
import pkg.json
```

Rules:

- `std...` for standard library modules
- a leading `.` for relative imports
- dot-separated package module paths

The compiler resolves `std...` from a standard-library root selected at the
toolchain/API boundary. Compiler-authoritative file/module paths never search the
current working directory implicitly. The default root is discovered relative to
the compiler executable; embedding APIs may supply the `std/` directory directly.
An explicitly relative supplied path is interpreted relative to the caller's cwd
once, which is explicit path interpretation rather than hidden discovery.
---

## 24. Subscript Hooks

Tempra allows user-defined types to support indexing and slicing via special method hooks.

### Hook Signatures

```tempra
extend MyType {
    // x[i]
    fn get(self, index: IdxType) -> ValueType { ... }

    // x[i] = value
    fn set(inout self, index: IdxType, value: ValueType) { ... }

    // x[a..b]
    fn slice(self, start: IdxType, end: IdxType) -> ValueType { ... }

    // x[a..b] = value
    fn set_slice(inout self, start: IdxType, end: IdxType, value: ValueType) { ... }
}
```

Rules:
- Hooks must be named exactly `get`, `set`, `slice`, or `set_slice`.
- `set` and `set_slice` **must** return `Unit` and **must** take `inout self` as the first parameter.
- Subscript assignment `x[i] = v` is only allowed if `x` is a mutable binding (`var`).
- The index type (`IdxType`) and value type (`ValueType`) can be any valid Tempra type.
- For built-in arrays, `set_slice` requires the RHS to have the same length as the slice range.

### Limitations in v0.1
- **Nested Subscript Assignment:** `x[i][j] = v` is unsupported in v1 (requires a reference system).
- **String Indexing:** `s[i]` remains intentionally unsupported to avoid common UTF-8 pitfalls. Use `s.bytes()[i]` or explicit iteration.
- **No Protocols:** There are no `Indexable` or `Sliceable` interfaces yet. Hooks are resolved structurally by name.
- **No Associated Types:** The return type of `get` and the value type of `set` are independent but must be compatible for valid assignment.

---

## 25. Built-in Interfaces

Tempra introduces minimal built-in interfaces to facilitate generic programming over core types.

### Collection

The `Collection` interface represents types that have a known length and can be checked for emptiness.

```tempra
interface Collection {
    type Item
    fn len(self) -> Int
    fn is_empty(self) -> Bool
}
```

**Satisfaction Rules:**
- `Array(T)` automatically satisfies `Collection` with `Item = T`.
- `Slice(T)` automatically satisfies `Collection` with `Item = T`.
- User-defined types satisfy `Collection` structurally if they provide the required methods and `Item` can be inferred.

**Example usage:**

```tempra
fn size(type T: Collection, value: T) -> Int {
    value.len()
}

fn id_item(type T: Collection, value: T.Item) -> T.Item {
    value
}
```

---

## 26. Capability Contracts

The current public capability surface is limited to `@deny(...)` and `@pure`.
E3 acquires/enforces `@deny`; E4 normalizes `@pure` into the same contracts.
The separate future `@unsafe` boundary belongs to U (DEFERRED / CONDITIONAL),
not the immediate Phase E sequence.

`@deny(alloc, panic)` requires at least one public name: `alloc`, `panic`, `io`,
`process`, `spawn`, `nondeterminism`. `@deny()` and `@deny(unsafe)` are errors.
Duplicate capability names and repeated `@deny` attributes merge; whitespace/newlines and a trailing
comma are allowed. Place attributes before optional `pub`, on named top-level
functions or methods inside `extend`. Externs, closures, interface requirements,
fields and other items cannot carry them. Unannotated functions have no contract.
E3.2 rejects any intersection with the emitted function's transitive requirements,
including non-entry functions and each concrete specialization.

- `@deny(...)` prohibits the named direct and transitive capability requirements.
- `@pure` means `@deny(io, process, spawn, nondeterminism)`; pure code may allocate, may panic and may return `Result`.
- `@unsafe` is reserved as a future direction for local audited implementation islands, not an alias for capability denial or currently accepted syntax.

Write bare `@pure` before optional `pub`, on the same functions/methods as deny.
It takes no parentheses or arguments; a duplicate is an error on the second marker.
Compose pure and deny in either order: restrictions are unioned. Pure does not
deny Alloc or Panic unless explicitly added through deny. Pure provenance is
preserved separately from explicit deny spans; `pure` is still an ordinary identifier.

`@unsafe` and `@verified` are not implemented. E3.2 cache hits require an
enforcement-aware semantic fingerprint. HIR execution, REPL, test execution and
source TIR interpretation explicitly reject annotated programs until they can
enforce contracts. `emit-tir` may emit without enforcement; `check` certifies
frontend/type correctness only. Neither is capability-contract certification.
`Panic` remains conservative may-panic; this is not `@verified` reasoning.
E4 retains `semantics-1`: pre-E4 parsing rejected pure, attribute edits change
source identity, and existing enforcement remains authoritative. No contract
metadata is serialized into cache.

K1.7 subsequently raises the compiler fingerprint to `semantics-2` so
artifacts predating the canonical comptime guarantees cannot bypass fresh
evaluation. This is independent of bytecode format version and adds no contract
or capability metadata to cache artifacts. K2.2 raises current trust to
`semantics-3` after repairing specialization function-identity mapping. K2.4
raises current trust to `semantics-4` after making interface selection and
diagnostic iteration follow canonical source/module order; the bytecode format
remains unchanged.

K3.2 raises current trust to `semantics-5` after establishing checked,
host-safe `Int64` arithmetic and its conservative `Panic` origins across the
supported runtime paths. This semantic revision still does not change the
bytecode format or serialize capability metadata.

K2.5 does not change compiler semantics or the fingerprint. It establishes a
permanent cross-process gate requiring identical raw artifacts for the currently
supported bytecode surface when source/module order, compiler build, semantic
revision, format, options, resolved stdlib contents, host and target are the same.
Comptime, specialization and capability-contract fixtures are included. Associated-
type selection and its diagnostics are checked at the frontend boundary because
that surface is not yet bytecode-lowerable. This is a bounded Level 3 guarantee,
not Level 4 reproducibility across machines or targets.

E1–E6 complete the currently meaningful semantic and tooling surface, not all
TIR coverage. K1 has completed its canonical backend-independent comptime
authority and cache trust boundary. K2 completes audited deterministic ordering,
explicit stdlib input and the bounded reproducibility gate. K3–K6 continue runtime parity,
timings, latency baselines and regression budgets. The immediate roadmap
continues K → R → M → C → V → O → P, with U conditional. R is future Result/value-use hardening:
no silent discard, including an accidental `let _ = ...` escape hatch; handling
through `?`, `match`, return/propagation or meaningful use. Neither wildcard-binding
syntax nor an opt-out such as `@discardable` is added or committed here.

U activates only when a real source-level operation can violate memory/type
invariants without explicit authorization. None is intentionally exposed today,
and no E1 origin produces internal `Unsafe`. Future safe wrapper interfaces should
remain callable without contaminating ordinary callers, while preserving safety
for their admitted inputs and all deny/pure restrictions. Unsafe-to-call rules
remain unspecified until a concrete operation needs them.

See [Capability Contracts v0.1](specs/effects-v0.1.md). Normal Tempra remains safe and checked by default; there is no `--checked`, global `--verified`, or Spark mode.

## 27. Future Directions (v0.2+)

- Iterators and `Iterable` interface
- `for-in` protocol lowering
- Associated constants
- Existential interfaces and dynamic dispatch
- Capability analysis
- Acyclic ownership analysis
- Concurrency based on owned/share-safe values
- Verified functions
