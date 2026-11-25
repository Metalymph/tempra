# Tempra — Surface Syntax Specification v0.2

**Date:** 15 November 2026
**Status:** Revised language-surface design specification. The memory semantics referenced here are **approved** by Memory & Ownership v0.4; this revision **does not** claim that all syntax and checks are implemented in the compiler.
**Supersedes:** `TEMPRA_SURFACE_SPEC_v0.1.md` as the active design reference once adopted in `docs/`.
**Companion documents:** [Memory & Ownership v0.4](Tempra_Memory_Ownership_v0.4.md), [Grammar](GRAMMAR.md), [Comptime](COMPTIME_SPEC.md), [Capability Contracts](specs/effects-v0.1.md), [Specialized Effects v0.2](specs/specialized-effects-v0.2.md).

### Normative precedence and compatibility

1. **Memory & Ownership v0.4 is the sole normative authority for memory semantics:** ownership, value copying, nonduplicable resources, `ref` / `inout` / `consume`, borrow duration, closure captures, async boundaries, resource cleanup, and backend parity. This document does not independently redefine those rules.
2. This Surface Specification describes the intended source language and preserves existing documented syntax. Where the spelling of an extension is *not* yet approved or parser-verified, it is marked **SURFACE OPEN**, rather than silently introduced as valid Tempra.
3. `GRAMMAR.md` is a parser-oriented snapshot, not proof of current branch implementation. It requires a separate reconciliation after the surface review. Some examples below are explicitly **semantic pseudocode**.
4. `COMPTIME_SPEC.md` and the effect specifications provide the detailed operational contracts for their respective domains. The current historical comptime HIR implementation is not evidence that the planned VM implementation has already passed parity.
5. **Status vocabulary:** **DOCUMENTED** = previously specified source form; **APPROVED SEMANTICS** = normative language behavior from Memory v0.4; **SURFACE OPEN** = exact spelling/grammar or declaration mechanism not approved; **IMPLEMENTATION TO VERIFY** = source behavior must be checked against the actual compiler and tests. These labels are intentionally distinct.

### v0.2 reconciliation summary

- Align `Slice(T)` to read-only non-owning views and `inout Slice(T)` to exclusive mutable access; retain syntax for mutable owner indexing without turning a read-only view into an implicit mutable borrow.
- Specify `Range` as a standalone half-open `[start, end)` descriptor and preserve bounds checks at application time.
- Formalize ordinary by-value arguments versus explicit `consume` and the distinction between semantic consumption and last-use *physical* move optimization.
- Preserve the existing closure grammar `fn[ref x]`, `fn[inout x]`, `fn[consume x]`, and recursive `fn[self]`, with no default capture; apply non-escape and NO-CYCLE restrictions.
- Unify method receiver behavior with ordinary parameters, while leaving unverified receiver/parameter spellings explicitly open.
- Document async suspension, actor/task isolation, nonduplicable resources and safe FFI at the semantic level; do not invent a complete actor API or an unsafe syntax.
- Unify generic type-constructor notation (`Type(T)`) with `T?` / `T!E` sugar, while preserving explicit function specialization `f[T](...)`.
- Retain existing numeric, error, comptime, interfaces, modules, commands, hooks and effect syntax without inventing features absent from the current documents.
- Remove obsolete assumptions that Cranelift JIT is a forward backend strategy. **Target:** Tempra Script stack VM for scripting and Tempra Native LLVM AOT. JIT and Cranelift retirement is an architecture/repository migration, not yet proof of source-code removal.

## 1. Design goals

Tempra focuses on:

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

The design preference is **one semantic model** across named functions, methods, closures, generics, tasks, `comptime`, Tempra Script and Tempra Native. An optimization may change the physical execution, never the source-level ownership contract.

## 2. Core language rules

- Immutable bindings by default; `var` for mutable bindings.
- No null references, general exception handling, explicit lifetime syntax or implicit coercions.
- One loop family: `for`, usable as statement or expression.
- `if` and `match` are expressions; blocks may return their final expression.
- Primary algebraic compound types: `struct`, `enum`, tuples, arrays and slices.
- Static structural interfaces for generic constraints; not runtime existential/vtable interfaces in v1.
- No general object-reference model. Mutable access is explicitly controlled by `inout` and source ownership.
- Script mode needs no `main`; app mode requires `main` and follows the separately documented entry/result contract.
- All ordinary source code is safe by default. Unsafe/FFI extensions are not granted implicitly.

**Scripting-first target:** Tempra Script uses an optimized stack bytecode VM with ARC NO-CYCLE as the intended owned-value memory management. **Native target:** Tempra Native uses LLVM AOT, with zero GC/RC a goal requiring implementation and evidence. These goals are not claims of completed production support.

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

