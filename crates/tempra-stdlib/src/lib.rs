//! Tempra standard library, implemented in Rust.
//!
//! Each submodule corresponds to a Tempra stdlib namespace (`io`, `fs`,
//! `process`). Functions here are registered into the interpreter and
//! compiler as built-in intrinsics.

pub mod fs;
pub mod io;
pub mod process;
