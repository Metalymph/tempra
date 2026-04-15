use fero_span::Span;

/// The broad category of a lexical token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    // Special
    EndOfFile,
    Invalid,

    // Identifiers and literals
    Identifier,
    IntegerLiteral,
    FloatLiteral,
    StringLiteral,

    // Keywords
    Let,
    Var,
    Fn,
    If,
    Else,
    Match,
    For,
    In,
    Struct,
    Enum,
    Extend,
    Import,
    Return,
    Break,
    Continue,
    Inout,
    True,
    False,

    // Delimiters
    LeftParen,      // (
    RightParen,     // )
    LeftBrace,      // {
    RightBrace,     // }
    LeftBracket,    // [
    RightBracket,   // ]
    Comma,          // ,
    Dot,            // .
    Colon,          // :
    Semicolon,      // ;
    Underscore,     // _

    // Multi-char punctuation
    Arrow,          // ->
    FatArrow,       // =>
    DotDot,         // ..
    ShiftRight,     // >>

    // Operators
    Equal,          // =
    EqualEqual,     // ==
    Bang,           // !
    BangEqual,      // !=
    Plus,           // +
    Minus,          // -
    Star,           // *
    Slash,          // /
    Less,           // <
    Greater,        // >
    Pipe,           // |
    AndAnd,         // &&
    OrOr,           // ||
    QuestionMark,   // ?
}

/// A single lexical token produced by the lexer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// Returns the `TokenKind` for a given keyword string, or `None` if it's not a keyword.
pub fn keyword_kind(text: &str) -> Option<TokenKind> {
    match text {
        "let" => Some(TokenKind::Let),
        "var" => Some(TokenKind::Var),
        "fn" => Some(TokenKind::Fn),
        "if" => Some(TokenKind::If),
        "else" => Some(TokenKind::Else),
        "match" => Some(TokenKind::Match),
        "for" => Some(TokenKind::For),
        "in" => Some(TokenKind::In),
        "struct" => Some(TokenKind::Struct),
        "enum" => Some(TokenKind::Enum),
        "extend" => Some(TokenKind::Extend),
        "import" => Some(TokenKind::Import),
        "return" => Some(TokenKind::Return),
        "break" => Some(TokenKind::Break),
        "continue" => Some(TokenKind::Continue),
        "inout" => Some(TokenKind::Inout),
        "true" => Some(TokenKind::True),
        "false" => Some(TokenKind::False),
        _ => None,
    }
}
