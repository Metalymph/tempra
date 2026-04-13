//! Recursive-descent parser for Fero.
//!
//! Consumes a [`Lexer`](fero_lexer::Lexer) token stream and produces an
//! AST ([`fero_ast`]) representing the syntactic structure of the program.

pub mod grammar;
pub mod parser;

pub use parser::{Parser, parse_source};
