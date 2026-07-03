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

## Array / Slice Methods
*(Phase 19.2A — available via method-call syntax on arrays and slices)*

- `.len() -> Int64` — number of elements
- `.count() -> Int` — number of elements, lowered through `iter()` / `next()`
- `.is_empty() -> Bool` — true if len == 0
- `.first() -> Option<T>` — `Some(first_element)` or `None` if empty
- `.last() -> Option<T>` — `Some(last_element)` or `None` if empty
- `.map(fn(T) -> U) -> Array<U>` — eager transform into a new array
- `.filter(fn(T) -> Bool) -> Array<T>` — eager filter into a new array
- `.find(fn(T) -> Bool) -> Option<T>` — first matching element
- `.any(fn(T) -> Bool) -> Bool` — true if any element matches
- `.all(fn(T) -> Bool) -> Bool` — true if all elements match
- `.for_each(fn(T) -> Unit) -> Unit` — eager side-effect loop
- `.reduce(fn(T, T) -> T) -> Option<T>` — eager left reduction; `None` for empty collections
- `.fold(init: U, fn(U, T) -> U) -> U` — eager left fold starting from `init`

Functional helpers currently accept direct top-level function references only. Inline closures,
stored callbacks, function values, lazy iterators, and generators are intentionally out of scope.

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

## Legacy Code

> [!WARNING]
> The older `tempra-stdlib` crate (if present) is **deprecated** and considered legacy. All new standard library development should occur in `tempra-native` and the `std/` directory.
