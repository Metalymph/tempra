//! Fero standard library, implemented in Rust.
//!
//! Each submodule corresponds to a Fero stdlib namespace (`io`, `fs`,
//! `process`). Functions here are registered into the interpreter and
//! compiler as built-in intrinsics.

pub mod io;
pub mod fs;
pub mod process;
