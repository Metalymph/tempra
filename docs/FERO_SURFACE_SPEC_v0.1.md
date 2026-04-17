# Fero Surface Syntax v0.1

## 1. Design Goals

Fero v0.1 focuses on:

- minimal and readable syntax
- explicit semantics
- strong scripting ergonomics
- no unnecessary symbols or keywords
- no `let mut`, no `<T>` generics syntax in v0
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

```fero
let name = "Lorenzo"
```

### Mutable

```fero
var count = 0
```

Rules:

- `let` = immutable binding
- `var` = mutable binding
- no `let mut`

---

## 4. Functions

```fero
fn add(a: UInt64, b: UInt64) -> UInt64 {
    a + b
}
```

Explicit return:

```fero
fn add(a: UInt64, b: UInt64) -> UInt64 {
    return a + b
}
```

The last expression of a block is returned if `return` is not used.

---

## 5. Blocks

```fero
{
    let x = 10
    let y = 20
    x + y
}
```

Blocks are expressions.

---

## 6. If Expression

```fero
let result = if ok {
    "done"
} else {
    "failed"
}
```

---

## 7. Match Expression

```fero
let text = match result {
    Ok(value) => value
    Err(err) => err.message
}
```

v0.1 supports:

- literal patterns
- `_` wildcard
- simple bindings
- enum cases

---

## 8. For

### Statement

```fero
for x in items {
    io.println(x)
}
```

### With filter

```fero
for x in items if x > 0 {
    io.println(x)
}
```

### Expression

```fero
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

```fero
struct User {
    name: String
    var bank_value: UInt64
}
```

### Default values

```fero
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

```fero
let user = User {
    name: "Lorenzo",
    bank_value: 50000,
}
```

### Struct update

```fero
let rich_user = User {
    bank_value: 99999,
    ..user
}
```

---

## 10. Enum

```fero
enum Result {
    Ok(String)
    Err(String)
}
```

Usage:

```fero
let r = Ok("done")
```

Rules:

- enum constructors can be inferred from context
- `Enum::Case` is not required

---

## 11. Tuple

```fero
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

```fero
let xs = [1, 2, 3]
```

### Static array

```fero
let xs: [UInt64; 4]
```

### Slice

```fero
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

```fero
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

Fero supports an explicit optional model:

- semantic type: `Option[T]`
- surface sugar in type position: `T?` (equivalent to `Option[T]`)
- constructors in expression position:
  - `Some(value)`
  - `None`

Examples:

```fero
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

Fero supports concise type-position sugar for `Result`:

- `T!E` is sugar for `Result[T, E]`
- `T!` is sugar for `Result[T, Error]`

Examples:

```fero
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

### `is` Operator (v0)

Fero supports runtime type checks as boolean expressions:

```fero
value is Type
```

Rules:

- result type is always `Bool`
- no binding (`is T(x)` is not supported)
- no type narrowing in branches (for now)
- works with primitive, struct, `Option`, and `Result` checks

Examples:

```fero
"abc" is String
Some(1) is Int?
Ok("x") is String!
```

### Regex (v0)

`Regex` is a first-class value type built using composable constructors, not a text mini-language as primary API.

Supported constructors:

- `Regex.text(String)`
- `Regex.any()`
- `Regex.seq(Regex, Regex, ...)`
- `Regex.alt(Regex, Regex, ...)`
- `Regex.optional(Regex)`
- `Regex.zero_or_more(Regex)`
- `Regex.one_or_more(Regex)`
- `Regex.start()`
- `Regex.end()`
- `Regex.digit()`
- `Regex.space()`
- `Regex.word()`

Supported methods:

- `re.is_match(text: String) -> Bool`
- `re.replace(text: String, with: String) -> String` (global replacement, no captures)

Runtime semantics (v0):

- matching and scanning are over Unicode scalar values (not raw bytes)
- `is_match` is true if any match exists somewhere in input, unless constrained by anchors
- `replace` is global, literal replacement only, and scans left-to-right
- after a non-empty match, scanning continues from match end
- after a zero-width match during scanning/replacement, engine advances by at least 1 Unicode scalar value
- zero-width at end-of-text is handled once and terminates (no infinite loops)
- `Regex.start()` matches only at absolute start of full input
- `Regex.end()` matches only at absolute end of full input
- no multiline anchor semantics in v0

Explicitly not part of v0:

- capture groups and named groups
- backreferences
- lookaround
- flags system

Rationale:

- constructor-based regex values are structured data and are ready for future compile-time and AOT optimizations.

---

## 14. Numeric Types

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

## 15. Inout

```fero
fn increment(inout value: UInt64) {
    value = value + 1
}
```

Rules:

- values are passed immutably by default
- `inout` enables explicit mutable passing
- no `&mut` syntax in surface syntax

---

## 16. Methods and Extend

### Methods inside a type

```fero
struct User {
    name: String
    var bank_value: UInt64

