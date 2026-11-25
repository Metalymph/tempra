# Tempra — Grammar v0.2 (Parser-Oriented)

**Date:** 15 November 2026
**Status:** Complete design-grammar revision; **not** a parser-conformance certificate.
**Replaces:** `docs/GRAMMAR.md` (v0.1 parser-oriented snapshot).
**Canonical source syntax:** [`TEMPRA_SURFACE_SPEC_v0.2.md`](TEMPRA_SURFACE_SPEC_v0.2.md).
**Sole normative memory authority:** [`Tempra_Memory_Ownership_v0.4.md`](Tempra_Memory_Ownership_v0.4.md).
**Other authorities:** [`COMPTIME_SPEC.md`](COMPTIME_SPEC.md), [`specs/effects-v0.1.md`](specs/effects-v0.1.md), [`specs/specialized-effects-v0.2.md`](specs/specialized-effects-v0.2.md), [`specs/pattern_matching_v1.md`](specs/pattern_matching_v1.md), [`specs/module-system-v0.1.md`](specs/module-system-v0.1.md).

> **Scope.** This document is a practical recursive-descent grammar for the **documented brace-based Tempra language**. It consolidates the approved Surface v0.2 and the earlier grammar, while identifying unapproved syntax and parser-support gaps explicitly. It does **not** define the Python-style indentation proposal, a new actor API, a resource-declaration keyword, or a new memory model. Syntax documented here may still require implementation or verification against the active compiler branch.

## 1. Authority, status and conventions

The following labels are used throughout:

| Label | Meaning |
|---|---|
| **DOCUMENTED** | Established syntax in the Surface Spec / historical grammar; actual compiler support must still be verified. |
| **APPROVED SEMANTICS** | Behavior established by Memory & Ownership v0.4, possibly without final source spelling. |
| **SURFACE OPEN** | Grammar/spelling not approved; **not** an accepted production. |
| **IMPLEMENTATION TO VERIFY** | Validate against lexer/parser/type checker and tests on the current branch. |
| **HISTORICAL** | Earlier document or prototype behavior, not an authority for new designs. |

Notation:

```text
A       ::= B C              sequence
A       ::= B | C            alternative
A       ::= [ B ]            optional
A       ::= { B }            zero or more repetitions
A       ::= B { "," B }      comma-separated sequence
TOKEN   ::= "literal"        exact source spelling
```

Terminals `Identifier`, `IntegerLiteral`, `FloatLiteral`, `StringLiteral`, `Newline`, and `Comment` are lexer categories. Productions describe source structure, not a prescribed AST shape, Pratt parser implementation, whitespace algorithm, or diagnostic wording.

**Semantic restrictions are not grammar productions.** A syntactically recognized expression can still be invalid due to type inference, effect contracts, borrow provenance, ownership, exhaustiveness, or unavailable runtime support.

## 2. Source files, declarations and script mode

```text
SourceFile       ::= { TopLevel }
TopLevel        ::= Declaration | TopLevelStatement

Declaration     ::= ImportDecl
                  | NamedFunctionDecl
                  | StructDecl
                  | EnumDecl
                  | InterfaceDecl
                  | ExtendDecl

TopLevelStatement ::= Statement
```

The original grammar used `File = Item*`, which omitted script-mode top-level statements. Tempra **documents** direct script execution without `main`; app-mode entry-point requirements are validated separately and do not require a second expression grammar.

`pub` is documented as a visibility modifier. Its precise permissible positions on non-function declarations and fields should be checked against the current parser and module-system implementation; the **canonical capability-attribute placement** on functions and `extend` methods is specified in §5.

A native/foreign `extern` declaration exists in the compiler and standard-library boundary, but its complete published grammar is not established by the two active source-language specifications. It is **not** silently reconstructed here from historical examples. See §37.

## 3. Lexical structure and delimiters

The documented source uses brace-delimited blocks and the following syntactic delimiters:

```text
( )    [ ]    { }    ,    :    .
->     =>     ..     =     ?    !
+      -      *      /      >>   |
==     !=     <      >      &&   ||
@
```

Keywords or context-sensitive terms appearing in documented productions include:

```text
let var fn return break continue
if else match for in is
struct enum interface extend import pub type
comptime ref inout consume self Self
true false
```

This list is a **documentation inventory**, not an exhaustive or authoritative lexer keyword table. In particular, `ref` and `consume` are documented as *capture modes*, not as generally accepted parameter modifiers. `self` in `fn[self]` is distinct from method receiver `self`.

- Braces `{ ... }` delimit executable blocks and type/interface bodies.
- `//` comments appear in established examples; exact comment and newline tokenization is implementation-owned.
- There is no indentation-sensitive grammar in the canonical language.
- Exact semicolon/newline separation rules, trailing-comma acceptance in every production, lexical `Int` aliases and numeric literal suffix rules require parser tests. This document does not fabricate them.
- A parser must tokenize multi-character operators such as `..`, `->`, `=>`, `>>`, `&&`, `||`, `==` and `!=` unambiguously.

## 4. Imports

```text
ImportDecl     ::= "import" ImportPath
ImportPath     ::= QualifiedName | "." QualifiedName
QualifiedName  ::= Identifier { "." Identifier }
```

```tempra
import std.fs
import std.process
import pkg.json
import .helper
```

The dotted paths above are the **active Surface v0.2 forms**. An older module-system document contains slash-separated imports (`std/fs`, `./utils`); those are historical conflicting examples, **not** an additional grammar alternative. The stdlib root is selected explicitly at the compiler/toolchain boundary, not silently searched from the process cwd.

## 5. Attributes and declarations of named functions

```text
NamedFunctionDecl ::= { CapabilityAttribute } [ "pub" ] FunctionDecl

CapabilityAttribute ::= PureAttribute | DenyAttribute
PureAttribute       ::= "@" "pure"
DenyAttribute       ::= "@" "deny" "(" DenyTarget
                        { "," DenyTarget } [ "," ] ")"

FunctionDecl        ::= "fn" Identifier "(" [ FunctionParameters ] ")"
                        [ ReturnType ] Block
ReturnType          ::= "->" Type
```

`@deny` requires **at least one** valid target; `@deny()` is invalid. Attributes can precede optional `pub` on named top-level functions and methods within `extend`. `@pure` has no arguments, and a second `@pure` marker is an error. A repeated `@deny` merges its restrictions under the effect specification.