`let` does **not** mean static storage, and `var` does **not** authorize aliasing incompatible with a live loan. Value independence, moves and nonduplicable resources follow the Memory Specification.

## 4. Named functions and generics

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

### Parameter modes (semantic contract)

| Parameter form | Contract | Source status |
|---|---|---|
| `value: T` | By-value; semantically independent copy when `T` is duplicable | **DOCUMENTED** |
| `inout value: T` | Non-owning exclusive mutable access | **DOCUMENTED** |
| `ref value: T` | Non-owning read access | **APPROVED SEMANTICS; SURFACE OPEN** for named parameter spelling/parser |
| `consume value: T` | Explicit ownership transfer; source becomes unusable | **APPROVED SEMANTICS; SURFACE OPEN** outside closure captures |

The grammar snapshot currently documents ordinary parameters and `inout`; it does **not** certify `ref` or `consume` parameter spellings. The table records the semantic requirement, not a parser acceptance claim. The exact surface forms need a joint parser/grammar decision.

A normal by-value argument does **not** semantically consume its source. The compiler may physically move its storage at proven last use, or copy a small aggregate into registers, when equivalent. A nonduplicable resource cannot be passed via an implicit semantic consume; a transfer or borrowed-access contract is mandatory.

Generic specialization must preserve these rules for the concrete `T`, including nested `struct`, `enum`, `Array(T)` and closure environments. `type T` does not imply that arbitrary values of `T` are duplicable. The source syntax for expressing additional copyability requirements is **SURFACE OPEN**.

## 5. Comptime

Tempra supports explicit compile-time evaluation via `comptime`. The documented implementation evaluates a restricted, deterministic language subset during type checking and materializes supported results as HIR constants.

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

### Items historically pending in the v0.1 snapshot — verify against current branch

- Generated HIR specialization / runtime erasure of comptime parameters
- Comptime IO, resources, or build configuration
- Complex patterns in `comptime match` (constructors, tuples, structs)

### Ownership and execution authority

`comptime` is an evaluation *phase*, not an ownership or capability exception. It does not make a dynamic local `Static`, permit borrowed values to escape, duplicate nonduplicable resources or waive an effect prohibition. The current `COMPTIME_SPEC.md` documents a canonical HIR-interpreter authority and deterministic budget; the intended transition to VM execution must preserve type/ownership checks, capability restrictions, progress budget, determinism, and cache trust. **VM comptime parity is not yet certified by this document.**

## 6. Static interfaces and iterator protocol

### Static duck interfaces (documented Phase 6)

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

### Iterator protocol

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

### Ownership compatibility of structural satisfaction

Interface satisfaction also requires matching access mode, copyability, return provenance, callback escape and capability contracts. It cannot convert a borrowed result into an owned result or make an affine resource duplicable. All constraints remain static; no runtime vtables are introduced to enforce memory semantics. A surface-level copyability constraint syntax is not yet agreed.

## 7. Blocks

```tempra
{
    let x = 10
    let y = 20
    x + y
}
```

Blocks are expressions.

## 8. If expression

```tempra
let result = if ok {
    "done"
} else {
    "failed"
}
```

The checker analyzes conditional branches separately. Borrowed values surviving a join must have compatible, demonstrable provenance, as specified in Memory v0.4. An `if` expression does not extend the lifetime of a dynamic owner.

## 9. Match expression and patterns

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

`match` arms are separate control-flow paths. A borrowed result from multiple independent dynamic owners is rejected under the v1 conservative join rules. An enum/ADT decision can describe which owner to access later without creating a multi-owner reference.

## 10. For statements and expressions

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

A dynamic borrow created inside an iteration must end before the next iteration unless the path exits immediately under a valid return/break contract. An existing compatible read borrow may span the loop; a loop does not permit carrying a dynamic slice to another iteration. `for-in` may entail an internal iterator loan checked under the same rules.

## 11. Structs, construction and updates

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

### Owned and borrowed fields

A struct is duplicable only if its components permit duplication. A struct containing a nonduplicable resource cannot be implicitly copied by construction, call, return or update. `..source` follows the same value semantics; it is not a secret ownership-transfer syntax.

A **temporary, non-escaping** local struct may contain `Slice(T)` tied to one dynamic owner, subject to borrow liveness and exclusive access. Such an aggregate may be used by a synchronous non-escaping call but cannot be stored as an independent persistent relationship, captured by an escaping closure, returned as an aggregate borrowed in v1, or carried across `.await()` with a dynamic loan active. The syntax of field declarations does not create lifetime parameters.

