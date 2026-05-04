//! WebAssembly output backend.
//!
//! Translates Tempra IR into a `.wasm` binary. This crate is strictly an
//! *output* backend — it produces Wasm artifacts but never executes them.
//! Execution of Wasm modules is handled by `tempra-wasm-host`.

pub mod emit;
pub mod lower;
