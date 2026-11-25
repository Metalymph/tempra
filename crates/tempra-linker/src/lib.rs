//! Native executable linking.
//!
//! Takes the object code produced by a native AOT backend and links it into
//! a standalone executable by invoking the system linker or performing
//! minimal self-contained linking.

pub mod exe;
