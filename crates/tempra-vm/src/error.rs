use std::fmt;
use tempra_bytecode::verify::BytecodeVerifyError;

#[derive(Debug, PartialEq)]
pub enum VmError {
    VerifyError(BytecodeVerifyError),
    UnknownFunction(String),
    WrongArity { expected: u16, found: u16 },
    StackUnderflow,
    LocalOutOfBounds(u32),
    LocalNotInitialized(u32),
    TypeMismatch { expected: String, found: String },
    DivisionByZero,
    UnreachableExecuted,
    InvalidInstructionPointer(usize),
    MissingReturn,
    Message(String),
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::VerifyError(e) => write!(f, "Verification error: {}", e),
            VmError::UnknownFunction(s) => write!(f, "Unknown function: {}", s),
            VmError::WrongArity { expected, found } => write!(f, "Wrong arity: expected {}, found {}", expected, found),
            VmError::StackUnderflow => write!(f, "Stack underflow"),
            VmError::LocalOutOfBounds(i) => write!(f, "Local out of bounds: {}", i),
            VmError::LocalNotInitialized(i) => write!(f, "Local not initialized: {}", i),
            VmError::TypeMismatch { expected, found } => write!(f, "Type mismatch: expected {}, found {}", expected, found),
            VmError::DivisionByZero => write!(f, "Division by zero"),
            VmError::UnreachableExecuted => write!(f, "Unreachable code executed"),
            VmError::InvalidInstructionPointer(ip) => write!(f, "Invalid instruction pointer: {}", ip),
            VmError::MissingReturn => write!(f, "Missing return instruction"),
            VmError::Message(s) => write!(f, "{}", s),
        }
    }
}

impl std::error::Error for VmError {}

impl From<BytecodeVerifyError> for VmError {
    fn from(e: BytecodeVerifyError) -> Self {
        VmError::VerifyError(e)
    }
}
