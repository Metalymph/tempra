//! Stack-based interpreter for Fero IR.
//!
//! The interpreter is the default execution mode (`fero run`). It evaluates
//! a [`Module`](fero_ir::module::Module) directly without any compilation
//! step, offering instant cold-start at the cost of lower peak throughput
//! compared to AOT backends.

pub mod eval;
pub mod frame;
