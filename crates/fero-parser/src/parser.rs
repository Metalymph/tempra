use anyhow::Result;
use fero_ast::item::Item;
use fero_lexer::Lexer;

/// The main parser state machine.
///
/// Wraps a [`Lexer`] and uses recursive descent to build the AST.
pub struct Parser {
    /// The token source driving this parse.
    lexer: Lexer,
}

impl Parser {
    /// Construct a parser that will read tokens from `lexer`.
    pub fn new(lexer: Lexer) -> Self {
        Self { lexer }
    }

    /// Parse an entire source file and return its top-level items.
    pub fn parse(&mut self) -> Result<Vec<Item>> {
        // TODO: implement top-level item parsing loop
        let _ = self.lexer.next_token();
        Ok(vec![])
    }
}