**Do not** place current capability attributes on closures, interface requirements, fields or foreign declarations. Source forms `@unsafe` and `@verified` are not currently accepted.

### 5.1 Broad and specialized `@deny` targets

The canonical target families are defined by **Specialized Effects v0.2**:

```text
DenyTarget       ::= CapabilityName | FamilyName | CategoryName
                   | OperationName [ "(" [ ResourceArguments ] ")" ]

CapabilityName   ::= "alloc" | "panic" | "io" | "process"
                   | "spawn" | "nondeterminism"
FamilyName       ::= "fs" | "net"
CategoryName     ::= "io.read" | "io.write"

OperationName    ::= "fs.read" | "fs.write" | "fs.remove"
                   | "net.connect" | "net.listen"
                   | "net.send" | "net.receive"
                   | "io.read" | "io.write"
                   | "process.exec" | "process.exit"
                   | "spawn.task" | "spawn.actor"

ResourceArguments ::= ResourceArgument { "," ResourceArgument }
ResourceArgument  ::= Identifier ":" ResourceValue
```

`ResourceValue` is constrained by the specialized-effect **resource-value grammar and target schema**. The examples below show only documented forms; this file does not broaden accepted values or resources:

```tempra
@deny(alloc, panic)
fn checked_work() -> Int { 1 }

@deny(fs, net)
fn offline() -> Int { 1 }

@deny(fs.write(destination: "private.txt"))
fn report() -> Int { 1 }

@deny(fs.write(destination: *))
fn no_writes() -> Int { 1 }

@deny(io.read, io.write())
fn constrained_io() -> Int { 1 }
```

**Mandatory name resolution:** bare `io.read` / `io.write` are *cross-family categories*; `io.read()` / `io.write()` are the corresponding concrete generic operations. `fs` / `net` are families, not capabilities. Other operation names may be bare or have an empty parameter list. Unknown names, invalid resource names/values, parenthesized families and `unsafe` as a public denial are rejected. Families/categories do not create new capability bits.

`@pure` means denial of `io`, `process`, `spawn`, `nondeterminism` and **does not** imply `@deny(alloc, panic)`.

### 5.2 Parameter lists and receiver parameters

```text
FunctionParameters ::= FunctionParameter { "," FunctionParameter }

FunctionParameter  ::= GenericTypeParameter
                     | ComptimeParameter
                     | ValueParameter
                     | InoutParameter
                     | ReceiverParameter

GenericTypeParameter ::= "type" Identifier [ ":" TypeConstraint ]
TypeConstraint       ::= Identifier
ComptimeParameter    ::= "comptime" Identifier ":" Type
ValueParameter       ::= Identifier ":" Type
InoutParameter       ::= "inout" Identifier ":" Type
ReceiverParameter    ::= "self" | "inout" "self"
```

- `type T` and `type T: Interface` are documented for specialization-oriented generics.
- `comptime name: Type` is documented for **named functions**, not for closures.
- `inout value: T` is an explicit non-owning exclusive parameter.
- `self` and `inout self` are the **documented** receiver spellings in methods and interface requirements. They are not free-standing ordinary parameters of top-level functions.
- The previous `ParamList` rule constrained generic parameters to appear first. This revised production does not assert that arbitrary mixed order is already supported: accepted ordering and lowering must be verified against parser/type checker. Prefer the established form with generic parameters first in examples.
- `ref value: T`, `consume value: T`, `ref self` and `consume self` have **APPROVED SEMANTICS** but their public parameter/receiver grammar is **SURFACE OPEN**. They are *deliberately absent* from this production.
- Nonduplicable resource transfer must therefore not be inferred automatically from an ordinary by-value parameter merely because it is at its last use.

Examples:

```tempra
fn add(a: UInt64, b: UInt64) -> UInt64 {
    a + b
}

fn identity(type T, value: T) -> T {
    value
}

fn show(type T: Display, value: T) -> String {
    value.display()
}

fn increment(inout value: UInt64) {
    value = value + 1
}
```

## 6. Function items, closure values and function types

A **named function item** has no lexical captures. A **closure value** may have an explicit environment.

```text
ClosureExpr      ::= "fn" [ "[" CaptureList "]" ]
                     "(" [ ClosureParameters ] ")"
                     [ ReturnType ] Block

ClosureParameters ::= ClosureParameter { "," ClosureParameter }
ClosureParameter  ::= ValueParameter | InoutParameter

CaptureList      ::= RecursiveSelfCapture [ "," Capture { "," Capture } ]
                   | Capture { "," Capture }
RecursiveSelfCapture ::= "self"
Capture          ::= CaptureMode Identifier
CaptureMode      ::= "ref" | "inout" | "consume"

FunctionType     ::= "fn" "(" [ TypeList ] ")" [ ReturnType ]
```

The legacy grammar permitted `self` among capture items but the Surface v0.1 gave it a special first-position grammar. This revision keeps **`self` first**, and never treats it as a fourth capture mode. The precise parser acceptance of mixed `self` and other captures needs verification. Empty capture brackets `fn[]` are **forbidden**; use `fn(...)` for an empty environment.

```tempra
let add_one = fn(x: Int) -> Int {
    x + 1
}

let base = 10
let with_base = fn[ref base](x: Int) -> Int {
    base + x
}

var total = 0
let accumulator = fn[inout total](x: Int) {
    total = total + x
}

let data = load_data()
let own_data = fn[consume data]() {
    process(data)
}

let factorial = fn[self](n: Int) -> Int {
    if n < 2 { 1 } else { n * self(n - 1) }
}
```

Every referenced outer binding must have an explicit capture. There is **no default capture** and no `captures` keyword. `ref` and `inout` are borrowed captures checked under the Memory v0.4 non-escape and exclusivity rules; `consume` is a **semantic** transfer invalidating the source after closure creation. `fn[self]` is explicit recursion that cannot form a strong self-owned cycle. Nested closures do not inherit recursive `self` implicitly. Captured names cannot be shadowed within the closure body; closures are not comparable.

`fn(Int) -> Int` is a callable type; `fn(x: Int) -> Int { ... }` is a closure expression. Generic/comptime parameters in closure declarations are not established and must not be inferred from named function syntax. Callback escape summaries and lifetime checks are semantic analysis, not syntactic capture defaults.

## 7. Struct declarations and methods inside structs

