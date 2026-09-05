# Bytecode Cache Format (v0)

This document specifies the binary cache format for Tempra bytecode artifacts.

## Overview

The bytecode cache allows saving and loading pre-compiled and verified bytecode modules to disk. This speeds up the execution pipeline by bypassing the parsing, typechecking, and lowering phases for unmodified source files.

**WARNING:** The v0 format does NOT provide a stable ABI. It is tightly coupled to the compiler version.

## Architecture

A cache artifact consists of a **Header** and a **Module**.

### Header (`BytecodeCacheHeader`)

The header ensures that the cached bytecode is compatible with the current runtime and source file.

- **Magic (4 bytes)**: Always `TMBC` (Tempra Bytecode Cache).
- **Format Version (u32)**: Version of the binary format (currently 2).
- **Compiler Fingerprint (String)**: `tempra-{CARGO_PKG_VERSION}-semantics-2`, identifying the compiler and semantic guarantees required for reuse.
- **Source Hash (32 bytes)**: A non-cryptographic `DefaultHasher` result, repeated to fill 32 bytes. This is a source identity hint, not SHA-256, an integrity check or a security primitive.
- **Language Version (String)**: Minimum Tempra language version required.
- **Stdlib ABI Version (String)**: Identifier for the standard library ABI.

### Serialization Strategy

For the v0 implementation, serialization is manual:
- Integers are encoded in Little-Endian (LE).
- Strings are prefixed with their length as a `u32` (LE), followed by UTF-8 bytes.
- Enums use a 1-byte (`u8`) tag followed by their payload.
- No compression or variable-length integer encoding is used.

### Verification Policy

The decoding function (`decode_artifact`) does **not** automatically verify the bytecode. This design allows tooling (like disassemblers or debuggers) to inspect potentially invalid bytecode.

However, before execution, the VM **MUST** use the `decode_and_verify_artifact` convenience function or manually call `verify_module` after decoding.

## Integration

The cache format is integrated into the Tempra CLI via the `--experimental-vm` flag.

- **Cache Directory**: Artifacts are stored in `.tempra/cache/` (relative to the current working directory).
- **Cache Hit**: A cache artifact is reused if it exists and its header precisely matches the current compiler version and source file hash.
- **Cache Miss/Corruption**: Missing, incompatible or unverifiable artifacts cause fresh compilation. Replacement and execution occur only if compilation, contract enforcement and bytecode verification succeed. A failed compilation leaves any existing artifact unchanged.

### Semantic validity (E3.2 and K1.7)

`semantics-N` versions the guarantees required to trust compiler-produced cache
artifacts; it is independent of bytecode serialization format. The bytecode
format version governs structural decoding, while the semantic revision governs
the compiler guarantees that make decoded bytecode reusable. Their numeric values
are unrelated even when both happen to be `2`.

Revision `1` was introduced by E3.2 and required `@deny` enforcement before
bytecode lowering and cache writing. K1.7 raises the current revision to `2` so
artifacts predating K1's panic-free arithmetic, explicit bodyless rejection,
canonical effect policy and deterministic termination limits cannot bypass fresh
comptime evaluation. Increase this revision whenever older readable artifacts no
longer establish the required compiler guarantees.

Pre-E3.2 fingerprints (`tempra-{version}`) and `semantics-1` artifacts do not
match the current fingerprint and are ignored. Both
ordinary and profiled source VM execution use one shared validity path. A hit
requires a matching source hash and semantic fingerprint and verified bytecode.
There is no TIR reconstruction, analysis, or synthetic capability product on a hit.
No contracts, capabilities, witnesses or source spans are serialized.

Fresh `compile_to_bytecode_with_analysis` always enforces contracts before writing;
multi-module compilation still has no cache. Source changes, including attributes,
participate in the existing hash identity. An obsolete artifact need not be deleted:
if fresh enforcement fails it remains an incompatible miss, never a usable new entry.
This policy assumes a local compiler-owned cache; headers are not authenticated
against deliberate modification and the existing source hash is non-cryptographic.

Historically, E4 (`@pure`) did not increment revision `1`: it used the same E3
contract enforcement, and pre-E4 compilers rejected that attribute syntax.
Adding, removing or changing pure changes source bytes and cache identity.
Pure metadata is not serialized; successful pure compilation can populate/hit
the same bytecode-only cache, while a failed contract cannot write a replacement.
K1.7 is the later, independent revision to `semantics-2`; it changes cache trust,
not bytecode serialization.

## Future Plans

- **Dependency Tracking**: Extend the cache format to handle multi-file dependencies (e.g., if an imported module changes, invalidate the cache).
- **Compression**: Add simple compression (like LZ4) to reduce disk I/O overhead.
