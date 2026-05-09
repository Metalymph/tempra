use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstId(pub u32);

impl fmt::Display for ConstId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeConstant {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
}

impl fmt::Display for BytecodeConstant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BytecodeConstant::Unit => write!(f, "()"),
            BytecodeConstant::Bool(b) => write!(f, "{}", b),
            BytecodeConstant::Int64(i) => write!(f, "{}", i),
            BytecodeConstant::UInt64(u) => write!(f, "{}", u),
            BytecodeConstant::Float64(v) => write!(f, "{}", v),
            BytecodeConstant::String(s) => write!(f, "{:?}", s),
        }
    }
}
