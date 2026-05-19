# Tempra Tooling, Diagnostics & LSP Foundation

This document details the structured diagnostics and symbol indexing subsystems introduced in **Phase 18.0B**. These features lay the groundwork for upcoming IDE/LSP clients (`tempra-lsp`) and facilitate reliable static analysis for human developers and AI coding agents.

---

## 1. Unified Diagnostic Model

The `tempra-diagnostics` crate provides a unified, highly precise structural model for compiling and reporting program warnings and errors.

```rust
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub span: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}
```

---

## 2. Structured JSON Formats

Every syntax (parser) and semantic (typechecking) diagnostic can be serialized into a robust, structured JSON format designed for IDE consumption.

### JSON Diagnostics Schema

```json
{
  "diagnostics": [
    {
      "severity": "error",
      "code": null,
      "message": "Type mismatch: expected String, found Int64",
      "labels": [
        {
          "span": {
            "file": "examples/jit_heap/string_len.tmpr",
            "start": 54,
            "end": 62
          },
          "message": null
        }
      ],
      "notes": []
    }
  ]
}
```

---

## 3. Symbol Indexing & Signatures

Tempra features a dedicated symbol extraction pass that crawls the compiler's High-level Intermediate Representation (HIR) to map structural symbols, generating precise, beautifully formatted signatures for autocomplete, documentation, and outlines.

### Extracted Symbol Kinds
* `function`: Top-level and local functions.
* `struct`: Declared data structures.
* `enum`: Algebraic datatypes and variants.
* `method`: Extended functions defined inside `extend` blocks.
* `extension`: Type extension blocks.
* `variable`: Variables/bindings (future-proofing LSP local variables).

### JSON Symbols Schema

```json
{
  "symbols": [
    {
      "name": "Point",
      "kind": "struct",
      "span": {
        "file": "path/to/source.tmpr",
        "start": 10,
        "end": 80
      },
      "signature": "struct Point"
    },
    {
      "name": "add",
      "kind": "function",
      "span": {
        "file": "path/to/source.tmpr",
        "start": 90,
        "end": 150
      },
      "signature": "fn add(a: Int64, b: Int64) -> Int64"
    },
    {
      "name": "Point.distance",
      "kind": "method",
      "span": {
        "file": "path/to/source.tmpr",
        "start": 180,
        "end": 260
      },
      "signature": "fn distance(self: Point) -> Int64"
    }
  ]
}
```

---

## 4. CLI Subcommands

Three primary commands handle structural output:

### A. Code Integrity Check
Verify both formatting and correctness, returning structured diagnostics upon failure:
```bash
tempra check --json examples/jit_heap/string_len.tmpr
```

### B. Extended Diagnostics
Emits only compilation warnings and errors in clean JSON payloads:
```bash
tempra diagnostics --json examples/jit_heap/string_len.tmpr
```

### C. Symbol Outlining
Dumps all structural symbols and their signatures present within the source file:
```bash
tempra symbols --json examples/jit_heap/string_len.tmpr
```

---

## 5. Verification Tasks

Verify all components and command structures using the `tooling-check` and `tooling-smoke` tasks:

```bash
just tooling-check  # Runs both compiler and CLI integration suites
just tooling-smoke  # Smoke-tests the JSON output from the command line
```

## 6. In-Memory Analysis Session Layer

To power upcoming Language Server implementations (`tempra-lsp`) and facilitate low-latency IDE features without heavy disk read/write loops, **Phase 18.1A** and **Phase 18.1B** introduce an in-memory document state manager: `tempra-analysis`.

This crate acts as the "brain" for editor features and exposes highly efficient, cached query-driven compiler pipelines completely decoupled from standard JSON-RPC or network transports.

### Document Lifecycle Management
- `open_document(uri, content, version)`: Registers a document in-memory, triggers parses/checks, and caches diagnostics & symbols.
- `update_document(uri, content, version)`: Updates in-memory snapshots and invalidates/refreshes caches.
- `close_document(uri)`: Evicts active session states to release memory.

### Dual Coordinate System
To avoid coordinate shift debt, Tempra splits positions into internal-first UTF-8 representations and outer LSP-style UTF-16 representations:
1. **Internal Positions (`TextPosition`)**:
   - `line`: 0-indexed line.
   - `byte_character`: 0-indexed UTF-8 byte offset within the line.
   - All core semantic lookup queries (`hover_at_position`, `definition_at_position`) use internal UTF-8 coordinates.
2. **External Positions (`LspPosition`)**:
   - `line`: 0-indexed line.
   - `character`: 0-indexed UTF-16 code-unit offset within the line (LSP standard).
   - Conversions (`lsp_to_text_position`, `text_to_lsp_position`) handle Unicode mapping rules flawlessly: Accents (`è`), CJK ideographs, and Emojis (`🐶` taking 2 code units in UTF-16 but 4 bytes in UTF-8).

