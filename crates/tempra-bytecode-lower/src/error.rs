use thiserror::Error;

#[derive(Debug, Error)]
pub enum BytecodeLowerError {
    #[error("unsupported instruction: {0}")]
    UnsupportedInstr(String),

    #[error("unsupported type: {0}")]
    UnsupportedType(String),

    #[error("invalid block: {0}")]
    InvalidBlock(String),

    #[error("invalid control flow: {0}")]
    InvalidControlFlow(String),

    #[error("unknown function: {0}")]
    UnknownFunction(String),

    #[error("unknown value: {0}")]
    UnknownValue(String),

    #[error("unknown slot: {0}")]
    UnknownSlot(String),

    #[error("stack model error: {0}")]
    StackModelError(String),

    #[error("{0}")]
    Message(String),
}
