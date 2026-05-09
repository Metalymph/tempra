use std::fmt;
use crate::opcode::BytecodeInstr;

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
    Float64,
    String,
}

impl fmt::Display for BytecodeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BytecodeType::Unit => write!(f, "Unit"),
            BytecodeType::Bool => write!(f, "Bool"),
            BytecodeType::Int64 => write!(f, "Int64"),
            BytecodeType::UInt64 => write!(f, "UInt64"),
            BytecodeType::Float64 => write!(f, "Float64"),
            BytecodeType::String => write!(f, "String"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BytecodeFunction {
    pub name: String,
    pub params: Vec<BytecodeType>,
    pub ret: BytecodeType,
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
            locals: Vec::new(),
            max_stack: 0,
            code: Vec::new(),
        }
    }
}
