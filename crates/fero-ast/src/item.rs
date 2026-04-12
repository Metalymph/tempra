use fero_span::Span;
use crate::expr::Expr;

/// A top-level syntactic item.
#[derive(Debug)]
pub enum Item {
    /// A function definition: `fn name() { body }`.
    Function {
        span: Span,
        name: String,
        body: Expr,
    },
}
