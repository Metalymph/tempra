/// Runtime values for the TIR micro-step interpreter.
#[derive(Debug, Clone, PartialEq)]
pub enum TirRuntimeValue {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
}

impl std::fmt::Display for TirRuntimeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TirRuntimeValue::Unit => write!(f, "()"),
            TirRuntimeValue::Bool(b) => write!(f, "{}", b),
            TirRuntimeValue::Int64(i) => write!(f, "{}", i),
            TirRuntimeValue::UInt64(u) => write!(f, "{}", u),
            TirRuntimeValue::Float64(v) => write!(f, "{}", v),
            TirRuntimeValue::String(s) => write!(f, "{}", s),
        }
    }
}
