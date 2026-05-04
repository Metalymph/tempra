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
- `main` can optionally return an `Int` for exit codes or a `Result` for error reporting.
- Command-line arguments are provided via `std/env`.

## 6. Future Directions
- **Wasm Component Imports:** Support for importing Wasm components via a specialized syntax, separate from native source imports, to leverage the Wasm Component Model.
- **Namespacing:** Explicit handling of name collisions between packages.
- **Vendor Directory:** Provision for local caching of all external dependencies to ensure offline reproducibility.
