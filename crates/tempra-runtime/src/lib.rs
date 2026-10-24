//! Runtime semantics shared across Tempra interpreters and backends.
//!
//! [`Value`] is the dynamically-typed container used by the interpreter to
//! store computation results at runtime. Backend-compiled code does not use
//! this type directly — it maps Tempra types to native representations — but
//! the definition lives here so that the stdlib and interpreter can share it.

pub mod arithmetic;

pub use arithmetic::{
    add_i64, add_u64, div_i64, div_u64, mul_i64, mul_u64, neg_i64, sub_i64, sub_u64,
    Int64ArithmeticError, UInt64ArithmeticError,
};

/// A dynamically-typed Tempra value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 64-bit signed integer.
    Int(i64),
    /// A 64-bit IEEE 754 float.
    Float(f64),
    /// A UTF-8 string.
    String(String),
    /// A boolean.
    Bool(bool),
    /// The unit value (returned by expressions with no meaningful result).
    Unit,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Unit => write!(f, "()"),
        }
    }
}
