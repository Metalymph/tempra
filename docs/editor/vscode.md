# VS Code Integration

You can integrate `tempra-lsp` into VS Code using a simple extension.

## 1. Simple Client Setup

If you have a generic LSP client extension like `LSP Multi Language Server` or similar, you can configure it directly.
Otherwise, the standard approach is to create a tiny VS Code extension that registers the `tempra` language and launches the language client.

### Expected Configuration

- **Language ID:** `tempra`
- **File Extension:** `.tmpr`
- **Executable Command:** `/absolute/path/to/tempra/target/debug/tempra-lsp`

## 2. Minimal Dev-Host Extension

If you are developing the compiler and want to test it locally without publishing an extension:

1. Create a minimal VS Code extension using `yo code`.
2. Add language contribution to `package.json`:
   ```json
   "contributes": {
     "languages": [{
       "id": "tempra",
       "extensions": [".tmpr"]
     }]
   }
   ```
3. Use `vscode-languageclient` in `extension.ts`:
   ```typescript
   import { ExtensionContext } from 'vscode';
   import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

   let client: LanguageClient;

   export function activate(context: ExtensionContext) {
       const serverOptions: ServerOptions = {
           command: "/path/to/tempra/target/debug/tempra-lsp"
       };

       const clientOptions: LanguageClientOptions = {
           documentSelector: [{ scheme: 'file', language: 'tempra' }]
       };

       client = new LanguageClient(
           'tempra-lsp',
           'Tempra Language Server',
           serverOptions,
           clientOptions
       );

       client.start();
   }

   export function deactivate(): Thenable<void> | undefined {
       if (!client) return undefined;
       return client.stop();
   }
   ```
4. Press `F5` to open the Extension Development Host.

## 3. Manual Smoke Checklist

Open `examples/lsp_smoke.tmpr` in the editor:
- [ ] Hover over `Point` and `add` to see Markdown hover information.
- [ ] Use `Go to Definition` (F12) on `p.x` or `add` to jump to their declarations.
- [ ] Open the Outline or run `Go to Symbol in Editor` to see the document symbols.
- [ ] Uncomment the `broken()` function at the bottom and verify that a red squiggly line appears for type errors.
