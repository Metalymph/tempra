//! Structural type checking for Fero.
//!
//! Operates on HIR and validates that every expression is well-typed.
//!
//! ### Technical Note: Generics v1 Stabilization
//! - Generic parameter identity is semantic (via GenericParamId), not string-based.
//! - Substitution is structural and recursive.
//! - Specialization metadata is attached directly to call expressions at the call site.
//! - Inference is strict and coercion-free.

pub mod types;
pub mod error;
pub mod env;
pub mod context;
pub mod check_expr;
pub mod pattern;
pub mod check_stmt;
pub mod check_item;
pub mod check_file;

pub use check_file::check_file;
pub use error::TypeError;
pub use types::Type;
pub use env::VariableInfo;
