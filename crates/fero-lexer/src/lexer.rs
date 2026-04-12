use crate::{Token, TokenKind};
use fero_span::{SourceId, Span};

pub struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
    source_id: SourceId,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            cursor: 0,
            source_id: SourceId(0),
        }
    }

    pub fn tokenize(mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        for part in self.source.split_whitespace() {
            let start = self.cursor;
            let end = start + part.len();
            self.cursor = end + 1;
            let kind = if part.chars().all(|ch| ch.is_ascii_digit()) {
                TokenKind::Int
            } else {
                TokenKind::Ident
            };
            tokens.push(Token {
                kind,
                lexeme: part.to_string(),
                span: Span::new(self.source_id, start, end),
            });
        }
        tokens.push(Token {
            kind: TokenKind::Eof,
            lexeme: String::new(),
            span: Span::new(self.source_id, self.source.len(), self.source.len()),
        });
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::Lexer;
    use crate::TokenKind;

    #[test]
    fn tokenizes_words_and_numbers() {
        let tokens = Lexer::new("io.println 42").tokenize();
        assert_eq!(tokens[0].kind, TokenKind::Ident);
        assert_eq!(tokens[1].kind, TokenKind::Int);
        assert_eq!(
            tokens.last().map(|token| &token.kind),
            Some(&TokenKind::Eof)
        );
    }
}
