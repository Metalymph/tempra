//! Structural type checking for Fero.
//!
//! Operates on HIR and validates that every expression is well-typed.
//! This is a v0 implementation: no generics, no traits, no inference beyond
//! literal defaults.

pub mod types;
pub mod error;
pub mod env;
pub mod context;
pub mod check_expr;
pub mod check_stmt;
pub mod check_item;
pub mod check_file;

pub use check_file::check_file;
pub use error::TypeError;
pub use types::Type;
