# Tempra Standard Library

The Tempra standard library (`std`) provides a foundational set of cross-platform APIs. It is implemented natively in Rust within the `tempra-native` crate and exposed via `extern` function declarations in `.tmpr` files located in the `/std` directory.

> [!NOTE]
> **Status:** MVP / Stable-enough. The current API surface is minimal to stabilize the module graph and native execution boundaries. It is not complete and will be expanded in future phases.

## Core Modules

### `std.fs`
File system operations. Fallible APIs return `Result<T, Error>`.

- `std.fs.exists(path: String) -> Bool`
- `std.fs.read_text(path: String) -> String!` — returns `Ok(String)` or `Err(String)`
- `std.fs.write_text(path: String, text: String) -> Unit!` — returns `Ok(())` or `Err(String)`

```tempra
import std.fs

pub fn main() -> Int64 {
    // Infallible existence check
    let exists = std.fs.exists("config.json")

    // Fallible read – returns Result<String, Error>
    let result = std.fs.read_text("data.txt")
    match result {
        Ok(text) => 0
        Err(_)   => 1
    }
}
```

### `std.path`
Path manipulation utilities.

- `std.path.join(a: String, b: String) -> String`
- `std.path.basename(path: String) -> String`

```tempra
import std.path
import std.io
let full = std.path.join("folder", "file.txt")
std.io.println(std.path.basename(full))
```

### `std.strings`
String manipulation utilities.

- `std.strings.contains(text: String, needle: String) -> Bool`
- `std.strings.starts_with(text: String, prefix: String) -> Bool`
- `std.strings.ends_with(text: String, suffix: String) -> Bool`
- `std.strings.trim(text: String) -> String` *(Phase 19.2A)*
- `std.strings.replace(text: String, from: String, to: String) -> String` *(Phase 19.2A)*

```tempra
import std.strings

let greeting = std.strings.trim("  hello world  ")   // "hello world"
let updated  = std.strings.replace("foo-bar", "-", "_")  // "foo_bar"
let has_space = std.strings.contains("hello world", " ")
```

### `std.io`
Basic input/output operations.

- `std.io.print(value: String)`
- `std.io.println(value: String)`

```tempra
import std.io
std.io.print("Status: ")
std.io.println("Running")
```

## v0.1 Collections API Status

This table freezes the implemented v0.1 collection API surface independently of the current
capability, ownership, concurrency, and backend roadmap. Entries marked eager execute immediately; helpers returning arrays allocate a new
`Array` result unless noted otherwise.

