//! Abstract Syntax Tree definitions for Fero.
//!
//! The AST is a faithful, lossless representation of the parsed source code.
//! It preserves every syntactic detail (parentheses, formatting hints) so
//! that tools like `fero fmt` can reconstruct the original text. Later
//! pipeline stages ([`fero_hir`]) desugar this into a simpler form.

pub mod expr;
pub mod item;
pub mod ty;
