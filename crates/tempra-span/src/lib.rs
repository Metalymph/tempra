//! Byte-offset spans and source file tracking for the Tempra compiler.
//!
//! Every token, AST node, and diagnostic produced by the compiler carries a
//! [`Span`] that points back into the original source text. This crate owns
//! the definition of that span as well as the [`SourceFile`] container used
//! to associate a file path with its contents.

pub mod source;
pub mod span;

pub use source::SourceFile;
pub use span::Span;