```text
StructDecl      ::= [ "pub" ] "struct" Identifier [ TypeConstructorParameters ]
                    "{" { StructMember } "}"
TypeConstructorParameters ::= "(" [ GenericTypeParameters ] ")"
GenericTypeParameters     ::= GenericTypeParameter { "," GenericTypeParameter }
StructMember    ::= StructField | StructMethod
StructField     ::= [ "var" ] Identifier ":" Type [ "=" Expression ]
StructMethod    ::= FunctionDecl
```

The historical grammar declared `StructField*` and accidentally omitted methods documented inside a `struct`. This production restores their source-language form. Capability attributes on *in-type* methods, and `pub` on fields/methods, need individual parser/feature verification; capability contracts are **currently** documented for named top-level functions and `extend` methods, not automatically every `StructMethod`.

```tempra
struct User {
    name: String
    var bank_value: UInt64 = 0

    fn display(self) -> String {
        self.name
    }
}

struct Box(type T) {
    value: T
}

let b: Box(Int) = Box(Int) { value: 1 }
```

`var` marks a mutable field; ordinary fields are immutable. `let` is not a field modifier. Struct type constructors use `Box(Int)`, **not** `Box[Int]`. The `StructField` syntax does not imply that a dynamic `Slice(T)` can be persistently stored: the v0.4 owner/escape rules apply. There is no public nonduplicable-type declaration keyword yet.

## 8. Enum declarations

```text
EnumDecl       ::= [ "pub" ] "enum" Identifier [ TypeConstructorParameters ]
                   "{" { EnumVariant } "}"
EnumVariant    ::= Identifier [ "(" TypeList ")" ]
TypeList       ::= Type { "," Type }
```

```tempra
enum State {
    Idle
    Ready(Int)
    Failed(String)
}

enum Maybe(type T) {
    None
    Some(T)
}

let found: Maybe(Int) = Some(42)
```

The documented v1 enum constructor arity is **zero or one positional payload component** even though the legacy generic `TypeList` production might appear to accept more. Do not infer unrestricted multi-payload variants without new approval and tests. A generic enum constructor requires a sufficiently informative expected type context; `let found = Some(42)` is **not** automatically sufficient in the documented v1 rules.

## 9. Static structural interfaces

```text
InterfaceDecl      ::= "interface" Identifier "{" { InterfaceMember } "}"
InterfaceMember    ::= InterfaceMethod | InterfaceAssociatedType
InterfaceMethod    ::= "fn" Identifier "(" [ InterfaceParameters ] ")"
                       [ ReturnType ]
InterfaceParameters ::= FunctionParameter { "," FunctionParameter }
InterfaceAssociatedType ::= "type" Identifier
```

```tempra
interface Display {
    fn display(self) -> String
}

interface Iterator {
    type Item
    fn next(inout self) -> Item?
}
```

Interfaces are **static-only structural constraints** for `type T: Interface`. No dynamic vtables or runtime existential interface types follow from this syntax. Associated types are declared by `type Item` and projected in **type position**, e.g. `T.Item`. Interface satisfaction must match parameter access, return provenance, copyability, callback escape and effects according to Memory v0.4. Constraint syntax on generic `struct`/`enum` type parameters is not supported in documented v1; the generic grammar shape is not permission to add it.

## 10. `extend` blocks and method hooks

```text
ExtendDecl     ::= "extend" Type "{" { ExtendMember } "}"
ExtendMember   ::= NamedFunctionDecl
```

```tempra
extend User {
    fn is_rich(self) -> Bool {
        self.bank_value > 100000
    }

    fn deposit(inout self, amount: UInt64) {
        self.bank_value = self.bank_value + amount
    }
}
```

The receiver has the **same memory semantics** as an ordinary parameter: `self` is by-value; `inout self` is exclusive mutable access; conceptual `ref self` and `consume self` are **SURFACE OPEN**, not secretly inferred from read-only or resource-using method bodies.

The documented subscript hooks are specially named methods:

```tempra
extend MyType {
    fn get(self, index: IdxType) -> ValueType { /* ... */ }
    fn set(inout self, index: IdxType, value: ValueType) { /* ... */ }
    fn slice(self, start: IdxType, end: IdxType) -> ValueType { /* ... */ }
    fn set_slice(inout self, start: IdxType, end: IdxType, value: ValueType) { /* ... */ }
}
```

`set`/`set_slice` return `Unit` and must use `inout self` by documented contract. Hook resolution and type checking are semantic, not a new bracket syntax. Nested subscript mutation `x[i][j] = v` is not documented as supported in v1.

## 11. Blocks and statements

```text
Block           ::= "{" { Statement } [ TailExpression ] "}"
Statement       ::= BindingStatement
                  | ReturnStatement
                  | BreakStatement
                  | ContinueStatement
                  | ForStatement
                  | ExpressionStatement

BindingStatement ::= ( "let" | "var" ) BindingPattern [ ":" Type ]
                     "=" Expression
BindingPattern  ::= Identifier | TupleBinding
TupleBinding    ::= "(" Identifier "," Identifier { "," Identifier } ")"

ReturnStatement ::= "return" [ Expression ]
BreakStatement  ::= "break"
ContinueStatement ::= "continue"
ExpressionStatement ::= Expression
TailExpression  ::= Expression
```

The `Statement`/`TailExpression` separation is conceptual: actual separators, lookahead and block-tail recognition belong to the parser. **Blocks are expressions**, and a final expression provides the result when no explicit `return` is used. No `let mut`; use `var`.

```tempra
let name = "Lorenzo"
var count = 0
let result = {
    let a = 1
    let b = 2
    a + b
}
let (first, second) = get_pair()
```

The v0.1 grammar accepted only `let Identifier = ...`; this revision includes the documented tuple-destructuring binding. Other binding patterns (for example a struct pattern in `let`) are **not** automatically accepted.

## 12. `for` statements and expressions

```text
ForStatement    ::= ForCore
ForExpression   ::= ForCore
ForCore         ::= "for" Identifier "in" Expression [ ForFilter ] Block
ForFilter       ::= "if" Expression
```

```tempra
for value in numbers {
    io.println(value)
}

let positive = for value in numbers if value > 0 {
    value
}
```

The same source syntax forms a statement or an expression depending on context. A `for` expression collects values into a dynamic array as documented; this is not an implicit lazy iterator. Only one loop family (`for`) is part of the documented source language; no `while` production is added. `for-in` may create an internal iterator borrow, subject to NLL-like checks.

## 13. Expressions: overview and precedence

The grammar below is an implementation-oriented **precedence model**, not a claim that all listed operators have identical support on all types or all current backend paths.

