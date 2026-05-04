use tempra_span::Span;

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
    Is,
    As,
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
    Pub,
    Type,
    Ref,
    Consume,
    Comptime,

    // Delimiters
    LeftParen,    // (
    RightParen,   // )
    LeftBrace,    // {
    RightBrace,   // }
    LeftBracket,  // [
    RightBracket, // ]
    Comma,        // ,
    Dot,          // .
    Colon,        // :
    Semicolon,    // ;
    Underscore,   // _

    // Multi-char punctuation
    Arrow,      // ->
    FatArrow,   // =>
    DotDot,     // ..
    ShiftRight, // >>

    // Operators
    Equal,        // =
    EqualEqual,   // ==
    Bang,         // !
    BangEqual,    // !=
    Plus,         // +
    Minus,        // -
    Star,         // *
    Slash,        // /
    Less,         // <
    Greater,      // >
    Pipe,         // |
    AndAnd,       // &&
    OrOr,         // ||
    QuestionMark, // ?
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
        "is" => Some(TokenKind::Is),
        "as" => Some(TokenKind::As),
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
        "pub" => Some(TokenKind::Pub),
        "type" => Some(TokenKind::Type),
        "ref" => Some(TokenKind::Ref),
        "consume" => Some(TokenKind::Consume),
        "comptime" => Some(TokenKind::Comptime),
        _ => None,
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TokenKind::EndOfFile => "end of file",
            TokenKind::Invalid => "invalid token",
            TokenKind::Identifier => "identifier",
            TokenKind::IntegerLiteral => "integer literal",
            TokenKind::FloatLiteral => "float literal",
            TokenKind::StringLiteral => "string literal",
            TokenKind::Let => "'let'",
            TokenKind::Var => "'var'",
            TokenKind::Fn => "'fn'",
            TokenKind::If => "'if'",
            TokenKind::Else => "'else'",
            TokenKind::Match => "'match'",
            TokenKind::For => "'for'",
            TokenKind::In => "'in'",
            TokenKind::Is => "'is'",
            TokenKind::As => "'as'",
            TokenKind::Struct => "'struct'",
            TokenKind::Enum => "'enum'",
            TokenKind::Extend => "'extend'",
            TokenKind::Import => "'import'",
            TokenKind::Return => "'return'",
            TokenKind::Break => "'break'",
            TokenKind::Continue => "'continue'",
            TokenKind::Inout => "'inout'",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::Pub => "'pub'",
            TokenKind::Type => "'type'",
            TokenKind::Ref => "'ref'",
            TokenKind::Consume => "'consume'",
            TokenKind::Comptime => "'comptime'",
            TokenKind::LeftParen => "'('",
            TokenKind::RightParen => "')'",
            TokenKind::LeftBrace => "'{'",
            TokenKind::RightBrace => "'}'",
            TokenKind::LeftBracket => "'['",
            TokenKind::RightBracket => "']'",
            TokenKind::Comma => "','",
            TokenKind::Dot => "'.'",
            TokenKind::Colon => "':'",
            TokenKind::Semicolon => "';'",
            TokenKind::Underscore => "'_'",
            TokenKind::Arrow => "'->'",
            TokenKind::FatArrow => "'=>'",
            TokenKind::DotDot => "'..'",
            TokenKind::ShiftRight => "'>>'",
            TokenKind::Equal => "'='",
            TokenKind::EqualEqual => "'=='",
            TokenKind::Bang => "'!'",
            TokenKind::BangEqual => "'!='",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Less => "'<'",
            TokenKind::Greater => "'>'",
            TokenKind::Pipe => "'|'",
            TokenKind::AndAnd => "'&&'",
            TokenKind::OrOr => "'||'",
            TokenKind::QuestionMark => "'?'",
        };
        write!(f, "{}", s)
    }
}
