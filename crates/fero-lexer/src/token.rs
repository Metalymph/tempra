/// A single lexical token produced by the scanner.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ── Literals & identifiers ──────────────────────────────────
    /// A user-defined name (variable, function, type).
    Ident(String),
    /// An integer literal.
    LitInt(i64),

    // ── Operators ───────────────────────────────────────────────
    /// `+`
    Plus,
    /// `-`
    Minus,

    // ── Keywords ────────────────────────────────────────────────
    /// `if`
    KwIf,
    /// `else`
    KwElse,

    // ── Special ─────────────────────────────────────────────────
    /// End of source input.
    Eof,
}