```tempra
struct PacketWindow {
    data: Slice(UInt8)
    payload: Range
}

let window = PacketWindow {
    data: buffer[0..16],
    payload: 2..12,
}
inspect_window(window)
```

*Semantic example from Memory v0.4; implementation of aggregate borrow/escape checking remains to be verified.*

## 12. Enums and generic constructors

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
let r: String! = Ok("done")
```

Rules:

- **Construction**: Enum constructors are resolved contextually.
- **Lexical Priority**: If a name matches a variable or function in scope, it is NOT treated as a constructor. Shadowing is explicit.
- **Ambiguity**: If multiple enums share a variant name, the `expected` type (from function signatures or explicit context) is used to disambiguate. If ambiguity remains, it is a type error.
- **Payload Arity**: v1 supports 0 or 1 positional payload components.
- **Pattern Matching**: Enum variants can be matched in `match` and `is`. Payloads are bound to identifiers or matched with nested patterns.

Enum payload ownership, destruction, copyability and borrowing are derived from the active variant. Pattern matching cannot duplicate a nonduplicable payload or make a returned dynamic view independent of its owner.

## 13. Tuples

```tempra
let pair = (name, age)
let (name, age) = get_user()
```

Rules:

- tuples are positional only
- no named tuples
- tuples do not replace structs

A tuple containing borrowed data inherits its provenance; a tuple containing a nonduplicable resource cannot be implicitly duplicated. Tuple destructuring follows the same ownership/access contracts as any other aggregate.

## 14. Arrays, Slice and Range

### Arrays

```tempra
let xs: Array(Int) = [1, 2, 3]
let ys: [Int] = [1, 2, 3] // shorthand for Array(Int)
```

- `Array(T)` is an owning dynamic collection; `[T]` is the documented array type sugar.
- `[]` requires an expected element type; a non-returning element expression does not infer an array element type alone.
- Copying an `Array(T)` is a semantic value copy only when its element type is duplicable. The backend may use copy, COW where valid, move and reuse without changing observed independence.
- When `T` is nonduplicable, copying the whole collection cannot create duplicate ownership of its elements.
- `.len()` and `.is_empty()` are read methods on arrays and slices; the precise concrete integer type of indices is subject to grammar/typechecker reconciliation.

### Slice(T) — immutable borrowed view

```tempra
let part: Slice(Int) = xs[1..3]
```

`Slice(T)` is **contiguous, read-only, non-owning**, and does not prolong the lifetime of its buffer. Access `part[i]` reads; assignment through `part[i]` is **not** an implicit mutable slice operation. This replaces the v0.1 statement that all slices are read/write views.

The compiler tracks `Static`, `Borrowed(owner)` or `Temporary(scope)` provenance. A dynamic slice can be returned only when its provenance is tied to a single identifiable parameter and the caller's owner remains valid. There are no explicit lifetime type parameters and no persistent dynamic borrowed fields in independent objects.

### Mutation: `inout` access, not a mutable slice type

```tempra
fn normalize(inout data: Slice(UInt8)) {
    // Mutate elements through this exclusive access.
}

var bytes: Array(UInt8) = [1, 2, 3, 4]
normalize(inout bytes[0..2])
```

`inout Slice(T)` grants an **exclusive temporary access to the underlying elements**, not just permission to reassign a slice descriptor. The underlying owner must actually be mutable. A simultaneous conflicting loan or alias is rejected; general interval-disjointness proofs are not required in v1. No public `MutableSlice(T)` type is introduced.

Built-in assignment `bytes[i] = value` or `bytes[a..b] = source` on a `var` owning collection remains an owner mutation subject to the same exclusivity checks. It does **not** turn a freely stored `Slice(T)` into a mutable reference. For slice-range assignment, the documented v0.1 length compatibility check still applies; bounds and failures must be controlled.

### Range — independent interval descriptor

```tempra
let selection = 2..5   // [2, 5)
let view = bytes[selection]
```

A `Range` denotes **`[start, end)`**, including `start` and excluding `end`. It contains no buffer reference and can be stored, returned or passed independently. Bounds are checked when applying it to a collection, not merely when constructing it. The concrete numeric index type and final indexing API remain to be reconciled with the parser/typechecker; `bytes[selection]` is an intended Range-application form, not a separate ownership mechanism.

`Range` is appropriate for persistent descriptions of parts of an owned buffer; it is not a universal replacement for `Slice(T)`. For example an owned `Array(T)` plus a `Range` is a valid long-lived representation, whereas a naked dynamic slice is not.

### Borrow control flow

Borrows terminate after the last reachable use on each control-flow path (NLL-like). Branches, `match`, loops and function calls follow Memory v0.4. The compiler never inserts a hidden owned copy to repair an illegal borrowed escape. `ref`, `inout` and slice access may not remain live across an async suspension.

## 15. String and UTF-8

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

`String.bytes()` returns a read-only borrowed slice and does not make its backing string static. The result is subject to the same provenance, last-use and escape rules as any other `Slice(UInt8)`. Mutating a string is not authorized through `bytes()`.

## 16. Option, Result, Never and type-position sugar

### Option

Tempra supports an explicit optional model:

- semantic type: `Option(T)`
- surface sugar in type position: `T?` (equivalent to `Option(T)`)
- constructors in expression position:
  - `Some(value)`
  - `None`

Examples:

```tempra
fn first(xs: [Int]) -> Int? {
    None
}

