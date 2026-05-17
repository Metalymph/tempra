use tempra_span::Span;
use tempra_tir::error::TirBuildError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TirLowerError {
    #[error("unsupported expression for lowering: {0}")]
    UnsupportedExpr(String, Span),

    #[error("unsupported statement for lowering: {0}")]
    UnsupportedStmt(String, Span),

    #[error("unsupported pattern for narrowing: {0}")]
    UnsupportedPattern(String, Span),

    #[error("unknown local variable: {0}")]
    UnknownLocal(String, Span),

    #[error("unknown function: {0}")]
    UnknownFunction(String, Span),

    #[error("type mismatch during lowering: expected {expected}, found {found}")]
    TypeMismatch {
        expected: String,
        found: String,
        span: Span,
    },

    #[error("missing return in block or unsupported implicit return")]
    MissingReturn(Span),

    #[error("generic or unsupported type cannot be mapped to TIR: {0}")]
    UnsupportedType(String),

    #[error("TIR build error: {0}")]
    BuildError(#[from] TirBuildError),
}