### Outer LSP Adapter Queries
Outside callers and LSP servers can query the engine directly using external UTF-16 coordinates through adapter query wrappers:
- `hover_at_lsp_position(uri, pos)`
- `definition_at_lsp_position(uri, pos)`
- `symbol_at_lsp_position(uri, pos)`
These translate positions, execute lookups against the cached spatial and lexical symbol indexes (`SymbolIndex`), and return results without exposing UTF-16 complexities to the semantic core.

---

## 7. Scope-Aware Semantic Symbol Resolution (Phase 18.1C)

To support precise, context-aware editor features (Hover, Goto-definition, future autocompletion and renaming), **Phase 18.1C** introduces the `SemanticIndex` and `SemanticResolver` within `tempra-analysis`. 

This system performs a full AST traversal of every document to construct a scope hierarchy that correctly handles:
- **Lexical Scopes**: Scopes are correctly nested (using blocks `{ ... }`). Lookups search from the innermost block outwards.
- **Shadowing**: Local bindings (`let`/`var`) declared in inner blocks shadow outer declarations.
- **Function Parameters & Loop Bindings**: Function arguments and loop iterator variables (`for item in items`) are resolved accurately to their specific scopes.
- **Best-Effort Struct Fields & Enum Variants**: Struct field accesses (`point.x`) and enum variant patterns are mapped to their definitions.
- **Precise AST-Derived Signatures**: Signatures are extracted directly from the source code slice matching the AST node, producing beautifully exact declarations (e.g. `fn add(a: Int64) -> Int64` or `let x: Int64` matching exact types).

---

## 8. Typecheck Facts Exposure & Type-Aware Analysis Bridge (Phase 18.1D)

To enable rich, authoritative editor features without duplicating typechecking logic inside `tempra-analysis`, **Phase 18.1D** introduces a structured `TypecheckFacts` exposure system:

### A. Authoritative Compiler Facts
The typechecker (`tempra-typeck`) computes a stable `TypecheckFacts` struct during execution and exposes it via `AnalysisResult`:
- `expr_types`: Maps all expression spans to their authoritative checked `Type`.
- `binding_types`: Maps all binding and parameter declaration spans to their checked `Type`.
- `resolved_symbols`: Maps identifier reference spans to their corresponding definition spans.
- `member_resolutions`: Maps member access reference spans (`point.x`) to their field definition spans.
- `variant_resolutions`: Maps enum variant reference spans (`Option.Some`) to their variant definition spans.
- `method_resolutions`: Maps extension method invocation spans to their exact method definition spans.

### B. Precise Spatial Binding Lookups
During AST-to-HIR lowering, Tempra captures precise declaration name/binding/member spans inside the HIR representation. This ensures that every definition's exact name spatial bound is perfectly recorded in typechecker maps.

### C. Type-Aware Rich Hover Enrichment
The in-memory analysis engine integrates the exposed `TypecheckFacts` dynamically inside query endpoints. When performing a Hover lookup:
1. It queries the spatial lexical index (`SemanticIndex`) to get the symbol's structure and signature prefix (e.g., `"let x: Int"`, `"Point.x: Int"`, `"parameter name: String"`).
2. It queries `TypecheckFacts` to find the exact, authoritative type computed by the compiler (e.g., `Int64`, `String`).
3. If both are present, it merges them beautifully by replacing the lexical suffix with the authoritative typechecker result, ensuring the user gets exact typing details even for complex, inferred nested local bindings!

---

## 9. Thin LSP Server Adapter (Phase 18.1E)

**Phase 18.1E** ships the first real LSP binary for Tempra: `tempra-lsp`.

### Architecture

```text
Editor --JSON-RPC--> tempra-lsp --> tempra-analysis --> tempra-driver
                      (adapter)       (IDE brain)        (compiler)
```

- **`tempra-analysis`** = IDE brain. Contains all semantic intelligence, scope resolution, typechecker-facts queries, and coordinate conversion logic.
- **`tempra-lsp`** = thin adapter. Forwards LSP requests to the brain and translates the results to LSP protocol types. Contains **zero compiler or typechecker logic**.

### Supported capabilities (Phase 18.1E)

| Capability | Status |
|---|---|
| `textDocument/didOpen` | ✅ Full sync |
| `textDocument/didChange` | ✅ Incremental sync (Phase 18.3A) |
| `textDocument/didClose` | ✅ Publishes empty diagnostics |
| `textDocument/hover` | ✅ Markdown |
| `textDocument/definition` | ✅ Goto Definition |
| `textDocument/documentSymbol` | ✅ Flat symbol list |
| `textDocument/completion` | ✅ Context-aware autocomplete & member completions (triggered by `.` and manual invocation). Features deterministic group sorting and detail formatting. |
| `textDocument/semanticTokens/full` | ✅ Full document semantic highlighting |
| `textDocument/references` | ✅ Find references (Phase 18.3B) |
| `textDocument/documentHighlight` | ✅ Highlight occurrences (Phase 18.3B) |
| `textDocument/prepareRename` / `textDocument/rename` | ✅ Safe local rename (Phase 18.3C) |