    fn display(self) -> String {
        self.name
    }
}
```

### Extend blocks

```fero
extend User {
    fn is_rich(self) -> Bool {
        self.bank_value > 100000
    }
}
```

---

## 17. Errors

Fero has no exceptions.

```fero
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

## 18. Conversion and Casting

### Simple explicit conversion

```fero
let x = UInt64(value)
let y = Float64(count)
let text = String(number)
```

### Parsing from text

```fero
let n = UInt64.parse(text)
let f = Float64.parse(text)
```

### Fallible conversion

```fero
let x = UInt8.try(value)
```

Rules:

- `Type(value)` is the standard explicit conversion form
- `Type.parse(text)` is for parsing text into values
- `Type.try(value)` is for fallible conversions such as narrowing

### v0.1 conversion rules

#### Allowed with `Type(value)`

- safe integer widening
- safe float widening
- total well-defined conversion to `String`

#### Allowed with `Type.parse(text)`

- `String -> UInt64`
- `String -> Int64`
- `String -> Float64`

#### Allowed with `Type.try(value)`

- narrowing integer conversions
- float-to-integer conversions
- conversions that may overflow or lose information

### Not part of v0.1

- `as`
- reinterpret casts
- pointer casts
- unsafe user-facing casts

---

## 19. Operator Overloading

Fero allows limited operator overloading.

### Supported for user-defined types

- `+`
- `-`
- `*`
- `/`

Potential later additions:

- unary `-`
- `==`

Rules:

- no custom operators
- no custom precedence
- no unrestricted operator overloading

---

## 20. Boolean Operators

Fero uses:

- `&&`
- `||`
- `!`

Fero does not use:

- `and`
- `or`
- `not`

Example:

```fero
if ok && ready {
    run()
}
```

---

## 21. Commands

Commands are ordinary values of type `Command`.

```fero
let c = cmd("git", ["status"])
```

Fero does not define a separate shell grammar.

### Supported command operators

- `|`
- `&&`
- `||`

Examples:

```fero
cmd("cat", ["file.txt"]) | cmd("grep", ["error"])
cmd("build") && cmd("test")
cmd("fetch") || cmd("fallback")
```

### Redirection

Redirection is not operator-based in v0.1.

Use methods or APIs instead:

```fero
cmd("echo", ["done"]).append_to("build.log")
```

Rules:

- operators are used for simple chaining
- methods are used for detailed command configuration
- no shell DSL inside Fero

---

## 22. Imports

Supported import forms:

```fero
import std/fs
import std/process
import ./util
import pkg/json
```

Rules:

- `std/...` for standard library modules
- `./...` for relative imports
- `pkg/...` for package imports

Not part of v0.1 implementation:

- URL imports
- Wasm component imports in regular source syntax

---

## 23. Script Mode vs App Mode

### Script mode

- no required `main`
- top-level code is allowed

### App/package mode

- an entrypoint is required
- `main` is the default entrypoint

---

## 24. Lexer Requirements for v0.1

The lexer must support:

### Keywords

- `let`
- `var`
- `fn`
- `if`
- `else`
- `match`
- `for`
- `in`
- `struct`
- `enum`
- `extend`
- `import`
- `return`
- `break`
- `continue`
- `inout`

### Literals

- integer literals
- float literals
- string literals
- boolean literals

### Symbols

- `(`
- `)`
- `{`
- `}`
- `[`
- `]`
- `,`
- `.`
- `:`
- `=`
- `->`
- `=>`
- `..`
- `+`
- `-`
- `*`
- `/`
- `&&`
- `||`
- `!`
- `<`
- `>`
- `|`
- `>>`
- `_`

---

## 25. Parser Requirements for v0.1

The parser must support:

### Items

- `import`
- `fn`
- `struct`
- `enum`
- `extend`

### Statements

- `let`
- `var`
- expression statements
- `return`
- `break`
- `continue`
- `for`

### Expressions

- literals
- identifiers
- function calls
- member access
- blocks
- `if`
- `match`
- struct literals
- tuple literals
- array literals
- `for` expressions
- unary operators
- binary operators

---

## 26. Frozen Decisions

The following decisions are considered frozen for implementation start:

- `let` and `var`
- only one loop construct: `for`
- `if` and `match` are expressions
- `inout` for mutable passing
- struct fields immutable by default
- `var` for mutable struct fields
- one public `String` type
- explicit numeric types
- enum constructors inferred from context
- `Command` as a normal type
- command chaining through `|`, `&&`, `||`
- conversions via:
  - `Type(value)`
  - `Type.parse(text)`
  - `Type.try(value)`

---

## 27. Implementation Priority

Implementation should begin with:

1. lexer
2. parser
3. AST
4. HIR
5. type checking
6. MIR
7. Fero IR
8. interpreter

Not part of the first implementation pass:

- structural interfaces
- comptime
- effects system
- concurrency model
- URL imports
- Wasm component imports
- advanced operator overloading
