//! Native executable linking.
//!
//! Takes the object code produced by the Cranelift backend and links it into
//! a standalone executable by invoking the system linker or performing
//! minimal self-contained linking.

pub mod exe;