```text
Expression       ::= AssignmentExpression | RangeExpression
RangeExpression  ::= RangeEndpoint ".." RangeEndpoint
RangeEndpoint    ::= LogicalOr
AssignmentExpression ::= LogicalOr [ "=" AssignmentExpression ]
LogicalOr       ::= LogicalAnd { "||" LogicalAnd }
LogicalAnd      ::= Equality { "&&" Equality }
Equality        ::= PatternTest { ( "==" | "!=" ) PatternTest }
PatternTest     ::= Comparison [ "is" IsPattern ]
Comparison      ::= Pipeline { ( "<" | ">" ) Pipeline }
Pipeline        ::= Shift { "|" Shift }
Shift           ::= Additive { ">>" Additive }
Additive        ::= Multiplicative { ( "+" | "-" ) Multiplicative }
Multiplicative  ::= Unary { ( "*" | "/" ) Unary }
Unary           ::= ( "!" | "-" ) Unary | Postfix
```

**Important corrections to the old snapshot:**

1. `is` takes a **pattern** on its right, not a general comparison expression. `if value is Some(v)` must parse and bind `v` locally.
2. `|` is present in command pipelines and OR-patterns; a distinct parser context separates them. A general public bitwise-OR operator is not inferred from the pipeline token alone.
3. `>>` was documented in the grammar as a shift operator; its actual supported types/semantics require implementation verification.
4. The old grammar mentioned `as?` in comparison syntax, while the active Surface v0.2 documents explicit conversions through `Type(value)`, `Type.parse(...)`, `Type.try(...)`. Because the precise `as?` grammar is not independently specified there, it remains a **legacy audit item**, not a newly certified cast production.
5. Other comparisons (`<=`, `>=`) appear in historical examples; their exact supported token set/precedence must be verified from current lexer/parser tests before treating them as canonical additions to this precedence grammar.
6. `..` forms a standalone `RangeExpression` (see also §16). The alternatives above recognize its shape without choosing a final universal precedence relative to assignment/comparison: parsers must disambiguate at `..` and verify the endpoint grammar against the current implementation.

Logical `&&` and `||` may operate on `Bool` or documented `Command` compositions, with type-specific semantics; they are not generic permission to introduce more overloads.

## 14. Primary expressions and postfix chains

```text
Postfix         ::= Primary { PostfixSuffix }
PostfixSuffix   ::= "(" [ CallArguments ] ")"
                  | "[" TypeList "]" "(" [ CallArguments ] ")"
                  | "." Identifier
                  | "[" IndexOrRange "]"
                  | "?" | "!"

Primary         ::= Literal
                  | Identifier
                  | GroupedOrTuple
                  | ArrayLiteral
                  | Block
                  | IfExpression
                  | MatchExpression
                  | ForExpression
                  | StructLiteral
                  | ClosureExpr
                  | ComptimeExpression

CallArguments   ::= CallArgument { "," CallArgument }
CallArgument    ::= [ "inout" ] Expression
```

`inout` in a call is a documented explicit exclusive-access argument, for example `normalize(inout data[0..4])`. `ref` and `consume` **call-argument** forms are not introduced because their final general-purpose source spellings are **SURFACE OPEN**. The `inout` token is not a global unary operator for arbitrary expression contexts.

Postfix `?` means `Result` propagation in expression position; postfix `!` means the documented unwrap/crash behavior. They are **not** the same as type-position `T?` or `T!E`.

The explicit function specialization suffix `f[T](args...)` must be disambiguated from indexing `a[i]`. The presence of `[...]` alone never forces generic specialization; call context, type grammar and symbol resolution determine the intended production.

## 15. Literals, tuples and arrays

```text
Literal         ::= IntegerLiteral | FloatLiteral | StringLiteral
                  | "true" | "false"
GroupedOrTuple  ::= "(" Expression ")" | TupleLiteral
TupleLiteral    ::= "(" Expression "," Expression
                     { "," Expression } [ "," ] ")"
ArrayLiteral    ::= "[" [ Expression { "," Expression } [ "," ] ] "]"
```

```tempra
let pair = ("Ada", 42)
let (name, age) = pair
let items: Array(Int) = [1, 2, 3]
let empty: Array(Int) = []
```

Tuples have arity at least two and are positional. `[]` needs an expected type. An array or tuple containing a nonduplicable resource inherits nonduplicability. Numeric suffixes and exact trailing-comma support are **IMPLEMENTATION TO VERIFY** beyond the explicitly documented examples.

## 16. Range, indexing, slicing and assignment

```text
IndexOrRange    ::= Expression
                  | Expression ".." Expression
```

`Range` is a value-based half-open interval **`[start, end)`**. The `RangeExpression` production in §13 explicitly permits constructing a Range value, not merely slicing inside brackets. The Surface v0.2 documents both `array[a..b]` and applying a stored range with `array[range]`:

```tempra
let section = 1..3
let view: Slice(UInt8) = bytes[section]
let direct = bytes[0..2]
```

The old grammar only showed range syntax in `a[a..b]` and did not make general value-form Range construction explicit. The final lexer/parser precedence and concrete numeric/index type are **IMPLEMENTATION TO VERIFY**. `Range` construction never extends the lifetime of the owner to which it might later be applied.

`Slice(T)` is **read-only and non-owning**. `var bytes: Array(T)` may be mutated via owned indexing `bytes[i] = value`, subject to borrow/exclusivity rules. A borrowed `Slice(T)` does **not** permit `view[i] = value` unless a valid `inout` exclusive access contract applies. `inout Slice(T)` is not a separate `MutableSlice(T)` type. Array slice-range assignment must satisfy the documented length check. Bounds checks and `Panic` contracts remain semantic obligations.

The lexer/parser must distinguish type-level `[T]`, array literal `[a, b]`, index access `a[i]`, range access `a[a..b]`, and explicit generic call `f[T](...)` according to context.

## 17. Struct literals and update syntax

```text
StructLiteral      ::= TypeName "{" [ StructInitializerList ] "}"
TypeName           ::= QualifiedTypeName [ "(" TypeList ")" ]
StructInitializerList ::= StructInitializer
                          { "," StructInitializer } [ "," ]
StructInitializer  ::= Identifier ":" Expression
                     | ".." Expression
```

```tempra
let user = User {
    name: "Ada",
    bank_value: 100,
}

let updated = User {
    bank_value: 200,
    ..user
}
```

