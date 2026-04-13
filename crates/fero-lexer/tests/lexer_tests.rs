use fero_lexer::{tokenize, TokenKind};

macro_rules! assert_tokens {
    ($source:expr, [ $($kind:expr),* $(,)? ]) => {
        let tokens = tokenize($source);
        let mut kinds: Vec<TokenKind> = tokens.into_iter().map(|t| t.kind).collect();
        // remove EOF for simpler matching
        if kinds.last() == Some(&TokenKind::EndOfFile) {
            kinds.pop();
        }
        let expected = vec![$($kind),*];
        assert_eq!(kinds, expected, "Tokens mismatch for source: {}", $source);
    };
}

#[test]
fn test_identifiers() {
    assert_tokens!("let x = 1", [
        TokenKind::Let,
        TokenKind::Identifier,
        TokenKind::Equal,
        TokenKind::IntegerLiteral,
    ]);
}

#[test]
fn test_keywords() {
    assert_tokens!("fn if else for match", [
        TokenKind::Fn,
        TokenKind::If,
        TokenKind::Else,
        TokenKind::For,
        TokenKind::Match,
    ]);
}

#[test]
fn test_numbers() {
    assert_tokens!("1 42 3.14 1..3", [
        TokenKind::IntegerLiteral,
        TokenKind::IntegerLiteral,
        TokenKind::FloatLiteral,
        TokenKind::IntegerLiteral,
        TokenKind::DotDot,
        TokenKind::IntegerLiteral,
    ]);
}

#[test]
fn test_strings() {
    assert_tokens!("\"hello\" \"a\\nb\"", [
        TokenKind::StringLiteral,
        TokenKind::StringLiteral,
    ]);
}

#[test]
fn test_operators() {
    assert_tokens!("= == ! != -> => .. >> && || | < >", [
        TokenKind::Equal,
        TokenKind::EqualEqual,
        TokenKind::Bang,
        TokenKind::BangEqual,
        TokenKind::Arrow,
        TokenKind::FatArrow,
        TokenKind::DotDot,
        TokenKind::ShiftRight,
        TokenKind::AndAnd,
        TokenKind::OrOr,
        TokenKind::Pipe,
        TokenKind::Less,
        TokenKind::Greater,
    ]);
}

#[test]
fn test_struct_sample() {
    let source = r#"
        struct User {
            name: String
            var bank_value: UInt64
        }
    "#;
    assert_tokens!(source, [
        TokenKind::Struct,
        TokenKind::Identifier,
        TokenKind::LeftBrace,
        TokenKind::Identifier,
        TokenKind::Colon,
        TokenKind::Identifier,
        TokenKind::Var,
        TokenKind::Identifier,
        TokenKind::Colon,
        TokenKind::Identifier,
        TokenKind::RightBrace,
    ]);
}