let x: Int? = Some(1)
```

Rules:

- `T?` is type-position sugar only
- `Some(value)` constructs `Option(T)` from `value: T`
- `None` is the empty option value
- the optional sugar does not add `if let`, `guard`, or Option helper methods
- `expr?` propagation is for `Result` and is not extended to `Option` by `T?`

### Result sugar

Tempra supports concise type-position sugar for `Result`:

- `T!E` is sugar for `Result(T, E)`
- `T!` is sugar for `Result(T, Error)`

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

- `T?` in type position: `Option(T)`
- `T!E` / `T!` in type position: `Result(T, E)` / `Result(T, Error)`
- `expr?` in expression position: Result propagation
- `expr!` in expression position: unwrap/crash behavior

### Never

`Never` is the bottom type in Tempra. It represents computations that never produce a value (e.g., those that crash or exit).

Rules:
- No runtime values of type `Never`.
- `crash(...)` and `exit(...)` return `Never`.
- `Never` is compatible with any expected type.
- Branch unification: `T + Never => T`.
- A `match` where all arms return `Never` is itself type `Never`.

Generic *type constructors* use `Option(T)` / `Result(T, E)` notation consistent with other type constructors; `T?` and `T!E` are their surface sugars. Explicit specialization of a generic *function call* still uses `f[T](...)`. An untyped `Some(...)` or `Ok(...)` expression must not be presented as automatically resolving an otherwise ambiguous generic enum. `Result` values participate in the planned strict value-use discipline; no new exception or implicit discard mechanism is defined here.

`T!` without explicit error argument uses the documented default `Error` type; expression `?` is `Result` propagation, not the `Option` sugar.

## 17. `is` operator and branch narrowing

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

## 18. Numeric types and checked arithmetic

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

A potentially failing numeric operation contributes its conservative capability requirements. No optimizer-specific proof may silently erase a declared `@deny(panic)` violation. `Int` appears in many existing examples; the definitive alias/default-width mapping must be checked against the implementation rather than inferred from those examples.

## 19. Access modes, value passing and ownership

### Ordinary values

Unqualified parameters and receivers are by-value. For duplicable types, the caller's value remains valid by contract. Physical copying, move at proven last use, register passing, COW and allocation reuse are backend choices; there is no source-level size threshold that automatically changes a value parameter into a borrowed one.

### `ref` and `inout`

- `ref`: read-only, non-owning access with checked provenance and escape.
- `inout`: exclusive, non-owning, mutable access. `var` is necessary for external mutable capture, but does not override an active incompatible borrow.
- The existing `inout value: T` declaration and `inout expression` call form remain documented.
- The memory semantics of `ref` are approved, but its **named-parameter surface spelling** remains **SURFACE OPEN** pending parser/grammar verification. Neither `&T` nor Rust-style `&mut T` is adopted.

```tempra
fn increment(inout value: UInt64) {
    value = value + 1
}
```

### `consume`: semantic transfer, not inferred consumption

`consume` invalidates the source **semantically** and transfers ownership. It is already documented in a closure capture (`fn[consume value]`); it is required conceptually for explicit transfer of a nonduplicable parameter or receiver, but exact general-purpose parameter/receiver syntax is **SURFACE OPEN**. Do not assume an unqualified by-value call can consume a unique resource because it happens to be the last use.

The future reconciliation may decide a general parameter production covering `ref` / `inout` / `consume`, but this document does not silently declare one parser-accepted.

### Borrow liveness and escape

Borrow termination is flow-sensitive. Accesses cannot outlive dynamic owners, cross `.await()` while live, be stored as persistent dynamic references, or be hidden behind a generic, closure or temporary aggregate to escape. Borrowed returns require a single inferable owner, subject to the stricter rule for aggregate returns in Memory v0.4.

## 20. Methods, `extend` and `self` receivers

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

### Receiver contract

`self` is a parameter, not an independent memory model. Documented receiver forms are `self` (by value) and `inout self` (exclusive mutable access). By-value `self` on duplicable types preserves the caller's value, with physical move/reuse possible at proven last use.

`ref self` (read-only loan) and `consume self` (semantic owner transfer) are **APPROVED SEMANTICS** but their exact surface/parser support is **SURFACE OPEN**. In particular, no implicit `ref self` is inferred for read-only-looking methods. Current `fn display(self)` and `fn get(self, ...)` examples are **by-value**, not hidden-reference syntax.

A `Reader` or `Writer` holding a nonduplicable handle cannot be semantically copied by an ordinary by-value method call. An async operation that needs it after suspension must acquire ownership via an explicit transfer contract, not retain `inout self` across `.await()`. Any `consume self` API example is illustrative until the grammar is approved and implemented.

The receiver named `self` is unrelated to the special `fn[self]` recursive-closure capture. **Grammar mismatch to fix:** the historical parser-oriented `GRAMMAR.md` describes `StructField*` but does not include method items in its `Struct` production, even though the Surface Spec documents in-type methods. This mismatch cannot be resolved by assuming either side has already been implemented.

## 21. Errors and explicit propagation

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

`Result` is an owned/borrowed value like any other: error propagation, early returns and pattern bindings must preserve resource ownership and cleanup. `expr?` propagates a `Result` according to its public contract. No exception-based path silently bypasses `consume` or releases borrowed storage.

## 22. Conversions and casts

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

Conversions do not imply that an arbitrary borrowed value becomes an independent owned value. A conversion that materializes ownership, duplicates dynamic storage or consumes a unique resource must obey the Memory Specification and associated effects.

## 23. Operator overloading

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

Operators on nonduplicable values must not secretly duplicate ownership. The precise callable signatures and access modes for overloaded operators require the same verification as named methods.

## 24. Boolean operators and pipelines

Tempra uses:

- `&&`
- `||`
- `!`

Tempra does not use:

- `and`
- `or`
- `not`

`|` is also used for command pipelines. Neither the use of `&&`/`||` on `Command` values nor command composition authorizes implicit I/O outside the established effect contracts.

## 25. Closures, explicit captures and recursion

A closure is a first-class callable value consisting conceptually of code and an environment. Its public callable type is `fn(Params...) -> Return`; its concrete storage strategy is not part of the surface contract.

### Existing source syntax

```tempra
let add_one = fn(x: Int) -> Int {
    x + 1
}