`TypeName` includes a nominal type constructor such as `Box(Int)` in documented examples. The parser must disambiguate `Type { ... }` from a brace-delimited block after a condition or another construct. `..source` is a **struct update**, not an explicit `consume`; it cannot silently duplicate or transfer a nonduplicable resource.

## 18. Conditional `if` expressions

```text
IfExpression   ::= "if" [ "comptime" ] Expression Block
                   [ "else" ( Block | IfExpression ) ]
```

```tempra
let label = if active {
    "active"
} else {
    "inactive"
}

if comptime T == Int {
    0
} else {
    compile_error("unsupported type")
}
```

The optional `comptime` after `if` selects a compile-time branch, not a new effect or lifetime mode. Only the selected branch is semantically checked after specialization; unselected branches must still parse. Ordinary branches receive path-sensitive borrow analysis, with conservative provenance joins.

## 19. `match` expressions, guards and pattern grammar

```text
MatchExpression  ::= [ "comptime" ] "match" Expression
                     "{" { MatchArm } "}"
MatchArm         ::= Pattern [ "if" Expression ] "=>" Expression
Pattern          ::= OrPattern
OrPattern        ::= AtomicPattern { "|" AtomicPattern }

AtomicPattern    ::= "_"
                   | Literal
                   | Identifier
                   | ConstructorPattern
                   | StructPattern
                   | TuplePattern

ConstructorPattern ::= Identifier "(" [ PatternArguments ] ")"
PatternArguments   ::= Pattern { "," Pattern }
StructPattern      ::= Identifier "{" [ StructPatternFields ] "}"
StructPatternFields ::= StructPatternField
                        { "," StructPatternField } [ "," ]
StructPatternField ::= Identifier [ ":" Pattern ]
TuplePattern       ::= "(" Pattern "," Pattern
                       { "," Pattern } [ "," ] ")"

IsPattern          ::= AtomicPattern
```

```tempra
enum State {
    Idle
    Ready(Int)
    Failed(String)
}

let text = match state {
    Idle => "idle"
    Ready(n) if n > 0 => "positive"
    Ready(_) => "ready"
    Failed(message) => message
}

if state is Ready(n) {
    use(n)
}
```

`Pattern` construction is recursive; OR-patterns are supported in `match` only. All alternatives must bind the same identifiers with the same types; catch-all binding/wildcard alternatives in OR-patterns are rejected. Guards do not contribute to exhaustiveness coverage. `Bool` and enums require exhaustive matching; tuple/struct pattern exhaustiveness remains conservatively limited in documented v1. A `match` on `Never` has no arms.

**Pattern name resolution matters:** lowercase identifiers in pattern position may bind, whereas enum constructors are resolved using the expected scrutinee type. `is` does not support OR-patterns, wildcard catch-all or naked binding patterns. The simplified `IsPattern` production above must be refined by semantic pattern restrictions. `comptime match` uses the same surface introducer, but supports only the deterministic subset documented in `COMPTIME_SPEC.md`.

The historical grammar omitted `if` guards in `MatchArm`; this revision includes them. A `match` arm body is an expression and can itself be a block.

## 20. `comptime` expressions, type values and specialization

```text
ComptimeExpression ::= "comptime" ( Block | Expression )
ComptimeMatch      ::= "comptime" "match" Expression
                       "{" { MatchArm } "}"
```

A parser should recognize the more specific `comptime match` form **before** a generic `comptime Expression` prefix to avoid an ambiguity in naive recursive descent. Similarly, `if comptime` has the form specified in §18.

```tempra
let answer = comptime fib(10)
let table = comptime {
    let a = 1
    let b = 2
    a + b
}

fn default_value(type T) -> T {
    comptime match T {
        Int => 0
        Bool => false
        else => compile_error("unsupported type")
    }
}
```

Generic **type values** may be used at compile time; type equality uses `==` / `!=`. `comptime` parameters belong to named functions and participate in specialization identity. This phase does not grant IO, unsafe authority or extended lifetime to values. Current HIR execution and planned VM execution must share the **same semantics**, bounded by the separate comptime and Memory v0.4 specifications.

## 21. Types, type constructors and sugar

```text
Type             ::= BaseType [ TypeSuffix ]
TypeSuffix       ::= "?" | "!" [ BaseType ]

BaseType         ::= QualifiedTypeName [ "(" [ TypeList ] ")" ]
                   | ArrayType
                   | TupleType
                   | FunctionType
                   | "Never"
                   | "Self"

QualifiedTypeName ::= Identifier { "." Identifier }
ArrayType        ::= "[" Type "]"
TupleType        ::= "(" Type "," Type { "," Type } ")"
FunctionType     ::= "fn" "(" [ TypeList ] ")" [ ReturnType ]
```

Documented constructions:

```tempra
let values: Array(Int) = [1, 2, 3]
let values2: [Int] = [1, 2, 3]
let part: Slice(Int) = values[0..2]
let maybe: Int? = Some(42)
let answer: String!IoError = load_name()
let callback: fn(Int) -> Int = fn(x: Int) -> Int { x + 1 }
```

- Type constructors use `Array(T)`, `Slice(T)`, `Box(T)`, `Option(T)`, `Result(T, E)`.
- `T?` is **type-position** sugar for `Option(T)`.
- `T!E` is **type-position** sugar for `Result(T, E)`; `T!` uses default `Error`.
- `fn(T) -> U` denotes a callable type.
- `T.Associated` is permitted in **type position** as projection of an associated type; it is not an expression-level member lookup.
- The older grammar included `[T; Integer]` as a static array type; the active Surface v0.2 does not certify its implementation or final semantics. It remains a **legacy audit item**, not an automatically approved type form.
- The grammar does not create `&T`, `&mut T`, `MutableSlice(T)`, `unique T`, `resource T` or lifetime type parameters.
- Chaining/nesting of `?` / `!` suffixes, unqualified `Int` width and `Unit` spelling require the normal typechecker/parser verification; the simplified production does not invent additional transformations.

## 22. Type argument calls versus generic constructors

The same tokens `(` and `[` occur in several roles with different meanings:

```tempra
let boxed = Box(Int) { value: 42 }   // Generic type constructor
let result = identity[Int](42)      // Explicit function specialization
let part = values[0..2]            // Slice expression
let x: Int? = Some(42)              // Expected-type enum construction
```

A generic type constructor uses parentheses in **type position**. An explicit generic **function specialization** uses square brackets before the argument call. Neither a constructor nor an ordinary call may silently infer semantic `consume` from its argument shape.

