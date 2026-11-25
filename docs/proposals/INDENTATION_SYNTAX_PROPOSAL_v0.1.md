# Tempra — Significant-Indentation Syntax Proposal v0.1

**Status:** PROPOSAL / FUTURE SPIKE — non-normative, not implemented, not approved as canonical syntax.
**Date:** 15 November 2026
**Repository destination:** `docs/proposals/INDENTATION_SYNTAX_PROPOSAL_v0.1.md`
**Authoritative references:** `docs/TEMPRA_SURFACE_SPEC_v0.2.md`, `docs/Tempra_Memory_Ownership_v0.4.md`, `docs/COMPTIME_SPEC.md`, `docs/specs/effects-v0.1.md`, `docs/specs/specialized-effects-v0.2.md`.

> This document defines a **candidate layout syntax for an isolated parser spike after the VM work**. It must **never** be interpreted as approval to switch the canonical grammar or change Tempra's semantics. The existing `{ ... }` block syntax remains authoritative unless an explicit later decision supersedes it.

## 1. Objective and limits

Investigate a Python/Roc/Mojo-like **significant-indentation syntax** while retaining Tempra's core language: `let`/`var`, `fn`, `type T`, `comptime`, `struct`, `enum`, `extend`, static interfaces, `if`, `match`, `for`, `is`, `ref`/`inout`/`consume`, explicit closure captures, effects and Result-based error handling.

The experiment changes **only the representation of suites/blocks** and related formatting. It does not adopt Python's dynamic typing, exception handling, implicit capture, reference model, implicit coercion, `def`, `class`, `case`, `async def`, decorators or generator machinery.

**Non-goals:** replacing the memory model, inventing actor-handle ownership, defining new resource qualifiers, removing generic specialization, changing type syntax or modifying runtime semantics.

## 2. Proposed layout rules (spike defaults, not canonical)

1. A compound header is followed by `:` and an indented suite. The first logical line of the suite must be indented more than the header; its indentation becomes the suite's baseline. A dedent closes the suite.
2. Indentation uses spaces. The spike initially uses **four spaces per level** in the formatter and rejects tabs in indentation, so behavior is deterministic. The final policy is subject to experiment.
3. Blank lines and comment-only lines do not cause `INDENT` or `DEDENT`.
4. Parentheses `(...)`, brackets `[...]`, and **expression-level braces** `{...}` suppress structural newline/indent tokens while they are open, allowing multiline expressions, type specializations, arrays and struct literals.
5. No semicolons or explicit block-ending keywords. Braces remain available for **struct-value literals** (`User { name: "Ada" }`) and are not used to delimit statement suites.
6. A last expression may remain the value of an expression block, just as in Tempra's baseline. An explicit `return` retains its current meaning.
7. `else` must align with its `if`; match arms must align with one another. A nested multiline arm body is indented beneath `=>`.
8. A multiline expression outside delimiters does not silently continue solely because a following line is more indented. Continuation must be explicit by being inside `()`, `[]`, `{}`; alternatives can be benchmarked later.
9. Lexical diagnostics report inconsistent dedents, missing expected suites, tabs and accidental indentation as source errors.
10. Do **not** infer any new ownership/borrowing behavior from indentation. All memory checks still follow Memory & Ownership v0.4.

## 3. Before/after example: functions, structs, methods

Baseline syntax:

```tempra
struct Counter {
    name: String
    var value: Int = 0

    fn label(self) -> String {
        self.name
    }
}

extend Counter {
    fn increment(inout self, amount: Int) {
        self.value = self.value + amount
    }
}
```

Proposed indentation syntax:

```tempra
struct Counter:
    name: String
    var value: Int = 0

    fn label(self) -> String:
        self.name

extend Counter:
    fn increment(inout self, amount: Int):
        self.value = self.value + amount
```

**Compatibility note:** in-type method syntax is documented in Surface v0.2 but not correctly represented in the older `GRAMMAR.md` struct production. Parser support must be verified separately. `ref self` and `consume self` are semantically defined by the memory model but remain **SURFACE OPEN**; this proposal does not silently approve them.

## 4. Generic types, static interfaces and comptime

```tempra
struct Box(type T):
    value: T

interface Display:
    fn display(self) -> String

fn show(type T: Display, value: T) -> String:
    value.display()

fn default_value(type T) -> T:
    comptime match T:
        Int => 0
        Bool => false
        String => ""
        else => compile_error("unsupported type")

fn add_n(comptime n: Int, x: Int) -> Int:
    x + n

let boxed = Box(Int) { value: 42 }
let answer = default_value[Int]()
let n = add_n(10, 5)
```

