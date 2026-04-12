use fero_span::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64, Span),
    Call {
        callee: String,
        args: Vec<Expr>,
        span: Span,
    },
}