| Receiver | API | Implemented | Eager | VM-supported | LSP-visible | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| `Array(T)`, `Slice(T)` | `.len() -> Int64` | Yes | Yes | Yes | Yes | Number of elements. |
| `Array(T)`, `Slice(T)` | `.is_empty() -> Bool` | Yes | Yes | Yes | Yes | True when length is zero. |
| `Array(T)`, `Slice(T)` | `.first() -> Option(T)` | Yes | Yes | Yes | Yes | `None` for empty collections. |
| `Array(T)`, `Slice(T)` | `.last() -> Option(T)` | Yes | Yes | Yes | Yes | `None` for empty collections. |
| `Array(T)`, `Slice(T)`, `String` | `.iter()` | Yes | Iterator protocol | Yes | Typechecked | `String.iter()` is byte-oriented and returns `SliceIter(UInt8)`. |
| `ArrayIter(T)`, `SliceIter(T)` | `.next() -> Option(T)` | Yes | Stepwise | Yes | Typechecked | Used by `for` lowering and eager helpers. |
| `Array(T)`, `Slice(T)` | `.count() -> Int` | Yes | Yes | Yes | Yes | Lowered through `iter()` / `next()`. |
| `Array(T)`, `Slice(T)` | `.map(fn(T) -> U) -> Array(U)` | Yes | Yes | Yes | Yes | Direct top-level callback only. |
| `Array(T)`, `Slice(T)` | `.filter(fn(T) -> Bool) -> Array(T)` | Yes | Yes | Yes | Yes | Direct top-level callback only. |
| `Array(T)`, `Slice(T)` | `.find(fn(T) -> Bool) -> Option(T)` | Yes | Yes | Yes | Yes | First matching element. |
| `Array(T)`, `Slice(T)` | `.any(fn(T) -> Bool) -> Bool` | Yes | Yes | Yes | Yes | True if any element matches. |
| `Array(T)`, `Slice(T)` | `.all(fn(T) -> Bool) -> Bool` | Yes | Yes | Yes | Yes | True if all elements match. |
| `Array(T)`, `Slice(T)` | `.for_each(fn(T) -> Unit) -> Unit` | Yes | Yes | Yes | Yes | Direct top-level callback only. |
| `Array(T)`, `Slice(T)` | `.reduce(fn(T, T) -> T) -> Option(T)` | Yes | Yes | Yes | Yes | `None` for empty collections. |
| `Array(T)`, `Slice(T)` | `.fold(init: U, fn(U, T) -> U) -> U` | Yes | Yes | Yes | Yes | Returns `init` for empty collections. |
| `Array(T)`, `Slice(T)` | `.enumerate() -> Array((Int64, T))` | Yes | Yes | Yes | Yes | Index starts at `0`. |
| `Array(T)`, `Slice(T)` | `.zip(other: Array(U) \| Slice(U)) -> Array((T, U))` | Yes | Yes | Yes | Yes | Stops at the shortest input. |
| `Array(T)`, `Slice(T)` | `.take(count: Int64) -> Array(T)` | Yes | Yes | Yes | Yes | `count <= 0` returns empty; oversized count returns the full collection. |
| `Array(T)`, `Slice(T)` | `.skip(count: Int64) -> Array(T)` | Yes | Yes | Yes | Yes | `count <= 0` returns the full collection; oversized count returns empty. |
| `Array(T)`, `Slice(T)` | `.chain(other: Array(T) \| Slice(T)) -> Array(T)` | Yes | Yes | Yes | Yes | Preserves left-to-right order and always allocates a new array. |
| `String` | `.bytes() -> Slice(UInt8)` | Yes | Yes | Yes | Typechecked | UTF-8 byte view. |
| `std.strings` | `contains`, `starts_with`, `ends_with`, `trim`, `replace` | Yes | Yes | Yes | Module-visible | Native stdlib string helpers. |

### v0.1 Collection Limitations

- No lazy iterators, generators, `yield`, or `collect()`.
- No `flat_map()`, `flatten()`, `chunks()`, or `windows()`.
- No `Map` or `Set` collection types yet.
- No function values or closure callbacks for collection helpers.
- Functional helpers accept direct top-level function references only; callbacks must be resolvable
  at compile time.
- Eager helpers that produce collections allocate `Array` outputs.

Reduction semantics:

```tempra
[].reduce(add)        // None
[5].reduce(add)       // Some(5)
[1, 2, 3].reduce(add) // Some(6)

[].fold(100, add)        // 100
[1, 2, 3].fold(0, add)   // 6
```

```tempra
let xs: [Int64] = [10, 20, 30]

if xs.is_empty() {
    // ...
}

match xs.first() {
    Some(v) => v
    None    => 0
}

match xs.last() {
    Some(v) => v
    None    => 0
}
```

## Native Error Convention

The standard library adheres to strict error handling conventions to ensure stability across the Tempra-to-Rust native boundary:

1. **No Panics**: Native Rust functions (`extern` implementations) must **never** panic. Any panic in a native function will crash the VM.
2. **Result Enum**: Fallible native APIs return `VmValue::Enum("Result", "Ok"/"Err", payload)` which maps to Tempra `Result<T, Error>`. Pattern match on `Ok(...)` / `Err(...)` to handle failures.
3. **Non-fallible APIs**: Boolean or value-returning APIs that cannot fail (e.g. `exists`, `contains`) return plain values directly.
4. **`BuiltinError`**: The internal `Error` type in `Result<T, Error!>` signatures is lowered as `String` in TIR, carrying the error message.

## Planned after v0.1

- `Map`
- `Set`
- `flat_map`
- `flatten`
- `chunks`
- `windows`
- Lazy iterators
- Generators / `yield`
- Closure and function-value callbacks

Capability classification of native functions will be introduced by Phase E. Until then, this document describes API behavior but does not assign normative capability summaries. In particular, allocation classifications must be confirmed against the current runtime representation rather than inferred only from a return type.

## Legacy Code

> [!WARNING]
> The older `tempra-stdlib` crate (if present) is **deprecated** and considered legacy. All new standard library development should occur in `tempra-native` and the `std/` directory.
