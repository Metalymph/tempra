use fero_span::Span;

/// A syntactic expression node.
///
/// Every variant carries a [`Span`] for error reporting and source mapping.
#[derive(Debug)]
pub enum Expr {
    /// A literal value as written in source (e.g. `42`, `"hello"`).
    Literal {
        span: Span,
        /// The raw text of the literal — parsing into a typed value happens later.
        raw: String,
    },
    /// A binary operation (e.g. `a + b`).
    Binary {
        span: Span,
        /// Operator symbol (e.g. `+`, `-`, `==`).
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// An `if` expression with an optional `else` branch.
    If {
        span: Span,
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
}
