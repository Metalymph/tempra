//! Wasm module host (future Wasmtime integration).
//!
//! Responsible for *executing* third-party `.wasm` modules from within a
//! Tempra program. This is the consumer side of WebAssembly — the producer
//! side lives in `tempra-backend-wasm`.

pub mod runtime;
