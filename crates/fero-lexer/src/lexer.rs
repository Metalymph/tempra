use crate::token::{Token, TokenKind, keyword_kind};
use fero_span::Span;

pub struct Lexer<'src> {
    source: &'src str,
    bytes: &'src [u8],
    position: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            position: 0,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();
        
        let start = self.position;
        if start >= self.bytes.len() {
            return self.make_token(TokenKind::EndOfFile, start, start);
        }

        let byte = self.current_byte().unwrap();

        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_identifier_or_keyword(start),
            b'0'..=b'9' => self.lex_number(start),
            b'"' => self.lex_string(start),
            
            b'(' => { self.advance(); self.make_token(TokenKind::LeftParen, start, self.position) }
            b')' => { self.advance(); self.make_token(TokenKind::RightParen, start, self.position) }
            b'{' => { self.advance(); self.make_token(TokenKind::LeftBrace, start, self.position) }
            b'}' => { self.advance(); self.make_token(TokenKind::RightBrace, start, self.position) }
            b'[' => { self.advance(); self.make_token(TokenKind::LeftBracket, start, self.position) }
            b']' => { self.advance(); self.make_token(TokenKind::RightBracket, start, self.position) }
            b',' => { self.advance(); self.make_token(TokenKind::Comma, start, self.position) }
            b':' => { self.advance(); self.make_token(TokenKind::Colon, start, self.position) }
            b';' => { self.advance(); self.make_token(TokenKind::Semicolon, start, self.position) }
            
            b'-' => {
                self.advance();
                if self.current_byte() == Some(b'>') {
                    self.advance();
                    self.make_token(TokenKind::Arrow, start, self.position)
                } else {
                    self.make_token(TokenKind::Minus, start, self.position)
                }
            }
            b'=' => {
                self.advance();
                match self.current_byte() {
                    Some(b'=') => {
                        self.advance();
                        self.make_token(TokenKind::EqualEqual, start, self.position)
                    }
                    Some(b'>') => {
                        self.advance();
                        self.make_token(TokenKind::FatArrow, start, self.position)
                    }
                    _ => self.make_token(TokenKind::Equal, start, self.position)
                }
            }
            b'.' => {
                self.advance();
                if self.current_byte() == Some(b'.') {
                    self.advance();
                    self.make_token(TokenKind::DotDot, start, self.position)
                } else {
                    self.make_token(TokenKind::Dot, start, self.position)
                }
            }
            b'>' => {
                self.advance();
                if self.current_byte() == Some(b'>') {
                    self.advance();
                    self.make_token(TokenKind::ShiftRight, start, self.position)
                } else {
                    self.make_token(TokenKind::Greater, start, self.position)
                }
            }
            b'!' => {
                self.advance();
                if self.current_byte() == Some(b'=') {
                    self.advance();
                    self.make_token(TokenKind::BangEqual, start, self.position)
                } else {
                    self.make_token(TokenKind::Bang, start, self.position)
                }
            }
            b'&' => {
                self.advance();
                if self.current_byte() == Some(b'&') {
                    self.advance();
                    self.make_token(TokenKind::AndAnd, start, self.position)
                } else {
                    self.make_token(TokenKind::Invalid, start, self.position)
                }
            }
            b'|' => {
                self.advance();
                if self.current_byte() == Some(b'|') {
                    self.advance();
                    self.make_token(TokenKind::OrOr, start, self.position)
                } else {
                    self.make_token(TokenKind::Pipe, start, self.position)
                }
            }
            b'+' => { self.advance(); self.make_token(TokenKind::Plus, start, self.position) }
            b'*' => { self.advance(); self.make_token(TokenKind::Star, start, self.position) }
            b'/' => {
                self.advance();
                if self.current_byte() == Some(b'/') {
                    self.skip_line_comment();
                    self.next_token()
                } else {
                    self.make_token(TokenKind::Slash, start, self.position)
                }
            }
            b'<' => { self.advance(); self.make_token(TokenKind::Less, start, self.position) }
            
            _ => {
                self.advance();
                self.make_token(TokenKind::Invalid, start, self.position)
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(b) = self.current_byte() {
            if b.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while let Some(b) = self.current_byte() {
            self.advance();
            if b == b'\n' {
                break;
            }
        }
        self.skip_whitespace();
    }

    fn lex_identifier_or_keyword(&mut self, start: usize) -> Token {
        self.advance();
        while let Some(b) = self.current_byte() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.source[start..self.position];
        if text == "_" {
            return self.make_token(TokenKind::Underscore, start, self.position);
        }

        let kind = keyword_kind(text).unwrap_or(TokenKind::Identifier);
        self.make_token(kind, start, self.position)
    }

    fn lex_number(&mut self, start: usize) -> Token {
        self.advance();
        while let Some(b) = self.current_byte() {
            if b.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }
        
        if self.current_byte() == Some(b'.') {
            if let Some(next) = self.peek_byte() {
                if next.is_ascii_digit() {
                    self.advance(); // consume '.'
                    while let Some(b) = self.current_byte() {
                        if b.is_ascii_digit() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return self.make_token(TokenKind::FloatLiteral, start, self.position);
                }
            }
            return self.make_token(TokenKind::IntegerLiteral, start, self.position);
        }

        self.make_token(TokenKind::IntegerLiteral, start, self.position)
    }

    fn lex_string(&mut self, start: usize) -> Token {
        self.advance();
        let mut closed = false;
        while let Some(b) = self.current_byte() {
            if b == b'"' {
                self.advance();
                closed = true;
                break;
            }
            if b == b'\\' {
                self.advance();
            }
            self.advance();
        }

        if !closed {
            return self.make_token(TokenKind::Invalid, start, self.position);
        }

        self.make_token(TokenKind::StringLiteral, start, self.position)
    }

    fn current_byte(&self) -> Option<u8> {
        if self.position < self.bytes.len() {
            Some(self.bytes[self.position])
        } else {
            None
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        if self.position + 1 < self.bytes.len() {
            Some(self.bytes[self.position + 1])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.position += 1;
    }

    fn make_token(&self, kind: TokenKind, start: usize, end: usize) -> Token {
        Token {
            kind,
            span: Span {
                start: start as u32,
                end: end as u32,
            },
        }
    }
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token();
        let is_eof = token.kind == TokenKind::EndOfFile;
        tokens.push(token);
        if is_eof {
            break;
        }
    }
    tokens
}
