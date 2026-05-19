# Zed Integration

You can integrate `tempra-lsp` into Zed, though it currently requires adding an extension to Zed.

## 1. Extension Setup

Zed extensions are written in Rust/Wasm. Until a packaged Zed extension for Tempra is published, you would need to create a local Zed extension that registers the `tempra` language and configures the LSP.

### Expected Configuration

- **Language ID:** `tempra`
- **File Extension:** `.tmpr`
- **LSP Command:** `/absolute/path/to/tempra/target/debug/tempra-lsp`

## 2. Integration Notes & TODOs

- **TODO:** Create a standard Zed extension for Tempra once the LSP features are more finalized.
- For now, you can test Zed integration if you are familiar with building local Zed extensions by pointing the `command` in the extension's LSP configuration to the `tempra-lsp` binary.

## 3. Manual Smoke Checklist

Once integrated, open `examples/lsp_smoke.tmpr` in Zed:
- [ ] Hover over `Point` and `add` to see Markdown hover information.
- [ ] Use `Go to Definition` on `p.x` or `add` to jump to their declarations.
- [ ] Open the symbol outline to see the document symbols.
- [ ] Type `Point.` (triggering autocomplete on `.`) or `re...` to verify auto-completion appears.
- [ ] Uncomment the `broken()` function at the bottom and verify that diagnostics appear for type errors.
