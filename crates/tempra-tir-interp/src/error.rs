use tempra_tir::ir::{TirBlockId, TirSlotId, TirValueId};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TirInterpError {
    #[error("unknown function: {0}")]
    UnknownFunction(String),

    #[error("wrong arity for function '{name}': expected {expected}, found {found}")]
    WrongArity {
        name: String,
        expected: usize,
        found: usize,
    },

    #[error("missing value for %{}", .0.0)]
    MissingValue(TirValueId),

    #[error("missing slot ${}", .0.0)]
    MissingSlot(TirSlotId),

    #[error("missing block ^block{}", .0.0)]
    MissingBlock(TirBlockId),

    #[error("type mismatch: {0}")]
    TypeMismatch(String),

    #[error("division by zero")]
    DivisionByZero,

    #[error(transparent)]
    Int64Arithmetic(#[from] tempra_runtime::Int64ArithmeticError),

    #[error("unsupported instruction: {0}")]
    UnsupportedInstr(String),

    #[error("unreachable instruction executed")]
    UnreachableExecuted,

    #[error("TIR verifier failed: {0}")]
    VerifierFailed(String),

    #[error("missing terminator in block ^block{}", .0.0)]
    MissingTerminator(TirBlockId),

    #[error("function has no entry block")]
    NoEntryBlock,
}
