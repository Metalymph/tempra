# Tempra Standard Library

The Tempra standard library (`std`) provides a foundational set of cross-platform APIs. It is implemented natively in Rust within the `tempra-native` crate and exposed via `extern` function declarations in `.tmpr` files located in the `/std` directory.

## Core Modules

### `std.fs`
File system operations.
- `std.fs.exists(path: String) -> Bool`

*Note*: Read/write operations (`read_text`, `write_text`) are currently deferred until native `Result` extern support is fully implemented.

### `std.path`
Path manipulation utilities.
- `std.path.join(a: String, b: String) -> String`
- `std.path.basename(path: String) -> String`

### `std.strings`
String manipulation utilities.
- `std.strings.contains(text: String, needle: String) -> Bool`
- `std.strings.starts_with(text: String, prefix: String) -> Bool`
- `std.strings.ends_with(text: String, suffix: String) -> Bool`

### `std.io`
Basic input/output operations.
- `std.io.print(value: String)`
- `std.io.println(value: String)`

## Native Error Convention

The standard library adheres to strict error handling conventions to ensure stability across the Tempra-to-Rust native boundary:

1. **No Panics**: Native Rust functions (`extern` implementations) must **never** panic. Any panic in a native function will crash the VM.
2. **Runtime Failures**: If an operation fails in a way that should halt execution (or isn't catchable in current Tempra semantics), it returns `VmError::Native(String)`.
3. **Future `Result` Design**: In the future, operations that can fail gracefully (like file I/O) will return a Tempra `Result<T, E>`. Native functions will construct the `Enum` representation of `Result` to pass the success or error state back to Tempra code. Until `Result` mapping across the FFI boundary is finalized, fallible APIs are deferred.

## Legacy Code

> [!WARNING]
> The older `tempra-stdlib` crate (if present) is **deprecated** and considered legacy. All new standard library development should occur in `tempra-native` and the `std/` directory.
