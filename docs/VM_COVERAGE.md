# Experimental VM Coverage Report

This document tracks the feature coverage of the experimental Tempra VM compared to the reference HIR interpreter.

## Supported Features

The following features are verified via differential testing (`differential_tests.rs`):

- **Arithmetic & Logic**: Full support for `Int64`, `Bool`, and basic math operators.
- **Control Flow**: `if/else` (including narrowing with `is`), `for` loops (iteration over `Array` and `Slice`), `for`-expressions (collecting results into arrays).
- **Functions**: Function calls with positional arguments, recursion (except some closure cases).
- **Structs**: Struct creation, member access, and mutable field assignment (`p.x = 30`).
- **Methods**: `extend` blocks on user-defined structs and methods with `self` receiver.
- **Fallible Operations**: `Result<T, E>` and `Option<T>` support, including the `?` propagation operator.
- **Standard Methods**: `.len()` on Arrays/Slices/Strings, `.size()` and `.bytes()` on Strings.
- **Type Narrowing**: `if x is Some(val)` style narrowing and payload unwrapping.
- **Caching**: Bytecode caching is implemented and verified for integrity and invalidation.

## Known Limitations (VM)

The following features are currently **unsupported** or have known issues in the experimental VM:

- **`match` expressions**: Not yet supported in TIR lowering. Use `if x is Variant` as a temporary workaround.
- **Closures**: Basic closures might work, but recursive closures or complex captures are largely untested or known to fail in the driver's TIR lowerer.
- **String Iteration**: `for ch in string` is not yet supported in VM.
- **Generic Methods**: Only simple methods on concrete or fully resolved struct types are supported.
- **Complex Pattern Matching**: Multi-payload variants or nested patterns are not yet supported.
- **CLI Arguments**: The VM runner does not yet pass CLI arguments to `main`.

## Known Issues (Reference/HIR)

- **Recursive Closures**: The HIR interpreter currently suffers from stack overflow on some recursive closure tests (e.g., `closure_tests.tmpr`). This is a pre-existing issue in the reference implementation and unrelated to the VM.

## Verification Status

All 14 differential tests and cache lifecycle tests are passing as of May 12, 2026.