For generic enum construction, the expected type must resolve the enum type/variant. Do not rely on a payload alone to choose among alternative generic ADTs.

## 23. Strings and indexing restrictions

The public string type is a single UTF-8 `String`. `String.bytes()` yields a **read-only** `Slice(UInt8)`; it is not an independent owned array and does not make its backing memory `Static`.

```tempra
let text = "hello"
let n = text.len()        // Unicode scalar count
let byte_count = text.size()
for byte in text.bytes() {
    inspect_byte(byte)
}
```

Direct string indexing `text[i]` and slicing `text[a..b]` are intentionally **not supported** by the source-language contract. The fact that a general postfix index can be parsed does not imply every indexed type passes type checking.

## 24. Options, Results, propagation and `Never`

`Some`, `None`, `Ok` and `Err` are ordinary resolved enum constructors in expression/pattern position. An expected type is needed when the generic constructor is otherwise ambiguous.

```tempra
fn maybe_first(xs: Array(Int)) -> Int? {
    if xs.is_empty() { None } else { Some(xs[0]) }
}

fn load() -> String!IoError {
    read_name()
}

fn app() -> String!IoError {
    let name = load()?
    Ok(name)
}
```

`expr?` is Result propagation. `expr!` is unwrap/crash and can require `Panic`. They are distinct from `T?`/`T!E`. `Never` is the bottom type; explicit `exit(...)` and `crash(...)` yield `Never` in the documented language. Whether `?` is accepted at a given return type is a type/effect check, not a parsing decision.

## 25. Conversion and casting

The active Surface v0.2 documents explicit conversion through ordinary type-like call syntax:

```tempra
let number = UInt64(value)
let float = Float64(count)
let parsed = UInt64.parse(text)
let narrowed = UInt8.try(value)
```

This needs no new general-purpose cast operator in the grammar. `as?` was present in the prior parser-oriented grammar but is **IMPLEMENTATION TO VERIFY / LEGACY AUDIT** until it has a reconciled source-language contract. No implicit coercion is introduced.

## 26. Operators, commands and overload boundaries

The active Surface Spec documents `+`, `-`, `*`, `/` as the limited user-overloadable operators. This grammar does not authorize arbitrary user-defined operators or precedence changes. The boolean spellings are `&&`, `||`, `!`, not `and`, `or`, `not`.

```tempra
let command = cmd("git", ["status"]) | cmd("grep", ["modified"])
```

`cmd(...)` parses through normal identifier/function-call syntax. `|` is the documented pipeline operator for command values. `&&` / `||` also compose commands under their respective type-specific semantics. Construction of a command value does not necessarily launch a process; actual operation effects must follow the capability contracts.

## 27. Numeric types and checked arithmetic

The Surface Spec documents:

```text
Int8 Int16 Int32 Int64
UInt8 UInt16 UInt32 UInt64
Float32 Float64
```

It intentionally does not introduce public aliases `Float`, `Double` or `UInt`. Historical examples use `Int`; exact interpretation/width is **IMPLEMENTATION TO VERIFY**. Checked `Int64` addition, subtraction, multiplication and negation, plus division-by-zero/minimum-overflow checks, are semantic runtime contracts with conservative `Panic` requirements. They are not parsing features and cannot be optimized away merely to bypass `@deny(panic)`.

## 28. Ownership constraints on otherwise valid syntax

Grammar recognition is insufficient to establish memory validity. The following are **APPROVED SEMANTICS** from Memory & Ownership v0.4:

| Surface form | Required semantic check |
|---|---|
| `f(value)` with duplicable `T` | Caller retains a valid value; a physical last-use move is optional. |
| `f(inout value)` | Borrowed exclusive access; validity and aliasing checked. |
| `fn[ref x]` | Read-only borrowed capture; dynamic escape limited. |
| `fn[inout x]` | Exclusive mutable borrowed capture; source must permit mutation. |
| `fn[consume x]` | Semantic owner transfer; source invalidated. |
| `fn[self]` | Stable recursion with no strong ownership self-cycle. |
| Local struct containing dynamic `Slice(T)` | Temporarily allowed only if non-escaping; owner tracked. |
| `array[range]` | Bounds checked on application; Range does not own memory. |
| `inout Slice(T)` | Exclusive mutation of underlying elements, not merely descriptor. |
| Ordinary by-value use of nonduplicable resource | Rejected unless an explicitly specified transfer contract exists. |
| Closure escaping with dynamic borrowed capture | Rejected. |
| Owned value across `.await()` | Allowed when continuation owns it. |
| Dynamic loan live across `.await()` | Rejected. |
| `@deny(alloc)` and potentially allocating value copy | Conservatively rejected; physical elision alone is not proof. |

The grammar does not include NLL, provenance parameters or lifetime annotations. The type checker and verified TIR must enforce these constraints uniformly on Tempra Script VM and future Tempra Native LLVM AOT. ARC NO-CYCLE is not a runtime fallback for invalid source borrowing.

## 29. Async/task/actor syntax: explicit boundary of this revision

**Approved:** ownership/isolation semantics for suspension, spawned tasks, actor state, messages and cancellation under Memory v0.4.

**Not approved here:** the full declaration/expression grammar for actor types, `spawn`, task groups, cancellation APIs, supervision or scheduling. Actor handle identity/lifetime is **DEFERRED POST NO-CYCLE**.

The documented `.await()` style can be parsed as an ordinary member access and call suffix, but this fact **does not certify** an async runtime or define a new `await` keyword production. The source-level concurrency grammar must be approved in a dedicated revision.

Therefore the following is **illustrative pseudocode only**, not a normative example for the current parser:

```text
actor Counter {
    var value: Int = 0
    // Actor messages/method execution contract: to be specified.
}
```

A Reader/Writer task that carries a nonduplicable handle across suspension must own that handle. `consume self` is a semantic design requirement, **not** a parser-accepted spelling until separately approved. Do not add a hidden borrowed receiver default.

## 30. FFI, unique resources and unsafe declarations

The accepted grammar has no new `resource`, `unique`, `move`, `shared`, `weak` or `lifetime` keyword. A compiler/stdlib wrapper may own a nonduplicable resource, but the **public declaration mechanism** is **SURFACE OPEN**. `@unsafe` and raw-pointer authorization remain deferred.

`extern` and native bridge constructs present in current tooling must be reconciled with real source examples and parser tests before this grammar claims a complete foreign-function declaration production.

