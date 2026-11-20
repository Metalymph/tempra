# Tempra Grammar v0.1 (Parser-Oriented)

This document defines a practical, parser-friendly grammar for Tempra v0.1.

It is not a formal EBNF, but a structured specification designed for implementing a recursive descent parser.

---

## 1. File Structure

```text
File
  = Item*
```

```text
Item
  = NamedFunction
  | ("pub")? ItemKind

ItemKind
  = Import
  | Struct
  | Enum
  | Extend
  | Interface
```

---

## 2. Imports

```text
Import
  = "import" ImportPath

ImportPath
  = Identifier ("." Identifier)*
  | "." Identifier ("." Identifier)*
```

Examples:

```tempra
import math
import net.http
import .helper
```

---

## 3. Functions

```text
NamedFunction
  = CapabilityAttribute* ("pub")? Function

CapabilityAttribute
  = DenyAttribute | PureAttribute

PureAttribute
  = "@" "pure"

DenyAttribute
  = "@" "deny" "(" DenyName ("," DenyName)* (",")? ")"

DenyName
  = "alloc" | "panic" | "io" | "process" | "spawn" | "nondeterminism"

Function
  = "fn" Identifier "(" ParamList? ")" ReturnType? Block

ParamList
  = GenericParam ("," GenericParam)* ("," Param)*
  | Param ("," Param)*

GenericParam
  = "type" Identifier

Param
  = "comptime" Identifier ":" Type
  | ("inout")? Identifier ":" Type

ReturnType
  = "->" Type

Closure
  = "fn" ("[" CaptureList "]")? "(" ParamList? ")" ReturnType? Block

CaptureList
  = CaptureItem ("," CaptureItem)*

CaptureItem
  = "self"
  | CaptureMode Identifier

CaptureMode
  = "ref" | "inout" | "consume"
```

Example:

```tempra
fn add(a: UInt64, b: UInt64) -> UInt64 {
    a + b
}
```

---

## 4. Struct

```text
Struct
  = "struct" Identifier ("(" GenericParamList? ")")? "{" StructField* "}"

GenericParamList
  = GenericParam ("," GenericParam)*

StructField
  = ("var")? Identifier ":" Type ("=" Expression)?
```

Example:

```tempra
struct User {
    name: String
    var bank_value: UInt64 = 0
}
```

---

## 5. Enum

```text
Enum
  = "enum" Identifier ("(" GenericParamList? ")")? "{" EnumVariant* "}"

EnumVariant
  = Identifier ("(" TypeList? ")")?
```

```text
TypeList
  = Type ("," Type)*
```

Example:

```tempra
enum Result {
    Ok(String)
    Err(String)
}
```

---

## 6. Interface

```text
Interface
  = "interface" Identifier "{" InterfaceItem* "}"

InterfaceItem
  = InterfaceMethod
  | InterfaceAssociatedType

InterfaceMethod
  = "fn" Identifier "(" ParamList? ")" ReturnType?

InterfaceAssociatedType
  = "type" Identifier
```

Example:

```tempra
interface Display {
    fn display(self) -> String
}

interface Iterator {
    type Item
    fn next(self) -> Option(Item)
}
```

---

## 7. Extend

```text
Extend
  = "extend" Type "{" NamedFunction* "}"
```

Example:

```tempra
extend User {
    fn is_rich(self) -> Bool {
        self.bank_value > 100000
    }
}
```

---

## 8. Block

```text
Block
  = "{" Statement* "}"
```

---

## 9. Statements

```text
Statement
  = LetStmt
  | VarStmt
  | ReturnStmt
  | BreakStmt
  | ContinueStmt
  | ForStmt
  | ExpressionStmt
```

### Let / Var

```text
LetStmt
  = "let" Identifier "=" Expression

VarStmt
  = "var" Identifier "=" Expression
```

### Return

```text
ReturnStmt
  = "return" Expression?
```

### Break / Continue

```text
BreakStmt
  = "break"

ContinueStmt
  = "continue"
```

### Expression Statement

```text
ExpressionStmt
  = Expression
```

---

## 10. For

```text
ForStmt
  = "for" Identifier "in" Expression ForFilter? Block

ForFilter
  = "if" Expression
```

Example:

```tempra
for x in items if x > 0 {
    io.println(x)
}
```

---

## 11. Expressions Overview

```text
Expression
  = Assignment
```

---

## 12. Assignment

```text
Assignment
  = LogicOr ("=" Assignment)?
```

---

## 13. Logical Operators

```text
LogicOr
  = LogicAnd ("||" LogicAnd)*

LogicAnd
  = Equality ("&&" Equality)*
```

---

## 14. Equality & Comparison

```text
Equality
  = Comparison (("==" | "!=" | "is") Comparison)*

Comparison
  = Bitwise (("<" | ">" | "as?") Bitwise)*
```

---

## 15. Bitwise / Shift

```text
Bitwise
  = Additive (">>" Additive)*

Pipe
  = Bitwise ("|" Bitwise)*
```

NOTE:

- `|` is also used for Command pipelines
- precedence must be lower than arithmetic, higher than logical OR

---

## 16. Arithmetic

```text
Additive
  = Multiplicative (("+" | "-") Multiplicative)*

Multiplicative
  = Unary (("*" | "/") Unary)*
```

---

## 17. Unary

```text
Unary
  = "comptime" Expression
  | ("!" | "-") Unary
  | Call
```

---

## 18. Call / Access

