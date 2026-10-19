# Tempra Bytecode ISA Specification (v0.1)

This document defines the Instruction Set Architecture (ISA) for the Tempra stack-based virtual machine.

## 1. Introduction

Tempra uses a stack-based bytecode for its interpreted execution mode. This mode is optimized for rapid cold-starts, low memory overhead, and development workflows (scripting and compile-time execution).

### Objectives
- **Simplicity**: Easy to implement and verify.
- **Portability**: Agnostic of the underlying hardware.
- **Verifiability**: Designed for strict structural and type validation before execution.
- **Cacheability**: Support for serializing compiled bytecode to disk to skip re-compilation.

### Non-Goals
- **Highest possible performance**: AOT (native) compilation remains the target for high-performance production workloads.
- **Register VM**: While register-based VMs can be faster, a stack-based model is simpler for the initial bootstrap and `comptime` migration.

### Pipeline
The compilation pipeline follows these steps:
`Source Code` -> `HIR (Typed)` -> `TIR (Tempra IR)` -> `Bytecode` -> `Stack VM`

---

## 2. Architecture & ABI Stability

### Stack-Based Model
The VM operates on a data stack. Instructions push operands onto the stack and pop them to produce results. Local variables are stored in a separate indexed table (frame) for efficiency.

### ABI Stability Rules
> [!IMPORTANT]
> **v0 bytecode is NOT a stable ABI.**
> 
> The instruction set, binary format, and runtime semantics are subject to change without notice. Cache artifacts are strictly tied to the compiler version and source hash; they may be invalidated or deleted by the compiler at any time.

### Instruction Encoding (v0)
- **Format**: Opcode followed by operands.
- **Fixed-width**: Operands use fixed-width encoding (e.g., 4 or 8 bytes) where practical in v0 to simplify decoding.
- **Optimizations**: Compact encodings (variable-length, bit-packing) are deferred to later phases.

### Call Convention (v0)
1. **Arguments**: Pushed onto the data stack in **left-to-right** order.
2. **Execution**: The `Call` instruction transfers control to the callee.
3. **Consumption**: The callee pops its arguments from the stack upon entry (or accesses them via local slots mapped to the stack).
4. **Return**: The return value (if any) is pushed back onto the data stack by the `Return` instruction before exiting the callee.

---

## 3. Data Model

### Module Structure
A compiled Tempra module (`.tmprc` or internal representation) consists of:
- **Magic Number**: `0x544D5052` ("TMPR")
- **Version**: Compiler version identifier.
- **Fingerprint**: Fingerprint of the compiler and feature set.
- **Constant Pool**: A table of shared constants (Strings, large numbers).
- **Function Table**: Metadata and entry points for all functions in the module.
- **Bytecode Vec**: The actual instruction stream.

### Function Structure
Each function definition includes:
- **Name**: For debugging and lookup.
- **Signature**: Argument types and return type.
- **Locals Count**: Number of slots reserved for parameters and local variables.
- **Max Stack**: The peak height of the data stack during execution. 
    - *Note*: This can be derived by the lowerer or verifier, but it should be explicitly stored in the serialized cache for fast loading.
- **Code**: Range or offset in the module's bytecode vector.
- **Debug Info**: (Optional) Mapping of bytecode offsets to source spans.

### Value Model
The VM handles the following primitive types directly:
- **Unit**: Represents `()`.
- **Bool**: `true` or `false`.
- **Int64**: 64-bit signed integer.
- **UInt64**: 64-bit unsigned integer.
- **UInt8**: 8-bit unsigned integer (often used for bytes).
- **Float64**: 64-bit IEEE-754 floating point.
- **String**: Reference to a string (interned or heap-allocated).
- **Composite/Placeholder**: (Future) Metadata for structs, enums, and arrays.

---

## 4. Instruction Set (ISA)

### Constants
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `ConstUnit` | - | `[] -> [unit]` | Pushes the unit value. |
| `ConstBool` | bool | `[] -> [bool]` | Pushes a boolean literal. |
| `ConstI64` | i64 | `[] -> [i64]` | Pushes a 64-bit signed integer literal. |
| `ConstU64` | u64 | `[] -> [u64]` | Pushes a 64-bit unsigned integer literal. |
| `ConstF64` | f64 | `[] -> [f64]` | Pushes a 64-bit float literal. |
| `ConstString` | idx | `[] -> [string]` | Pushes a string from the constant pool. |

### Locals
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `LoadLocal` | index | `[] -> [T]` | Pushes value from local slot `index`. |
| `StoreLocal` | index | `[T] -> []` | Pops value and stores it in local slot `index`. |

### Stack Manipulation
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `Pop` | - | `[T] -> []` | Discards the top value. |
| `Dup` | - | `[T] -> [T, T]` | Duplicates the top value. |

### Arithmetic (Int64)
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `AddI64` | - | `[i64, i64] -> [i64]` | Integer addition. |
| `SubI64` | - | `[i64, i64] -> [i64]` | Integer subtraction. |
| `MulI64` | - | `[i64, i64] -> [i64]` | Integer multiplication. |
| `DivI64` | - | `[i64, i64] -> [i64]` | Integer division. |

These opcodes use checked Tempra `Int64` semantics. Addition, subtraction and
multiplication overflow fail with `Int64ArithmeticError::Overflow`; division by
zero fails with `DivisionByZero`, and `Int64::MIN / -1` is overflow. The VM and
native JIT must return the same controlled failure. The bytecode encoding is
unchanged by this contract.

### Arithmetic (UInt64)
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `AddU64` | - | `[u64, u64] -> [u64]` | Unsigned addition. |
| `SubU64` | - | `[u64, u64] -> [u64]` | Unsigned subtraction. |
| `MulU64` | - | `[u64, u64] -> [u64]` | Unsigned multiplication. |
| `DivU64` | - | `[u64, u64] -> [u64]` | Unsigned division. |

