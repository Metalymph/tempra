# Tempra Editor Integration

This directory contains integration guides for using `tempra-lsp` in various editors:

- [VS Code](vscode.md)
- [Neovim](neovim.md)
- [Zed](zed.md)

## Common Requirements

- `tempra-lsp` must be built. The easiest way is to build it in debug mode:
  ```bash
  cargo build -p tempra-lsp
  ```
- The resulting binary will be located at `target/debug/tempra-lsp` relative to the workspace root.
- The binary accepts JSON-RPC over `stdio`.
- Language ID: `tempra`
- File Extension: `.tmpr`

## Feature Support (Phase 18.1F)

Currently supported LSP capabilities:
- Diagnostics (Full document sync)
- Hover (Rich markdown)
- Goto Definition
- Document Symbols

Not yet implemented:
- Completion
- Rename
- References
- Semantic Tokens
- Code Formatting
- Incremental text sync