The candidate preserves type constructors `Box(Int)` and function specialization `default_value[Int]()`. Static interface constraints remain `type T: Interface`, including associated-type projections such as `T.Item` in type position only. Compile-time evaluation remains a phase subject to the same effect and ownership restrictions.

### Comptime block expression

Current form `comptime { ... }` becomes a block-introducing expression in the proposal:

```tempra
let total = comptime:
    let x = 10
    let y = 20
    x + y
```

This requires a parser test because `let x = comptime:` creates a nested expression suite after an assignment. `comptime fib(10)` remains a single-line expression; `if comptime ...:` and `comptime match ...:` are distinct existing constructions with layout bodies.

## 5. `if`, `match`, guards and `is`

```tempra
enum State:
    Idle
    Ready(Int)
    Failed(String)

fn describe(state: State) -> String:
    match state:
        Idle => "idle"
        Ready(n) if n > 0 => "positive"
        Ready(_) => "ready"
        Failed(message) => message

fn nonzero(state: State) -> Bool:
    if state is Ready(n):
        n > 0
    else:
        false
```

The existing pattern semantics, exhaustive checking, `is` narrowing and arm guards remain unchanged. **Open parser experiment:** choose whether a multi-statement `match` arm uses `=>` followed by `NEWLINE INDENT`, for example:

```tempra
match state:
    Ready(n) =>
        let doubled = n * 2
        doubled
    _ => 0
```

This is a *layout proposal*, not a change to enum or pattern semantics.

## 6. `for`, collection expressions and error propagation

```tempra
fn collect_positive(values: Array(Int)) -> Array(Int):
    for x in values if x > 0:
        x

fn first_line() -> String!IoError:
    let text = read_text()?
    text
```

`for` remains the only loop family and remains an expression where its body collects results. The suffix `?` remains Result propagation, not Python optional chaining. Type-position `T?`, `T!`, `T!E` keep their Tempra meanings.

## 7. Closure environments and `fn[self]`

```tempra
let offset = 10
let add_offset = fn[ref offset](x: Int) -> Int:
    offset + x

var total = 0
let accumulate = fn[inout total](x: Int):
    total = total + x

let bytes = load_data()
let process_bytes = fn[consume bytes]():
    process(bytes)

let factorial = fn[self](n: Int) -> Int:
    if n <= 1:
        1
    else:
        n * self(n - 1)
```

The existing capture grammar survives verbatim: `fn[ref name]`, `fn[inout name]`, `fn[consume name]`, and `fn[self]`. No capture default, no `captures` keyword, no invisible copying to repair escaping borrows. `consume` invalidates its source semantically. Recursive `self` must not introduce a strong ownership cycle.

## 8. Slices, effects and literals

```tempra
fn inspect(data: Slice(UInt8)):
    for item in data:
        io.println(item)

var bytes: Array(UInt8) = [10, 20, 30]
let region = 0..2
let view: Slice(UInt8) = bytes[region]
inspect(view)

@pure
@deny(io, process)
fn constant() -> Int:
    42
```

`Slice(T)` is a nonowning read-only view; `inout Slice(T)` permits checked exclusive mutation. `Range` is a half-open independent descriptor. Specialized capability targets such as `@deny(fs)` or `@deny(io.read)` retain their existing meaning. The proposal does not invent alternative effect syntax.

## 9. Actor and async: illustrative syntax ONLY

Memory & Ownership v0.4 specifies isolation, owned messages, suspended-borrow restrictions and cancellation invariants, **but does not approve actor, spawn, async declaration, mailbox or actor-handle grammar**.

The following is a **separate speculative syntax sample** for later comparison, not a parser production:

```text
actor Counter:
    var value: Int = 0

    fn increment(inout self, by: Int):
        self.value = self.value + by

# Calling convention, handles, spawn and await syntax: NOT YET DECIDED.
```

Do not treat `actor` above as a canonical keyword. In the spike, test layout on this *hypothetical* body only after the concurrency surface is designed. In particular, a dynamic `inout self` cannot remain active across suspension; an async reader/writer continuation must own its state. Actor handles remain **DEFERRED POST NO-CYCLE**.

## 10. Candidate grammar fragment (layout only)

The fragment below describes an **experimental parser shape**, not the canonical `GRAMMAR.md`. `Expr`, `Type`, `Pattern`, `Attribute`, `CaptureList` etc. reuse the language's existing grammar except where layout is explicitly shown.

