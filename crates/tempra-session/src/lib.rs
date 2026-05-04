//! Compilation session state.
//!
//! A [`Session`] is the top-level context threaded through every stage of the
//! Tempra pipeline. It holds the input file path, the selected compilation
//! target, and will eventually carry the diagnostics sink, interning tables,
//! and any other cross-cutting concerns.

use std::path::PathBuf;

/// Describes which output target the compiler should produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Emit a native binary via Cranelift.
    Native,
    /// Emit a `.wasm` module via the Wasm backend.
    Wasm32Wasi,
}

/// Top-level compilation session.
///
/// Created once per invocation of `tempra run`, `tempra build`, etc.
pub struct Session {
    /// Path to the root source file being compiled.
    pub input: PathBuf,
    /// The output target for this compilation.
    pub target: Target,
}

impl Session {
    pub fn new(input: PathBuf, target: Target) -> Self {
        Self { input, target }
    }
}