let offset = 10
let add_offset = fn[ref offset](x: Int) -> Int {
    offset + x
}

var total = 0
let accumulate = fn[inout total](x: Int) {
    total = total + x
}

let data = load_data()
let process_data = fn[consume data]() {
    process(data)
}
// 'data' is no longer available here.
```

The grammar is the existing `fn[CaptureItem, ...](...)` form. **No default capture exists.** No keyword `captures` is introduced. All external names referenced must be explicitly captured with a mode. `fn(...)` has an empty environment; `fn[]` is forbidden.

| Capture | Public contract | Constraints |
|---|---|---|
| `ref x` | Read-only dynamic/static loan | Dynamic captures must be non-escaping; source owner stays valid |
| `inout x` | Exclusive mutable loan | Source must be mutable; dynamic captures must be non-escaping; exclusivity throughout |
| `consume x` | Semantic ownership transfer | Source invalidated immediately on closure creation; environment obeys copyability/NO-CYCLE |
| `self` | Special recursive identity | Not a capture mode; must not produce a strong self-cycle |

`fn[consume x]` does **not** mean copy-and-keep-source. In contrast, ordinary parameters of a closure follow by-value semantics unless their access contract explicitly says otherwise. Optimizing those calls using a last-use physical move never changes public behavior.

### Captured borrowing, escape and callback summaries

Borrowed dynamic captures are legal only if the closure is proven non-escaping relative to the loan. A closure that is returned, persistently stored, moved to an independent task or passed to a callback parameter with unknown escape is treated as escaping. A proven static immutable capture may persist. A `consume` environment can escape when owned-value, cycle and resource requirements permit it.

The compiler may infer modular non-escape/callback summaries; unknown callback behavior is conservatively escaping. No user-visible lifetime generic or extra callable trait taxonomy is introduced. Copying a closure that contains nonduplicable resources is forbidden; invocation/reuse is subject to the resource ownership contract, not a new memory mode.

### Recursive `fn[self]`

```tempra
let factorial = fn[self](n: Int) -> Int {
    if n <= 1 {
        1
    } else {
        n * self(n - 1)
    }
}
```

`fn[self]` gives explicit recursive identity. `self` inside the closure is an immutable special binding, cannot be shadowed, and is not available unless listed. The identity remains stable through calls and value passing; its implementation **must not form a strong ownership edge from the closure to itself**. The special recursion marker is distinct from method receiver `self` and not a fourth capture mode. Nested closures do not inherit recursive self automatically; explicit nested capture remains subject to provenance and escape.

Named function items have no lexical captures and may recurse normally. Closures are not comparable. Tail-call optimization is optional, not guaranteed. Closure allocation or non-allocation is backend dependent and participates in `@deny(alloc)` when it can require heap storage.

**Implementation to verify:** full closure lowering, stable recursive identity, mutable capture, callback escape summaries, NO-CYCLE and source-level diagnostic quality. The syntax being documented does not establish these proofs.

## 26. Commands

Commands are ordinary values of type `Command`.

```tempra
let c = cmd("git", ["status"])
```

### Supported command operators

- `|`
- `&&`
- `||`

Commands are ordinary values but their execution and process effects must respect `@deny(process)` and the operation-specific specialized-effect contracts. The exact effect of construction versus execution must follow the effect specs; creation of a command value is not implicitly a process launch.

## 27. Imports and module resolution

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

Import resolution must remain reproducible; no hidden current-working-directory stdlib lookup or implicit lifetime extension across module boundaries. Exported borrow and effect summaries are required for semantically checked imported calls.

## 28. Subscript hooks and indexing

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

### Documented v0.1 limitations (implementation to verify)
- **Nested Subscript Assignment:** `x[i][j] = v` is unsupported in v1 (requires a reference system).
- **String Indexing:** `s[i]` remains intentionally unsupported to avoid common UTF-8 pitfalls. Use `s.bytes()[i]` or explicit iteration.
- **No Protocols:** There are no `Indexable` or `Sliceable` interfaces yet. Hooks are resolved structurally by name.
- **No Associated Types:** The return type of `get` and the value type of `set` are independent but must be compatible for valid assignment.

### Memory alignment

Hooks use ordinary receiver semantics. `get(self, ...)` is by-value as written, while `set(inout self, ...)` and `set_slice(inout self, ...)` require exclusivity. A hook returning a dynamic `Slice(T)` must preserve an identifiable source owner; a by-value receiver does not automatically grant a lifetime for a returned borrow. The relationship between general `get`/`slice` hooks, source provenance and an eventual `ref self` form requires an implementation and grammar audit. Built-in array mutations must not be used to bypass exclusive borrowing.

## 29. Built-in interfaces

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

As with all interfaces, `self` in these documented signatures is by-value, not an implicit loan. A future read-only receiver syntax may improve efficiency but cannot be treated as already parser-accepted. Collection methods must respect element duplicability, provenance and capability constraints.

## 30. Capability contracts and effects

### Public attribute surface

The accepted capability-attribute surface documented by E3/E4 is limited to:

```tempra
@deny(io, process)
fn compute() -> Int {
    42
}