```ebnf
File             ::= (Item | Statement | NEWLINE)* EOF
Item             ::= Attribute* (NamedFunction | Struct | Enum | Interface | Extend)
NamedFunction    ::= 'fn' IDENT '(' Params? ')' ReturnType? Suite
Struct           ::= 'struct' IDENT GenericDecl? ':' NEWLINE INDENT StructItem+ DEDENT
StructItem       ::= Field NEWLINE | NamedFunction
Enum             ::= 'enum' IDENT GenericDecl? ':' NEWLINE INDENT EnumVariant+ DEDENT
Interface        ::= 'interface' IDENT ':' NEWLINE INDENT InterfaceItem+ DEDENT
Extend           ::= 'extend' Type ':' NEWLINE INDENT NamedFunction+ DEDENT
Suite            ::= ':' NEWLINE INDENT Statement+ DEDENT
IfExpr           ::= 'if' ('comptime')? Expr Suite ('else' (Suite | IfExpr))?
MatchExpr        ::= ('comptime')? 'match' Expr ':' NEWLINE INDENT MatchArm+ DEDENT
MatchArm         ::= Pattern Guard? '=>' (Expr NEWLINE | NEWLINE INDENT Statement+ DEDENT)
ForExpr          ::= 'for' IDENT 'in' Expr ('if' Expr)? Suite
Closure          ::= 'fn' ('[' CaptureList ']')? '(' Params? ')' ReturnType? Suite
ComptimeBlock    ::= 'comptime' Suite
ReturnType       ::= '->' Type
Guard            ::= 'if' Expr
```

**Deliberate grammar gaps for the spike:**

- `Suite` vs expression-block continuation and a single-line suite: **single-line suite not proposed initially**.
- Statements carrying a last-expression result need AST treatment, especially `let name = comptime:` and nested `if`/`match`.
- `MatchArm` with a multiline body lacks a colon; the alternative `pattern =>:` should be compared if needed rather than assumed.
- Function return/parameter markers and `Type(T)` must remain unambiguous with colon-as-suite.
- Struct **literals** keep `{...}`; only statement suites switch to significant indentation.
- Generic type names, associated-type projections, `Result`, `Option`, guards and specialized `@deny(...)` retain their separate authoritative grammars.
- The snippet omits imports, attributes' exact placement restrictions, `pub`, `extern`, structs' default values, all expression precedence and exhaustive pattern productions. These are **not removed** by the proposal; they must be reused when the prototype parser is built.
- `ref self`, `consume self`, general `ref`/`consume` parameter spellings, resource declarations, actors and async APIs are **SURFACE OPEN** in canonical Tempra. They cannot become authoritative merely by appearing in a syntax experiment.

## 11. Prototype acceptance criteria

A future spike should compare both syntaxes against identical typed AST/HIR fixtures:

1. **Semantic identity:** same typechecking, capability checks, ownership and diagnostics categories for equivalent programs; Memory & Ownership v0.4 remains authoritative.
2. **Determinism:** reproducible token/AST results for identical sources; no hidden whitespace-sensitive alternative meanings.
3. **Lexer/parser simplicity:** bounded additional complexity for `INDENT`, `DEDENT`, `NEWLINE`, multiline expressions and error recovery.
4. **Editor ergonomics:** formatter stability, copy/paste, indentation recovery, syntax highlighting and LSP diagnostics.
5. **Real program coverage:** structs, generics, interface constraints, `comptime` selection, match, nested closures, return-by-final-expression, `for` expressions, module imports and capability attributes.
6. **Comparative measurements:** parser speed, cold-start, diagnostic quality, source churn, and line counts on representative Tempra projects.
7. **Scope:** implement in an isolated spike branch *after* the VM is stable, without changing canonical syntax by default.

**Go/no-go:** adopt only after explicit review of the measured outcomes and a separately approved Surface Spec and `GRAMMAR.md` revision. Until then the brace-based syntax is the source of truth.

## 12. Pending questions (for the spike, not the current grammar review)

- Should layout replace all statement blocks or only declarations/control flow?
- Should an explicit end marker or optional `{...}` fallback be permitted? The initial experiment assumes **no mixed block syntax**.
- Should `match` arms with multiline bodies use `=> NEWLINE INDENT` or `=>:`?
- What is the precise indentation policy for multiline calls, chained method calls and operators?
- How should the formatter handle generated code, macro-free comptime output and embedded multi-line strings?
- Once actor syntax exists, can its body reuse the exact same `Suite` production without special cases?

---

**Invariant:** This is a *syntax proposal*, not a second Tempra language. Value semantics, borrowing, ownership transfers, effects, no strong cycles and backend equivalence must remain exactly those of the normative specifications.
