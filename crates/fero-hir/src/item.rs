use fero_span::Span;
use crate::expr::Expr;

/// A top-level HIR item after name resolution.
#[derive(Debug)]
pub enum Item {
    /// A function whose body has been desugared.
    Function {
        span: Span,
        name: String,
        body: Expr,
    },
}