### Semantic Tokens (Phase 18.2D)

The LSP server provides precise semantic token highlighting for Tempra source files:
- **Scope**: Full-document requests (`textDocument/semanticTokens/full`). Range-based queries are not supported.
- **Highlights**: Emits tokens for keywords, string literals, number literals, operators, type declarations/references, parameter declarations, local variables, properties (fields), and enum members (variants).
- **Comments**: Not emitted in the MVP.
- **Token Legend**: Advertises standard LSP types: `namespace`, `type`, `struct`, `enum`, `interface`, `typeParameter`, `parameter`, `variable`, `property`, `enumMember`, `function`, `method`, `keyword`, `string`, `number`, `operator`.
- **Deduplication**: Handles token priority and resolves overlapping spans deterministically.

### Autocomplete & Member Completions (Phase 18.2C)

The completion system supports trigger character `.` to invoke autocomplete when writing member access expressions (`instance.field` or `instance.method()`).

To improve user experience, completions are:
1. **Sorted Deterministically**: Ordered by group prefixes (`sort_text`) as follows:
   - Keywords (`01_keyword_`)
   - Local variables/bindings (`02_local_`)
   - Struct fields (`03_field_`)
   - Extension methods (`04_method_`)
   - Functions (`05_function_`)
   - Types (`06_type_`)
   - Modules/namespaces (`07_module_`)
   - Enums (`08_enum_`)
   - Structs (`09_struct_`)
2. **Context-Aware**: Member access autocompletes filter out irrelevant keywords, local bindings, and global symbols, only presenting fields and methods available on the receiver's type.
3. **Formatted Presentation Details**:
   - Keywords: `"keyword"`
   - Types: `"type"`
   - Modules: `"module"`
   - Fields: `"field: Type"` (e.g. `field: Int64`)
   - Methods/Functions: Signature if available
   - Locals: Binding signature/type if available

### UTF-16 coordinate safety

All LSP positions from the editor pass through the `LspPosition` wrappers in `tempra-analysis`'s `LineIndex`. The adapter (`tempra-lsp`) **never** performs manual UTF-16/UTF-8 conversions. Span-to-range conversion for diagnostics, definition, and symbols uses the `session.span_to_lsp_range()` helper in `tempra-analysis`.

### Incremental Sync & Reanalysis (Phase 18.3A)

The LSP server uses incremental text synchronization and lazy reanalysis to maximize editor responsiveness:
- **Synchronization**: Advertises `TextDocumentSyncKind::INCREMENTAL`. When a document is modified, the editor sends a sequence of range changes containing only the modified segments rather than the full source code.
- **Lazy Recomputation**: Editor modifications invalidate cached diagnostic, completion, and definition states immediately, but recomputation is deferred until a query is executed. This eliminates redundant analysis cycles on typing.

### References & Document Highlights (Phase 18.3B)

The LSP server tracks identifier declarations and usages within the document's semantic index:
- **References**: Triggers `textDocument/references` to lookup all definition/usage locations. Supports the `include_declaration` option to optionally exclude the declaration span.
- **Document Highlights**: Triggers `textDocument/documentHighlight` to highlight matching references under the cursor. Declaration spans are assigned `Write` highlights, while usages/references are assigned `Read` highlights.
- **Shadowing Correctness**: Accurately scope-resolves identifiers. If an inner scope local variable shadows an outer parameter/variable, looking up references/highlights for the inner variable does not include outer references, and vice-versa.

### Workspace Edit & Rename (Phase 18.3C)

The LSP server provides safe local variable and parameter renaming using a workspace edit planner:
- **Rename Preparation**: `textDocument/prepareRename` verifies that the symbol under the cursor is safely renameable (only local variables, parameters, and loop bindings are allowed in the MVP) and returns the precise identifier span.
- **Rename Execution**: `textDocument/rename` generates a `WorkspaceEdit` with a list of `TextEdit`s to update the declaration and all reference spans.
- **Robust Verification**: Validates edits to prevent overlapping edits, filters duplicate spans, and reverse-sorts edits (descending start offset) for shift-free, safe application.
- **Lexical Isolation**: Rejects keywords or invalid identifier formats (e.g. starting with digits) and restricts renaming strictly to semantic-index resolved local bindings.

### Running the server

```bash
# Standalone binary (add to PATH):
tempra-lsp

# Or via cargo during development:
cargo run -p tempra-lsp
```

Communicates over **stdin / stdout** using the standard LSP JSON-RPC transport.

### Adding to an editor (VS Code example)

```json
{
  "languageClient": {
    "command": "tempra-lsp",
    "filenamePatterns": ["*.tmpr"]
  }
}
```