@pure
fn double(x: Int) -> Int {
    x * 2
}
```

- `@deny(...)` requires at least one **valid target**. Broad capabilities are `alloc`, `panic`, `io`, `process`, `spawn`, `nondeterminism`; repeated targets and repeated `@deny` attributes merge as documented, and a trailing comma is supported.
- The **Specialized Effects v0.2** contract additionally accepts families (`fs`, `net`), categories (`io.read`, `io.write`), concrete operations (`fs.write`, `net.connect`, `spawn.task`, etc.) and supported resource selectors. These are **not** new fundamental capabilities. `@deny(io.read)` denies the cross-family *read category*, while `@deny(io.read())` denotes the concrete generic I/O read operation. See the specialized specification for the complete canonical grammar and resolution precedence.

```tempra
@deny(fs, net)
fn offline_work() -> Int {
    42
}

@deny(fs.write(destination: "private.txt"))
fn read_only_report() -> Int {
    42
}
```

The two examples illustrate valid attribute target forms, not full coverage of operation-aware source enforcement.
- `@pure` is shorthand for `@deny(io, process, spawn, nondeterminism)`. **It does not prohibit allocation or panic** unless separately denied.
- Attributes occur before optional `pub` on named top-level functions and methods inside `extend`. They are not currently accepted on closures, extern declarations, interface requirements or fields.
- An unannotated function has no declared denial. Denial is checked against direct and transitively required capabilities, including concrete specializations.
- `@deny()` and `@deny(unsafe)` are invalid. `@unsafe` and `@verified` are not currently accepted as public source attributes; they are deferred designs, not existing permissions.
- The [Specialized Effects v0.2](specs/specialized-effects-v0.2.md) specification refines operation/family/category/resource restrictions within the compiler's current bounded surface. It does not create a second memory model.

### Allocation and cleanup under Memory v0.4

`@deny(alloc)` is conservative and **backend-independent** at the semantic boundary. Operations that may require dynamic storage to preserve independent value copies, materialize a buffer or create an escaping closure environment cannot be accepted merely because one optimizer sometimes removes their allocations. A pure function may allocate; a function denying allocations cannot depend on a non-portable elimination of required allocations.

A resource's cleanup contributes its actual capabilities (including I/O or other effects). Hidden lowering, runtime helpers and cleanup are **not** blanket exceptions to denied capability contracts. A dynamic allocation introduced purely to repair an escaping borrow is forbidden even in an unannotated function.

`@deny(panic)` requires a valid proof/contract for potentially failing bounds or arithmetic operations; optimization-dependent hope is not a proof. This document does not promise full effect coverage of every future TIR or runtime operation.

### Enforcement status and cache

The historical E1–E6 and K4 documents describe implemented, bounded analysis and enforcement; they are not evidence that future ownership checking, VM parity or native LLVM exists. Compiler semantic fingerprints and cache invalidation must reflect ownership, specialization and effect-contract changes. Cached bytecode cannot be accepted when its semantic revision cannot guarantee the current contracts. Do not treat the old Cranelift JIT as a current future backend or a fallback for capability enforcement.

For precise effect taxonomy, syntax grammar and implementation gates, consult `docs/specs/effects-v0.1.md`, `docs/specs/specialized-effects-v0.2.md` and the current roadmap.

## 31. Async, task and actor boundaries (semantic surface)

Memory v0.4 approves **ownership semantics** for concurrency, but this Surface Specification does **not** introduce a new comprehensive async/actor grammar, task-group API, actor-handle representation or scheduling policy.

### Suspension

- Owned values, `Range` and truly `Static` immutable views may remain available across `.await()`.
- A dynamic `ref`, `inout` or `Slice(T)` borrow **must not remain live across a suspension**; a borrow ended before suspension is valid.
- A task that needs to use a reader/writer or buffer after suspension must possess the state needed by its continuation. `inout self` is not automatically promoted into ownership.
- Retaking a fresh borrow *after* suspension is allowed when the owner remains valid; logical state may have changed during the pause.

### Spawn, messages and cancellation

- Spawned work may acquire independent owned values and proven static immutable access, not dynamic borrowed captures from the spawning frame, even for a structured task group in v1.
- Actor state is isolated, and messages crossing an actor boundary are independent owned values, never dynamic borrowed views into the sender.
- Cancellation is cooperative where the runtime requires it: resource/buffer cleanup waits until pending accesses or OS operations are safely completed/canceled/transferred.
- These safety rules do not imply deadlock freedom, actor operation atomicity across suspension or a particular actor reentrancy policy.

**DEFERRED POST NO-CYCLE:** actor handle identity, mailbox lifetime, strong/non-owning handle semantics and supervision. They must not be inferred from this document.

The spelling of future actor, task-group, spawn and await declarations/expressions remains governed by separate concurrency/grammar work. This section is intentionally not a parser production.

## 32. Nonduplicable resources, FFI and cleanup

A resource owning a file descriptor, socket or other externally managed handle is a **nonduplicable owned value**, not a special lifetime system. A plain integer representing such a handle is a duplicable integer; a wrapper responsible for releasing it is not implicitly duplicable. Composite structs, enums, arrays and closure environments inherit this restriction.

- Explicit `consume` transfers semantic ownership, invalidates the source binding, and prevents double ownership. Its general parameter/receiver spelling is not yet specified.
- A nonduplicable resource must not be copied by `f(value)`, `self` or struct update purely because its last use is known. Physical move optimization is not permission for an unmarked semantic transfer.
- A resource is released once after the last valid owner, including ordinary returns, controlled error paths and completed cancellations, after all outstanding accesses have ended. No guarantee of user cleanup after process kill/crash is implied.
- Automatically infallible cleanup and explicit fallible `close() -> Result(...)` are different API contracts. Cleanup cannot silently ignore an error that the public API requires to be reported, nor conceal forbidden effects.
- Raw FFI pointers do not become safe `Slice(T)`, `ref` or `inout` without established provenance, length, exclusivity and lifetime. The concrete unsafe boundary/FFI ABI is deferred. Safe wrappers must preserve the normal access and ownership contracts.
- Async native I/O may require runtime-owned pinned/stable storage while an operation is pending. This runtime detail does not allow source-level dynamic borrow across `.await()`.

**SURFACE OPEN:** public syntax/declaration mechanism for a nonduplicable type, parameter/receiver ownership transfer, raw pointers and audited unsafe operations. No new `resource`, `unique`, `move`, `lifetime`, `weak`, `shared` or `@unsafe` grammar is silently introduced here.

## 33. Compiler and backend obligations

The semantic pipeline is conceptually:

```text
Source → AST → typed HIR / ownership+effect checks → verified TIR
       → bytecode → Tempra Script stack VM
       ↘ future LLVM AOT → Tempra Native
