//! Mid-level Intermediate Representation.
//!
//! The MIR makes control flow explicit: the program is represented as a
//! control-flow graph of [`BasicBlock`]s connected by terminators. This is the
//! level at which borrow-like analysis, ARC insertion, and most
//! optimizations operate before lowering to the flat Tempra IR.

pub mod block;
pub mod lower;
