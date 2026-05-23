use crate::opcode::BytecodeInstr;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

impl fmt::Display for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalId(pub u32);

impl fmt::Display for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytecodeType {
    Unit,
    Bool,
    Int64,
    UInt64,
    UInt8,
    Float64,
    String,
    /// Verifier-only escape hatch for dynamic types (Enums, Structs, Arrays, Slices).
    /// Used when the bytecode verifier doesn't have enough metadata to perform full static type checking
    /// for complex user-defined types.
    Any,
}

impl fmt::Display for BytecodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BytecodeType::Unit => write!(f, "Unit"),
            BytecodeType::Bool => write!(f, "Bool"),
            BytecodeType::Int64 => write!(f, "Int64"),
            BytecodeType::UInt64 => write!(f, "UInt64"),
            BytecodeType::UInt8 => write!(f, "UInt8"),
            BytecodeType::Float64 => write!(f, "Float64"),
            BytecodeType::String => write!(f, "String"),
            BytecodeType::Any => write!(f, "Any"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BytecodeFunction {
    pub name: String,
    pub params: Vec<BytecodeType>,
    pub ret: BytecodeType,
    pub is_extern: bool,
    pub locals: Vec<BytecodeType>,
    pub max_stack: u32,
    pub code: Vec<BytecodeInstr>,
}

impl BytecodeFunction {
    pub fn new(name: String, params: Vec<BytecodeType>, ret: BytecodeType) -> Self {
        Self {
            name,
            params,
            ret,
            is_extern: false,
            locals: Vec::new(),
            max_stack: 0,
            code: Vec::new(),
        }
    }
}