```

This diagram is a target logical architecture, **not** an assertion that each path is already production-complete. The current implementation on the reviewed remote `main` historically retained a HIR interpreter and experimental bytecode VM; branch-local migrations must be verified against actual code before updating implementation claims.

- HIR/TIR must preserve independent value copies versus semantic consumption, loan provenance, exclusivity, last-use endings, callback escape, resource cleanup, suspend boundaries and modular effects.
- TIR verification should reject malformed ownership/control-flow states, including use-after-consume/drop and live dynamic borrows over suspend, without requiring a second full source checker.
- Cross-module summaries and specialization/cached artifacts must be stable, reproducible and invalidated by semantic contract changes.
- Tempra Script may implement owned values using ARC NO-CYCLE and must never use ARC as a workaround for an invalid source borrow.
- Tempra Native targets LLVM AOT with an **unproven** zero GC/RC objective. It may use physical copy/move/reuse, direct release and verified optimizations while preserving the identical source contract.
- Compile-time execution will move toward the VM only after verified parity for deterministic evaluation, capability restrictions, budget and memory semantics.
- The historical Cranelift JIT and Cranelift AOT direction are **retired design targets**. This does not claim that JIT/Cranelift crates, features, docs or tests have been physically removed from the repository already.

## 34. Conformance matrix and documentation follow-up

| Program or behavior | Required result / status |
|---|---|
| Unqualified `T` argument, duplicable, caller uses it afterward | Caller remains valid; independent value semantics |
| Unqualified `T` at proven last use | Same semantics; optimizer may physically move |
| `fn[ref x]` captures a dynamic local and closure does not escape | Allowed under provenance and liveness checks |
| Dynamic `ref` / `inout` capture in an escaping closure | Rejected |
| `fn[inout x]` where `x` is not mutable / exclusive | Rejected |
| `fn[consume x]` followed by outside use of `x` | Rejected |
| `fn[self]` recursion | Allowed only with no strong self-owned cycle |
| Implicit capture of external name | Rejected |
| Read-only `Slice(T)` assigned through as if mutable | Rejected |
| `inout Slice(T)` with incompatible live borrow | Rejected |
| Local non-escaping struct containing dynamic slice | Allowed with proven owner |
| Returning independent aggregate that embeds dynamic slice | Rejected in v1 |
| Dynamic borrow still live over `.await()` | Rejected |
| `Range` surviving without the buffer | Allowed; bounds checked when reapplied |
| Normal by-value duplication of nonduplicable resource | Rejected |
| Explicit ownership transfer | Approved semantics; general parameter spelling **SURFACE OPEN** |
| `@deny(alloc)` requiring potentially allocating independent copy | Rejected conservatively |
| Hidden dynamic copy solely to fix escaping borrow | Rejected |
| Actor handle memory/lifetime policy | **DEFERRED POST NO-CYCLE** |
| Unverified new grammar for `ref self`/`consume self` | Not asserted as implemented |

### Next implementation/docs audit (not source-language commitments)

1. Verify each documented production against the parser, lexer, type checker and representative compile tests on the **actual target branch**, not only the older `main` snapshot.
2. Reconcile `GRAMMAR.md` with all accepted forms, especially in-type methods, constrained generics (`type T: Interface`), `self` receivers, `ref`/`consume` parameter grammar, `inout` call syntax, `expr?` / `expr!`, pattern exhaustiveness, generic constructors, specialized `@deny` targets and source syntax for applying `Range`.
3. Ensure `COMPTIME_SPEC.md` preserves its semantic constraints when the canonical evaluator switches to the VM; remove references to the HIR interpreter only when that migration is complete.
4. Update docs for bytecode VM, TIR and architecture after VM parity and ownership implementation; remove/archival of JIT documents does not itself remove JIT code.
5. Propagate the LLVM-only AOT architecture and Memory v0.4 to the roadmap, stdlib/tutorials, tooling and cache specs. Audit docs links after moving historical files under `docs/archive/`.
6. Do not claim formal NO-CYCLE, runtime RC-free guarantees, complete resource cleanup effects or end-to-end VM parity without evidence and gates.

**Principle:** *Ownership to retain. Borrowing to access. Range to describe. ADTs to represent alternatives.* One memory model across the language; no implicit ownership recovery and no hidden new source syntax.
