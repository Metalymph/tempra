//! Lexical analysis for Fero source code.
//!
//! The lexer converts a raw source string into a flat sequence of [`Token`]s.
//! It is intentionally simple and hand-written — no generator — so that error
//! recovery and span tracking stay explicit and predictable.

pub mod lexer;
pub mod token;

pub use lexer::{Lexer, tokenize};
pub use token::{Token, TokenKind};
