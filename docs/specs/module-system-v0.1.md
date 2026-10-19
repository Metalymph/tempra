# Tempra Module System v0.1

The Tempra module system provides a disciplined and reproducible way to organize code, manage dependencies, and distinguish between script-first and application-first development.

## 1. Design Goals
- **Reproducibility:** Dependencies in App mode must be locked and deterministic.
- **Simplicity:** No complex search path logic.
- **Script Ergonomics:** Low friction for single-file scripts.

## 2. Import Categories

### Standard Library Imports
Accessed via the `std/` prefix.
```tempra
import std/io
import std/fs
```

The current compiler resolves these imports from one toolchain-selected `std/`
root. The module resolver receives that root as an explicit input and does not
search the current working directory. The default compiler configuration locates
the root relative to its executable; embedding callers may select it explicitly.

### Relative Imports
Used for local file organization.
```tempra
import ./utils
import ../models/user
```

### Package Imports
Refer to external dependencies managed by the Tempra package manager.
```tempra
import pkg/json
import pkg/http
```

### Future Script-only URL Imports
URL imports may be supported in the future exclusively for **Script Mode** to facilitate quick sharing. They will not be allowed in App Mode to ensure build stability.
```tempra
// Future direction (v0.x)
import "https://tempra.dev/pkg/math.tempra" 
```

## 3. Project Structure

### Script Mode
In script mode, any `.tempra` file can be executed directly. The module system treats the folder containing the script as the root for relative imports.

### App Mode
Requires a `tempra.mod` file at the root. The entrypoint is `main.tempra` or a file specified in the manifest containing a `fn main()`.
```
project/
├── tempra.mod         # Dependency and metadata
├── main.tempra        # Entrypoint (App mode)
└── src/
    └── core.tempra    # Sub-module
```

## 4. Visibility
By default, all members of a module are private. The `pub` keyword is used to expose structs, enums, and functions to other modules.
```tempra
pub struct User {
    pub name: String
    id: Int // Private to the module
}
```

## 5. Main Entrypoint
- In **App Mode**, a `main` function is required.
- Command-line arguments are provided via `std/env`.
- The v0.1 entry-point result contract is:
  - `Unit` (or no declared return type): successful exit with code `0`.
  - `Int`: process exit code. The portable range is `0..=255`. A value outside
    that range is rejected with a diagnostic; it is never truncated or wrapped.
  - `Result`: `Ok` means successful exit with code `0`, regardless of its
    payload. `Err` reports `Uncaught error: <payload>` to stderr and exits
    nonzero.
  - Any other return type is rejected at the CLI entry point.
- Explicit `exit(n)` keeps its existing runtime semantics and is not governed by
  the `Int`-return portability policy. The experimental VM currently reports
  `exit` as unsupported.
- Script Mode is unchanged: top-level statements are not entry-point results.
- Lower-level execution APIs continue to return raw HIR/VM values; entry-point
  normalization applies only to the CLI-facing execution adapters.

## 6. Future Directions
- **Wasm Component Imports:** Support for importing Wasm components via a specialized syntax, separate from native source imports, to leverage the Wasm Component Model.
- **Namespacing:** Explicit handling of name collisions between packages.
- **Vendor Directory:** Provision for local caching of all external dependencies to ensure offline reproducibility.