Semantic requirements are nevertheless fixed: no implicit duplication of resource-owning handles, no raw-pointer-to-safe-slice conversion without provenance/length/exclusivity/lifetime proof, deterministic cleanup for completed paths, and no leaked capability effects from cleanup or runtime helpers.

## 31. Operator precedence summary

| Higher → lower | Family | Tokens / notes |
|---|---|---|
| 1 | Postfix | call, specialization-call, member, index, propagation `?`, unwrap `!` |
| 2 | Prefix | `!`, `-`; `comptime` uses contextual parsing |
| 3 | Multiplicative | `*`, `/` |
| 4 | Additive | `+`, `-` |
| 5 | Shift | `>>` (support to verify) |
| 6 | Command pipeline | `|` (type/context specific) |
| 7 | Comparison | `<`, `>`; additional comparisons to verify |
| 8 | Pattern test | `is` with restricted pattern RHS |
| 9 | Equality | `==`, `!=` |
| 10 | Logical AND | `&&` |
| 11 | Logical OR | `||` |
| 12 | Assignment | `=` (right associative; valid lvalue required) |

**Range `..`:** intentionally left as a contextual range/index production pending lexer/parser reconciliation rather than given a speculative global precedence. **`as?`:** retained only as a legacy audit item. Match arm `|` is an OR-pattern, not the command pipeline operator.

## 32. Parser disambiguation requirements

A recursive-descent/Pratt parser should explicitly test:

1. `fn(...)` closure vs `fn name(...)` named function vs `fn(Type...) -> Type` function type.
2. `fn[self]` recursive marker vs `fn[ref x]`, `fn[inout x]`, `fn[consume x]`; reject `fn[]` and unqualified capture identifiers.
3. `Box(Int)` as type constructor vs `identity[Int](...)` specialization vs `a[index]` subscript.
4. `Type { field: expr }` struct literal vs `{ statements; expr }` block after `if`, `match` or `for`.
5. `(expr)` grouping vs `(expr, expr)` tuple and corresponding type/pattern forms.
6. `[Type]` array type vs `[expr]` literal vs `a[index]` postfix vs `a[start..end]` slice.
7. `T?`, `T!E` in type position vs `expr?`, `expr!` in expression position.
8. `Some(x)` / `Ok(x)` expected-type constructor resolution vs local symbol shadowing.
9. `match` pattern OR `|` vs `cmd(...) | cmd(...)` expression pipeline.
10. `if x is Some(v)` and pattern bindings vs ordinary binary comparisons.
11. `if comptime condition` vs `comptime match value` vs `comptime { ... }` vs `comptime expr`.
12. `inout value` at a call site vs `inout value: T` in a signature vs `inout self` receiver.
13. Attribute target `io.read` category vs `io.read()` operation (syntactically similar, semantically distinct).
14. Top-level script statements vs named declarations; app-entry validation after parsing.
15. Return, match guards and implicit block-tail values without inventing a second expression grammar.

## 33. Minimal AST/HIR inventory (non-normative)

A practical frontend should represent enough syntax to preserve distinctions through type checking and TIR lowering:

```text
Module, Import, Visibility
NamedFunction, Parameter, Receiver, CapabilityAttribute
Struct, StructField, StructMethod, Enum, EnumVariant
Interface, AssociatedType, Extend
Block, Binding, Return, Break, Continue, For
If, Match, MatchArm, Guard, Pattern, PatternOr
Unary, Binary, PatternTest, Assignment
Call, CallArgument, FunctionSpecialization, MemberAccess
Index, Slice, Range, StructLiteral, StructUpdate
ArrayLiteral, TupleLiteral, TupleBinding
Closure, CaptureMode, RecursiveSelfCapture
ComptimeExpr, ComptimeBlock, ComptimeMatch, TypeValue
FunctionType, TypeConstructor, TypeProjection, TypeSugar
ResultPropagation, ExplicitUnwrap, CommandPipeline
```

This is an **inventory**, not a mandated Rust enum layout. Ownership and capability analyses consume typed semantic data, not lexical guesses or runtime fallback checks.

## 34. Canonical examples for parser/typechecker fixtures

The following examples exercise distinct documented forms. Each is an isolated fixture, **not** a complete runnable program with imported names and support libraries.

### 34.1 Named functions and generic specialization

```tempra
fn identity(type T, value: T) -> T { value }
let result = identity[Int](42)
```

### 34.2 Struct, field default and receiver

```tempra
struct Counter {
    var value: Int = 0
    fn current(self) -> Int { self.value }
}
extend Counter {
    fn increment(inout self) { self.value = self.value + 1 }
}
```

### 34.3 Static interfaces and comptime

```tempra
interface Iterator {
    type Item
    fn next(inout self) -> Item?
}
fn constant(type T) -> T {
    comptime match T {
        Int => 0
        Bool => false
        else => compile_error("unsupported")
    }
}
```

### 34.4 Enum, guard and exhaustive match

```tempra
enum State { Idle Ready(Int) Failed(String) }
fn describe(s: State) -> String {
    match s {
        Idle => "idle"
        Ready(n) if n > 0 => "positive"
        Ready(_) => "ready"
        Failed(message) => message
    }
}
```

### 34.5 Closure captures and recursion

```tempra
let offset = 5
let add = fn[ref offset](n: Int) -> Int { offset + n }
let fact = fn[self](n: Int) -> Int {
    if n < 2 { 1 } else { n * self(n - 1) }
}
```

### 34.6 Borrow and slice access

```tempra
fn inspect(data: Slice(UInt8)) { /* read only */ }
fn normalize(inout data: Slice(UInt8)) { /* exclusive mutation */ }
var bytes: Array(UInt8) = [1, 2, 3]
let view = bytes[0..2]
inspect(view)
normalize(inout bytes[0..2]) // valid only after view's last use
```

### 34.7 Effect categories and resource selectors

```tempra
@pure
@deny(alloc)
fn no_io_or_alloc() -> Int { 5 }

@deny(fs.write(destination: "secret.txt"))
fn constrained() -> Int { 5 }
```

## 35. Conformance matrix: syntax versus semantic permission

