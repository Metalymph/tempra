# Tempra Standard Library

The Tempra standard library (`std`) provides a foundational set of cross-platform APIs. It is implemented natively in Rust within the `tempra-native` crate and exposed via `extern` function declarations in `.tmpr` files located in the `/std` directory.

> [!NOTE]
> **Status:** MVP / Stable-enough. The current API surface is minimal to stabilize the module graph and native execution boundaries. It is not complete and will be expanded in future phases.

## Core Modules

### `std.fs`
File system operations.
- `std.fs.exists(path: String) -> Bool`

```tempra
import std.fs
let has_config = std.fs.exists("config.json")
```

*Note*: Read/write operations (`read_text`, `write_text`) are currently deferred until native `Result` extern support is fully implemented.

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

```tempra
import std.strings
let is_greeting = std.strings.starts_with("hello world", "hello")
let has_space = std.strings.contains("hello world", " ")
let is_txt = std.strings.ends_with("file.txt", ".txt")
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

## Native Error Convention

The standard library adheres to strict error handling conventions to ensure stability across the Tempra-to-Rust native boundary:

1. **No Panics**: Native Rust functions (`extern` implementations) must **never** panic. Any panic in a native function will crash the VM.
2. **Runtime Failures**: If an operation fails in a way that should halt execution (or isn't catchable in current Tempra semantics) or targets an unknown native target, it returns `VmError::Native(String)`.
3. **Future `Result` Design**: In the future, operations that can fail gracefully (like file I/O) will return a Tempra `Result<T, E>`. Native functions will construct the `Enum` representation of `Result` to pass the success or error state back to Tempra code. Until `Result` mapping across the FFI boundary is finalized, fallible APIs are deferred.

## Legacy Code

> [!WARNING]
> The older `tempra-stdlib` crate (if present) is **deprecated** and considered legacy. All new standard library development should occur in `tempra-native` and the `std/` directory.
