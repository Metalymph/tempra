use fero_span::Span;

/// A desugared expression in the HIR.
///
/// Unlike the AST, HIR expressions have no syntactic sugar — every control
/// flow construct is represented in its canonical form.
#[derive(Debug)]
pub enum Expr {
    /// A resolved literal value.
    Literal { span: Span, raw: String },
    /// A binary operation.
    Binary {
        span: Span,
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// A conditional branch (always has an else in HIR; missing else is
    /// lowered to `Unit`).
    If {
        span: Span,
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
}
