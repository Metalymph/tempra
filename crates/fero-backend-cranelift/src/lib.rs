//! Cranelift-based native AOT backend.
//!
//! Translates Fero IR into native machine code using the Cranelift code
//! generator. This backend is used exclusively for native AOT compilation
//! (`fero build --target native`) and must never be used for Wasm output.

pub mod abi;
pub mod lower;
