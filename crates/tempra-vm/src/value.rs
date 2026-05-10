use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    Float64(f64),
    String(String),
    Enum(String, String, Vec<VmValue>), // enum_name, variant_name, payloads
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Unit => write!(f, "()"),
            VmValue::Bool(b) => write!(f, "{}", b),
            VmValue::Int64(i) => write!(f, "{}", i),
            VmValue::UInt64(u) => write!(f, "{}", u),
            VmValue::Float64(v) => write!(f, "{}", v),
            VmValue::String(s) => write!(f, "{:?}", s),
            VmValue::Enum(_e, v, payloads) => {
                if payloads.is_empty() {
                    write!(f, "{}", v)
                } else {
                    write!(f, "{}(", v)?;
                    for (i, p) in payloads.iter().enumerate() {
                        if i > 0 { write!(f, ", ")?; }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ")")
                }
            }
        }
    }
}
