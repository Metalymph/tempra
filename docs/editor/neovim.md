# Neovim Integration

You can easily integrate `tempra-lsp` into Neovim using the native LSP client.

## 1. Native LSP Configuration

You can use `vim.lsp.start` within a `FileType` autocommand to automatically start the server when opening `.tmpr` files.

Add the following to your `init.lua`:

```lua
-- Register .tmpr files as the 'tempra' filetype
vim.filetype.add({
  extension = {
    tmpr = "tempra",
  },
})

-- Autocommand to start tempra-lsp for Tempra files
vim.api.nvim_create_autocmd("FileType", {
  pattern = "tempra",
  callback = function()
    -- Set this to the absolute path of your workspace target directory, 
    -- or ensure `tempra-lsp` is in your PATH.
    local cmd = { "/absolute/path/to/tempra/target/debug/tempra-lsp" }

    vim.lsp.start({
      name = "tempra-lsp",
      cmd = cmd,
      root_dir = vim.fs.root(0, { "Cargo.toml", ".git" }), -- adjust if necessary
      settings = {},
    })
  end,
})
```

## 2. Manual Smoke Checklist

Open `examples/lsp_smoke.tmpr` in Neovim:
- [ ] Hover over `Point` and `add` (usually `K`) to see Markdown hover information.
- [ ] Use Goto Definition (usually `gd`) on `p.x` or `add` to jump to their declarations.
- [ ] Open document symbols (e.g., using Telescope `lsp_document_symbols` or the native picker) to see the outline.
- [ ] Uncomment the `broken()` function at the bottom and verify that diagnostics appear (virtual text and signs).
