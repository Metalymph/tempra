//! Recursive-descent parser for Tempra.
//!
//! Consumes a [`Lexer`](tempra_lexer::Lexer) token stream and produces an
//! AST ([`tempra_ast`]) representing the syntactic structure of the program.

pub mod grammar;
pub mod parser;

pub use parser::{parse_source, Parser};
