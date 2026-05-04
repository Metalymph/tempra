# Tempra Syntax Specification v0.1

Tempra syntax is designed to be direct and readable, taking inspiration from Swift, Rust, and Go. It balances the ergonomics of a scripting language with the rigor of a compiled language.

## 1. Core Principles
- **Small Surface Area:** Minimize the number of keywords and control flow constructs.
- **Expression-Oriented:** Most blocks (if, match, for) evaluate to a value.
- **Explicit over Implicit:** Explicit error handling and type annotations where clarity is required.

## 2. Variables and Bindings
Variables are immutable by default.
```tempra
let name = "Tempra"      // Immutable
let mut count = 0      // Mutable
count = 1
```

## 3. Functions
Functions use the `fn` keyword. Parameters and return types are required in package mode, optional but encouraged in script mode.
```tempra
fn calculate(x: Int, y: Int) -> Int {
    x + y
}
```

## 4. Control Flow

### If Expressions
`if` is an expression and must handle all branches if used as one.
```tempra
let status = if count > 0 { "Active" } else { "Idle" }
```

### Match Expressions
Match provides simple pattern matching for values and enums.
```tempra
match result {
    Ok(val) => print(val)
    Err(e)  => print("Error: " + e)
}
```

### For Statements and Expressions
Tempra uses a single versatile `for` loop.
```tempra
// Statement
for x in items {
    print(x)
}

// Expression with filter
let doubled = for x in items if x % 2 == 0 {
    x * 2
}
```

## 5. Compound Types

### Structs
Simple groupings of data.
```tempra
struct Point {
    x: Int
    y: Int
}

let p = Point { x: 10, y: 20 }
```

### Enums (ADTs)
Enums support payloads, acting as Algebraic Data Types.
```tempra
enum Shape {
    Circle(Float)
    Rect(Int, Int)
    Point
}
```

## 6. Errors
Tempra does not have exceptions. Errors are explicit values, typically returned via a `Result` enum pattern.
```tempra
fn read_config() -> Result<String, Error> {
    let content = fs.read_file("config.tempra")? // '?' propagates error
    Ok(content)
}
```

## 7. Interfaces (Static Duck Typing)
Interfaces define requirements without explicit implementation markers. If a type matches the signature, it satisfies the interface.
```tempra
interface Writer {
    write(data: Bytes) -> Int
}
```

## 8. Mode Differences
- **Script Mode:** Allows top-level expressions. No `main` required. Intended for one-off tasks and automation.
- **App Mode:** Requires a `main` function as the entrypoint. Enforces stricter module boundaries for binary compilation.
