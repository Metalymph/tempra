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

/// Stable index into a module's structurally interned callable signatures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignatureId(pub u32);

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
    /// An indirect callable whose full signature is in `BytecodeModule::signatures`.
    Callable(SignatureId),
    /// Internal exclusive slice argument; valid only for direct inout parameters.
    ExclusiveSlice,
}

/// Function-valued signature excluding any captured/hidden parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BytecodeSignature {
    pub params: Vec<BytecodeType>,
    pub ret: BytecodeType,
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
            BytecodeType::Callable(id) => write!(f, "Callable(#{})", id.0),
            BytecodeType::ExclusiveSlice => write!(f, "ExclusiveSlice"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BytecodeFunction {
    pub name: String,
    pub params: Vec<BytecodeType>,
    /// Access modes for direct calls, indexed by parameter. An inout parameter
    /// receives a non-owning exclusive frame handle, never an ordinary copy.
    pub inout_params: Vec<bool>,
    pub ret: BytecodeType,
    pub is_extern: bool,
    pub locals: Vec<BytecodeType>,
    pub max_stack: u32,
    pub code: Vec<BytecodeInstr>,
}

impl BytecodeFunction {
    pub fn new(name: String, params: Vec<BytecodeType>, ret: BytecodeType) -> Self {
        let inout_params = vec![false; params.len()];
        Self {
            name,
            params,
            inout_params,
            ret,
            is_extern: false,
            locals: Vec::new(),
            max_stack: 0,
            code: Vec::new(),
        }
    }
}
