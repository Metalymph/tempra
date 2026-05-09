use thiserror::Error;
use crate::constant::ConstId;
use crate::function::{FunctionId, LocalId};

#[derive(Debug, Error, PartialEq, Clone)]
pub enum BytecodeError {
    #[error("invalid function id: @{0}")]
    InvalidFunctionId(FunctionId),

    #[error("invalid constant id: #{0}")]
    InvalidConstId(ConstId),

    #[error("invalid local id: ${0}")]
    InvalidLocalId(LocalId),

    #[error("invalid jump target: {0}")]
    InvalidJumpTarget(usize),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Message(String),
}
