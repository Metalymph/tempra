//! Structural type checking and inference for Fero.
//!
//! Operates on HIR and verifies that every expression is well-typed according
//! to Fero's structural interface rules. The result is a type-annotated HIR
//! that downstream passes can rely on.

pub mod checker;
pub mod context;
