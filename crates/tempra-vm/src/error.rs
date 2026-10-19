use std::fmt;
use tempra_bytecode::verify::BytecodeVerifyError;

#[derive(Debug, PartialEq)]
pub enum VmError {
    VerifyError(BytecodeVerifyError),
    UnknownFunction(String),
    WrongArity {
        expected: u16,
        found: u16,
    },
    StackUnderflow,
    LocalOutOfBounds(u32),
    LocalNotInitialized(u32),
    TypeMismatch {
        expected: String,
        found: String,
    },
    ExpectedEnum,
    WrongVariant {
        expected: String,
        found: String,
    },
    PayloadOutOfBounds(u16),
    Int64Arithmetic(tempra_runtime::Int64ArithmeticError),
    DivisionByZero,
    Native(String),
    UnreachableExecuted,
    InvalidInstructionPointer(usize),
    MissingReturn,
    Message(String),
    ExpectedArrayOrSlice,
    IndexOutOfBounds(usize),
    InvalidSliceRange {
        start: usize,
        end: usize,
        len: usize,
    },
    ExpectedIntIndex,
    InvalidConstant(u32),
    ExpectedArray,
}

impl fmt::Display for VmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmError::VerifyError(e) => write!(f, "Verification error: {}", e),
            VmError::UnknownFunction(s) => write!(f, "Unknown function: {}", s),
            VmError::WrongArity { expected, found } => {
                write!(f, "Wrong arity: expected {}, found {}", expected, found)
            }
            VmError::StackUnderflow => write!(f, "Stack underflow"),
            VmError::LocalOutOfBounds(i) => write!(f, "Local out of bounds: {}", i),
            VmError::LocalNotInitialized(i) => write!(f, "Local not initialized: {}", i),
            VmError::TypeMismatch { expected, found } => {
                write!(f, "Type mismatch: expected {}, found {}", expected, found)
            }
            VmError::ExpectedEnum => write!(f, "Expected enum value"),
            VmError::WrongVariant { expected, found } => {
                write!(f, "Wrong variant: expected {}, found {}", expected, found)
            }
            VmError::PayloadOutOfBounds(i) => write!(f, "Payload out of bounds: {}", i),
            VmError::Int64Arithmetic(error) => error.fmt(f),
            VmError::DivisionByZero => write!(f, "division by zero"),
            VmError::Native(msg) => write!(f, "native error: {}", msg),
            VmError::UnreachableExecuted => write!(f, "executed unreachable instruction"),
            VmError::InvalidInstructionPointer(ip) => {
                write!(f, "Invalid instruction pointer: {}", ip)
            }
            VmError::MissingReturn => write!(f, "Missing return instruction"),
            VmError::Message(s) => write!(f, "{}", s),
            VmError::ExpectedArrayOrSlice => write!(f, "Expected array or slice"),
            VmError::IndexOutOfBounds(i) => write!(f, "Index out of bounds: {}", i),
            VmError::InvalidSliceRange { start, end, len } => write!(
                f,
                "Invalid slice range: {}..{} for length {}",
                start, end, len
            ),
            VmError::ExpectedIntIndex => write!(f, "Expected integer index"),
            VmError::InvalidConstant(id) => write!(f, "Invalid constant ID: {}", id),
            VmError::ExpectedArray => write!(f, "Expected array"),
        }
    }
}

impl std::error::Error for VmError {}

impl From<BytecodeVerifyError> for VmError {
    fn from(e: BytecodeVerifyError) -> Self {
        VmError::VerifyError(e)
    }
}

impl From<tempra_runtime::Int64ArithmeticError> for VmError {
    fn from(error: tempra_runtime::Int64ArithmeticError) -> Self {
        Self::Int64Arithmetic(error)
    }
}
