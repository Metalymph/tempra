//! Tempra IR — the final, flat, platform-neutral instruction format.
//!
//! Every backend (interpreter, Wasm, Cranelift) consumes Tempra IR. The IR is
//! a simple list of instructions grouped into functions inside a [`Module`].
//! No backend is permitted to alter program semantics; all semantic
//! optimizations must happen in `tempra-opt` before reaching this stage.

pub mod function;
pub mod instr;
pub mod module;
pub mod ty;
