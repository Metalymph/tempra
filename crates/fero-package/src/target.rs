/// The compilation target architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    /// Produce a native binary via the Cranelift backend.
    Native,
    /// Produce a `.wasm` module via the Wasm backend.
    Wasm32Wasi,
}

/// Controls the optimisation strategy applied during compilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    /// Maximise runtime performance (aggressive optimisations, SIMD, ARC elision).
    Fast,
    /// Minimise binary size (size-oriented passes, symbol stripping).
    Light,
}
