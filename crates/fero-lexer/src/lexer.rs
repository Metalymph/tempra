use crate::token::Token;

/// A cursor-based lexer that walks through source text one character at a time.
///
/// Call [`Lexer::next_token`] repeatedly to consume tokens until [`Token::Eof`]
/// is returned.
pub struct Lexer {
    /// The complete source text being scanned.
    source: Vec<char>,
    /// Current read position (index into `source`).
    cursor: usize,
}

impl Lexer {
    /// Create a new lexer for the given source string.
    pub fn new(input: String) -> Self {
        Self {
            source: input.chars().collect(),
            cursor: 0,
        }
    }

    /// Advance past the current token and return it.
    ///
    /// Returns [`Token::Eof`] when the entire input has been consumed.
    pub fn next_token(&mut self) -> Token {
        if self.cursor >= self.source.len() {
            return Token::Eof;
        }
        // TODO: implement character-by-character scanning
        self.cursor = self.source.len();
        Token::Eof
    }
}