| Case | Parse/grammar | Semantic/result status |
|---|---|---|
| `fn add(a: Int, b: Int) -> Int { a + b }` | Documented | Depends on type/effects. |
| Top-level `let value = 1` in script mode | Documented | Valid script context. |
| `struct User { fn display(self) -> String { ... } }` | Documented surface; old grammar omission corrected | Parser support to verify. |
| `fn next(inout self) -> Item?` | Documented receiver | Requires valid method/interface context. |
| `fn[ref x](...) { ... }` | Documented | Dynamic nonescaping capture required. |
| `fn[inout x](...) { ... }` | Documented | Exclusivity and mutable source required. |
| `fn[consume x](...) { ... }` | Documented | Source unusable after creation. |
| `fn[self](...) { ... }` | Documented | Recursive identity; no strong cycle. |
| `fn[](x: Int) { x }` | Rejected | Empty bracket capture list forbidden. |
| `fn[x](...) { ... }` | Rejected | No implicit/default capture mode. |
| `fn(ref x: T)` | **SURFACE OPEN** | Not asserted as accepted. |
| `fn(consume x: T)` | **SURFACE OPEN** | Not asserted as accepted. |
| `fn method(ref self)` | **SURFACE OPEN** | Read-borrow semantics approved. |
| `fn method(consume self)` | **SURFACE OPEN** | Owner-transfer semantics approved. |
| `f(inout value)` | Documented | Must satisfy exclusive access. |
| `f[T](...)` | Documented | Generic specialization only. |
| `Box(T)` in type position | Documented | Generic type constructor. |
| `let x: Int? = Some(1)` | Documented | Expected-type constructor. |
| `let x = Some(1)` without context | Parses | Type inference rejected in documented v1. |
| `T.Item` in type position | Documented | Requires associated-type resolution. |
| `T.Item` in value position | Parses as access | Rejected as type projection in expression position. |
| `if value is Some(v)` | Documented | `v` bound in true branch only. |
| `match` guard `if cond` | Documented | Guard does not prove exhaustiveness. |
| `x[a..b]` and `x[range]` | Documented/intended | Bounds and ownership checks apply. |
| `slice[i] = x` on read-only slice | General assignment syntax parses | Rejected without valid exclusive access. |
| Dynamic borrow across `.await()` | Member-call form can parse | Rejected by Memory v0.4. |
| `@deny(io.read)` | Documented | Category denial. |
| `@deny(io.read())` | Documented | Generic-operation denial. |
| `@deny()` | Rejected | No targets. |
| `@unsafe` | Not accepted | Deferred unsafe boundary. |
| `actor Counter { ... }` | **SURFACE OPEN** | Only actor ownership semantics approved. |
| `resource File { ... }` | **SURFACE OPEN** | No approved resource keyword. |

## 36. Differences from Grammar v0.1

This revision intentionally corrects or clarifies the following without adding a second language surface:

1. Adds script-mode top-level statements to the file model.
2. Recognizes documented methods inside `struct`, in addition to `extend` methods.
3. Recognizes constrained generic function parameters `type T: Interface` and documented `comptime` parameter/branch forms.
4. Preserves explicit closure captures and the special `fn[self]` rule, rejecting `fn[]` and default captures.
5. Models `self` / `inout self` as documented receiver forms without claiming `ref self` / `consume self` parser support.
6. Includes match guards, OR-pattern constraints, `is` RHS pattern interpretation, and documented tuple binding.
7. Distinguishes generic function specialization `f[T](...)` from type construction `Box(T)`.
8. Distinguishes `T?`/`T!E` type sugar from postfix `expr?`/`expr!`.
9. Includes `inout` call-site arguments and aligns Slice mutability with Memory v0.4.
10. Treats Range as an independent half-open value, not solely a slicing token.
11. Imports the **Specialized Effects v0.2** denial categories/families/operations rather than retaining only broad capability names.
12. Records async, actor, resources and FFI as semantic contracts with **open grammar** rather than inventing accepted tokens.
13. Marks `as?`, `[T; N]`, additional comparison tokens, extern declarations and exact lexical separators for dedicated compiler verification rather than certifying unsupported extensions.
14. Removes obsolete JIT/Cranelift-forward assumptions. Tempra Script VM and Tempra Native LLVM AOT are the designated targets; repository removal and compiler parity remain separate implementation tasks.

## 37. Open grammar decisions and implementation verification gates

**SURFACE OPEN — requires a separate language decision before implementation:**

- General parameter and receiver spelling for read-only `ref` and semantic-transfer `consume`.
- Call-site spelling for explicit transfer and any corresponding parameter matching rules.
- Public declaration/derivation of nonduplicability for resource-owning types.
- Full actor/task/spawn/async declaration and scheduling syntax; actor handles **DEFERRED POST NO-CYCLE**.
- Foreign function/raw pointer/unsafe-authorized syntax, only when a concrete need arises.

**IMPLEMENTATION TO VERIFY on the actual target branch:**

- Parse/typecheck acceptance of struct-local methods and visibility modifiers.
- Generic parameter constraints, parameter ordering, function types and type projections.
- Closure capture-list edge cases (`fn[self, ref x]`, nesting, shadowing and recursive identity).
- `inout` argument syntax, tuple binding and match guards.
- Range expression parsing/application, array static-type legacy notation and comparisons like `<=`/`>=`.
- `as?` historical cast spelling and exact token precedence.
- `@deny` resource-value grammar and specialized family/category/operator target resolution.
- Effect enforcement, borrow/NO-CYCLE checker, TIR verifier and new semantic cache revision.
- Current HIR vs VM execution path, compile-time parity, cancellation/resource cleanup and LLVM AOT readiness.

**Documentation rule:** keep experimental/prototype records in `docs/archive/` and indentation syntax in `docs/proposals/`. Neither archived JIT documents nor `INDENTATION_SYNTAX_PROPOSAL_v0.1.md` overrides this brace-based grammar or Memory & Ownership v0.4.

## 38. Historical bootstrap notes (non-normative)

The first parser bootstrap proceeded through lexer → parser/AST → minimal interpreter → incremental type/IR/compiler work. Historical notes saying that full matching, advanced generics or capability analysis were not yet implemented describe that earlier phase, not the current language contract. The E1–E6 and K4 specialized-effects work provides bounded implementation evidence, not proof of a complete ownership-aware VM or AOT backend.

There is no separate JIT grammar or runtime-dependent source semantics. Removing the historical Cranelift JIT and Cranelift backend code is a repository migration, not a syntax rule and not something this file claims has already happened.

---

**Final rule:** The grammar recognizes source shape; the type checker enforces the single Memory & Ownership v0.4 model; compiled execution must preserve it on Tempra Script and Tempra Native. No hidden default capture, borrowed-to-owned promotion, semantic consume inference, or actor-handle design is introduced here.
