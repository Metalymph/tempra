//! High-level Intermediate Representation.
//!
//! The HIR is a desugared, simplified view of the AST. Syntactic sugar
//! (e.g. `for..in` as a `loop` + iterator) is expanded here, and names are
//! resolved to definition sites. The HIR is the input to type checking.

pub mod expr;
pub mod item;
pub mod lower;
