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


