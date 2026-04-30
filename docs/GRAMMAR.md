# Fero Grammar v0.1 (Parser-Oriented)

This document defines a practical, parser-friendly grammar for Fero v0.1.

It is not a formal EBNF, but a structured specification designed for implementing a recursive descent parser.

---

## 1. File Structure

```text
File
  = Item*
```

```text
Item
  = Import
  | Function
  | Struct
  | Enum
  | Extend
```

---

## 2. Imports

```text
Import
  = "import" ImportPath

ImportPath
  = Identifier ("/" Identifier)*
  | "." "/" Identifier ("/" Identifier)*
```

Examples:

```fero
import std/fs
import std/process
import ./util
import pkg/json
```

---

## 3. Functions

```text
Function
  = "fn" Identifier "(" ParamList? ")" ReturnType? Block

ParamList
  = GenericParam ("," GenericParam)* ("," Param)*
  | Param ("," Param)*

GenericParam
  = "type" Identifier

Param
  = ("inout")? Identifier ":" Type

ReturnType
  = "->" Type
```

Example:

```fero
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

```fero
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

```fero
enum Result {
    Ok(String)
    Err(String)
}
```

---

## 6. Extend

```text
Extend
  = "extend" Type "{" Function* "}"
```

Example:

```fero
extend User {
    fn is_rich(self) -> Bool {
        self.bank_value > 100000
    }
}
```

---

## 7. Block

```text
Block
  = "{" Statement* "}"
```

---

## 8. Statements

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

## 9. For

```text
ForStmt
  = "for" Identifier "in" Expression ForFilter? Block

ForFilter
  = "if" Expression
```

Example:

```fero
for x in items if x > 0 {
    io.println(x)
}
```

---

## 10. Expressions Overview

```text
Expression
  = Assignment
```

---

## 11. Assignment

```text
Assignment
  = LogicOr ("=" Assignment)?
```

---

## 12. Logical Operators

```text
LogicOr
  = LogicAnd ("||" LogicAnd)*

LogicAnd
  = Equality ("&&" Equality)*
```

---

## 13. Equality & Comparison

```text
Equality
  = Comparison (("==" | "!=" | "is") Comparison)*

Comparison
  = Bitwise (("<" | ">" | "as?") Bitwise)*
```

---

## 14. Bitwise / Shift

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

## 15. Arithmetic

```text
Additive
  = Multiplicative (("+" | "-") Multiplicative)*

Multiplicative
  = Unary (("*" | "/") Unary)*
```

---

## 16. Unary

```text
Unary
  = ("!" | "-") Unary
  | Call
```

---

## 17. Call / Access

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

## 18. Primary

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
```

---

## 19. Literals

```text
Literal
  = Integer
  | Float
  | String
  | "true"
  | "false"
```

---

## 20. Tuple

```text
Tuple
  = "(" Expression "," Expression ("," Expression)* ")"
```

---

## 21. Array

```text
Array
  = "[" Expression ("," Expression)* "]"
```

---

## 22. Struct Literal

```text
StructLiteral
  = Identifier "{" StructInitField* "}"

StructInitField
  = Identifier ":" Expression
  | ".." Identifier
```

Example:

```fero
User {
    name: "Lorenzo",
    bank_value: 50000,
}
```

---

## 23. If Expression

```text
IfExpr
  = "if" Expression Block ("else" Block)?
```

---

## 24. Match Expression

```text
MatchExpr
  = "match" Expression "{" MatchArm* "}"

MatchArm
  = Pattern "=>" Expression
```

---

## 25. Patterns (v0.1 minimal)

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

## 26. For Expression

```text
ForExpr
  = "for" Identifier "in" Expression ForFilter? Block
```

Same syntax as statement, but used as expression.

---

## 27. Types

```text
Type
  = Identifier ("(" TypeList? ")")?  // Named or Type Constructor
  | ArrayType
  | TupleType
  | "Never"
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

## 28. Command Expressions

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

```fero
cmd("cat", ["file.txt"]) | cmd("grep", ["error"])
```

---

## 29. Operator Precedence (High → Low)

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

## 30. Parsing Notes

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

## 31. Minimal AST Nodes (Suggested)

- File
- Import
- Function
- Struct
- Enum
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

## 32. Implementation Strategy

Start with:

1. lexer (tokens + spans)
2. parser (AST only, no types)
3. minimal evaluator / interpreter
4. incremental feature completion

Do NOT implement early:

- full pattern matching
- advanced type system
- generics
- effects system

Focus on correctness and clarity first.