```text
Call
  = Primary (CallSuffix)*

CallSuffix
  = "(" ArgList? ")"
  | "[" TypeList "]" "(" ArgList? ")"  // Explicit Specialization
  | "." Identifier
  | "[" Expression "]"
  | "[" Expression ".." Expression "]"
```

```text
ArgList
  = Expression ("," Expression)*
```

---

## 19. Primary

```text
Primary
  = Literal
  | Identifier
  | Tuple
  | Array
  | Block
  | IfExpr
  | MatchExpr
  | ForExpr
  | StructLiteral
  | Closure
```

---

## 20. Literals

```text
Literal
  = Integer
  | Float
  | String
  | "true"
  | "false"
```

---

## 21. Tuple

```text
Tuple
  = "(" Expression "," Expression ("," Expression)* ")"
```

---

## 22. Array

```text
Array
  = "[" Expression ("," Expression)* "]"
```

---

## 23. Struct Literal

```text
StructLiteral
  = Identifier "{" StructInitField* "}"

StructInitField
  = Identifier ":" Expression
  | ".." Identifier
```

Example:

```tempra
User {
    name: "Lorenzo",
    bank_value: 50000,
}
```

---

## 24. If Expression

```text
IfExpr
  = "if" ("comptime")? Expression Block ("else" (Block | IfExpr))?
```

---

## 25. Match Expression

```text
MatchExpr
  = ("comptime")? "match" Expression "{" MatchArm* "}"

MatchArm
  = Pattern "=>" Expression
```

---

## 26. Patterns (v0.1 minimal)

```text
Patterns
  = Pattern ("|" Pattern)*

Pattern
  = "_"
  | Identifier
  | Literal
  | Identifier "(" PatternList? ")"
  | Identifier "{" StructPatternField* "}"
  | TuplePattern

StructPatternField
  = Identifier (":" Pattern)?

TuplePattern
  = "(" Pattern "," Pattern ("," Pattern)* ")"
```

```text
PatternList
  = Pattern ("," Pattern)*
```

---

## 27. For Expression

```text
ForExpr
  = "for" Identifier "in" Expression ForFilter? Block
```

Same syntax as statement, but used as expression.

---

## 28. Types

```text
Type
  = TypeAtom ("?" | "!" TypeAtom?)?

TypeAtom
  = Identifier ("(" TypeList? ")")?  // Named or Type Constructor (e.g. Array(Int))
  | ArrayType                        // [T] Sugar
  | TupleType
  | FunctionType
  | "Never"
  | "Self"
  | Identifier "." Identifier  // Projection (e.g. T.Output)

FunctionType
  = "fn" "(" TypeList? ")" ReturnType?
```

```text
ArrayType
  = "[" Type "]"
  | "[" Type ";" Integer "]"
```

```text
TupleType
  = "(" Type ("," Type)* ")"
```

---

## 29. Command Expressions

```text
CommandExpr
  = "cmd" "(" Expression ("," Expression)* ")"
```

Command values integrate into normal expressions.

Operators:

- `|`
- `&&`
- `||`

Example:

```tempra
cmd("cat", ["file.txt"]) | cmd("grep", ["error"])
```

---

## 30. Operator Precedence (High → Low)

1. Call / indexing / member access
2. Unary (`!`, `-`)
3. `*` `/`
4. `+` `-`
5. `>>`
6. `|`
7. `<` `>`
8. `==` `!=`
9. `&&`
10. `||`
11. `=`

---

## 31. Parsing Notes

- use recursive descent
- avoid backtracking where possible
- use lookahead for:
  - struct literal vs block
  - tuple vs grouped expression
- prefer explicit parse functions:
  - `parse_expression`
  - `parse_block`
  - `parse_for`
  - `parse_struct`
- keep AST simple in v0

---

## 32. Minimal AST Nodes (Suggested)

- File
- Import
- Function
- Struct
- Enum
- Interface
- Extend
- Block
- Let / Var
- Return
- For
- If
- Match
- BinaryOp
- UnaryOp
- Call
- MemberAccess
- Index
- Slice
- Literal
- Identifier
- Tuple
- Array
- StructLiteral
- CommandExpr

---

## 33. Historical Bootstrap Strategy

The original parser bootstrap proceeded in this order:

1. lexer (tokens + spans)
2. parser (AST only, no types)
3. minimal evaluator / interpreter
4. incremental feature completion

The following items were intentionally deferred during that bootstrap:

- full pattern matching
- advanced type system
- generics
- capability analysis

This list is historical, not the current roadmap. E3.1 now accepts `@deny` on
named top-level functions and `extend` methods, before optional `pub`.
Each `@deny` attribute needs a capability name; duplicate names and repeated
`@deny` attributes merge.
`unsafe`, unknown names, other attributes and unsupported placement are errors.
Externs, closures, interface requirements, fields and non-function items cannot
carry capability attributes. E3.1 acquires deny intent; E3.2 enforces contracts in VM compilation and rejects
annotated execution on non-capability-aware paths. Emission and frontend-only
`check` do not certify contracts. E4 accepts bare `@pure` in the same positions,
composable with deny in either order. Parentheses/arguments and duplicate pure
markers are errors (the second marker is reported). `pure` remains an identifier
outside attribute position. `@unsafe` is not accepted; it belongs to standalone
U — Unsafe Boundary (DEFERRED / CONDITIONAL), not the immediate Phase E sequence.
Its trigger is the first real source operation requiring explicit memory/type
safety authorization. This adds no grammar production.
