//! Structural type checking for Tempra.
//!
//! Operates on HIR and validates that every expression is well-typed.
//!
//! ### Technical Note: Generics v1 Stabilization
//! - Generic parameter identity is semantic (via GenericParamId), not string-based.
//! - Substitution is structural and recursive.
//! - Specialization metadata is attached directly to call expressions at the call site.
//! - Inference is strict and coercion-free.

pub mod check_expr;
pub mod check_file;
pub mod check_item;
pub mod check_stmt;
pub mod context;
pub mod env;
pub mod error;
pub mod pattern;
pub mod projection;
pub mod types;
pub mod facts;

pub use check_file::{check_file, check_file_with_context, check_file_with_context_always};
pub use env::VariableInfo;
pub use error::TypeError;
pub use types::Type;
pub use facts::TypecheckFacts;