### Arithmetic (Float64)
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `AddF64` | - | `[f64, f64] -> [f64]` | Floating point addition. |
| `SubF64` | - | `[f64, f64] -> [f64]` | Floating point subtraction. |
| `MulF64` | - | `[f64, f64] -> [f64]` | Floating point multiplication. |
| `DivF64` | - | `[f64, f64] -> [f64]` | Floating point division. |

### Comparison & Logic
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `Eq` | - | `[T, T] -> [bool]` | Structural equality check. |
| `LtI64` | - | `[i64, i64] -> [bool]` | Signed less-than. |
| `GtI64` | - | `[i64, i64] -> [bool]` | Signed greater-than. |
| `Not` | - | `[bool] -> [bool]` | Boolean negation. |
| `And` | - | `[bool, bool] -> [bool]` | Boolean AND. |
| `Or` | - | `[bool, bool] -> [bool]` | Boolean OR. |

### Control Flow
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `Jump` | offset | `-` | Unconditional jump to bytecode offset. |
| `JumpIfFalse` | offset | `[bool] -> []` | Jump if top of stack is `false`. |
| `Return` | - | `[ret?] -> []` | Return from function. |
| `Unreachable` | - | `-` | Trap the VM. |

### Calls
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `Call` | fn_idx, argc| `[args...] -> [ret?]` | Calls function by index in function table. |

### Enums
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `MakeEnum` | enum, var, cnt| `[payloads...] -> [enum]` | Creates an enum instance with `cnt` payloads. |
| `Tag` | - | `[enum] -> [string]` | Extracts the variant name (tag) from an enum. |
| `Unwrap` | variant, index | `[enum] -> [T]` | Extracts the `index`-th payload of a specific variant. |

### Collections & Strings
| Opcode | Operands | Stack Effect | Description |
|---|---|---|---|
| `MakeArray` | count | `[args...] -> [array]` | Pops `count` elements and creates an array. |
| `MakeTuple` | count | `[args...] -> [tuple]` | Pops `count` elements and creates a tuple. |
| `GetIndex` | - | `[object, index] -> [T]` | Accesses an element by dynamic index. |
| `Slice` | - | `[object, start, end] -> [slice]` | Creates a view into an array or slice. |
| `Len` | - | `[object] -> [i64]` | Retrieves count of elements (Array/Slice) or chars (String). |
| `Size` | - | `[str] -> [i64]` | Retrieves UTF-8 byte size of String. |
| `Bytes` | - | `[str] -> [slice]` | Retrieves a Slice(UInt8) view of String bytes. |

---

## 5. Bytecode Verifier Specification

Before execution, the VM must verify the bytecode to ensure safety:
1. **Instruction Validity**: All opcodes must be valid and operands must be within bounds (e.g., local index, constant pool index).
2. **Jump Safety**: All jump targets must point to the beginning of a valid instruction.
3. **Stack Consistency**: 
    - The stack height at any instruction must be the same regardless of the path taken to get there.
    - The stack must be empty (or contain only the return value) at `Return`.
    - Stack depth must never exceed `Max Stack`.
4. **Type Safety**: Operands on the stack must match the types expected by the opcode (e.g., `AddI64` requires two `i64` values).
5. **Initialization**: Local variables must be initialized before they are loaded via `LoadLocal`.
6. **Arity**: `Call` must push exactly the number of arguments required by the function's signature.

---

## 6. Cache Policy

To improve startup performance, Tempra caches compiled bytecode in `.tempra/cache/`.

### Cache Metadata
- **Compiler Fingerprint**: Ensures cache is not used by a different version of the compiler.
- **Source Hash**: Ensures cache matches the current source code.
- **Feature Flags**: ABI version and enabled language features.

### Invalidation
- Caches are automatically invalidated if the source hash or compiler version changes.
- Users can clear the cache manually if needed.
- In v0, invalidation is aggressive to prevent stale ABI issues.

---

## 7. Roadmap

- **Phase 13.0**: ISA Draft (Current).
- **Phase 13.1**: `tempra-bytecode` crate data model.
- **Phase 13.2**: TIR → Bytecode lowering.
- **Phase 13.3**: Bytecode Verifier implementation.
- **Phase 13.4**: Minimal Stack VM.
- **Phase 13.5**: Bytecode serialization and cache integration.
- **Phase 13.6**: Micro-optimizations (super-instructions, dispatch speed).

---

## 8. Examples

### Simple Addition
**Source:**
```tempra
fn add(a: Int64, b: Int64) -> Int64 {
    return a + b
}
```

**TIR (Simplified):**
```
fn add (%0: i64, %1: i64) -> i64 {
  slot $0: i64
  store $0, %0
  slot $1: i64
  store $1, %1
  %2 = load $0
  %3 = load $1
  %4 = add %2, %3
  ret %4
}
```

**Bytecode:**
```
0: LoadLocal 0  // push parameter 'a'
1: LoadLocal 1  // push parameter 'b'
2: AddI64       // pop a, b -> push result
3: Return       // pop result and return
```

### Conditional Logic
**Source:**
```tempra
fn max(a: Int64, b: Int64) -> Int64 {
    if a > b {
        return a
    } else {
        return b
    }
}
```

**Bytecode:**
```
0: LoadLocal 0  // a
1: LoadLocal 1  // b
2: GtI64        // a > b
3: JumpIfFalse 7 // Jump to else if false
4: LoadLocal 0  // return a
5: Return
6: Jump 9       // Not reachable in this case, but standard
7: LoadLocal 1  // return b
8: Return
```
