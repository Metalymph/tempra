use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum VmValue {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt64(u64),
    UInt8(u8),
    Float64(f64),
    String(String),
    Enum(String, String, Vec<VmValue>), // enum_name, variant_name, payloads
    Tuple(Vec<VmValue>),
    Array(Rc<Vec<VmValue>>),
    Slice {
        parent: Rc<Vec<VmValue>>,
        start: usize,
        end: usize,
    },
    Struct {
        name: String,
        fields: Vec<(String, VmValue)>,
    },
}

impl VmValue {
    pub fn type_name(&self) -> &'static str {
        match self {
            VmValue::Unit => "Unit",
            VmValue::Bool(_) => "Bool",
            VmValue::Int64(_) => "Int64",
            VmValue::UInt64(_) => "UInt64",
            VmValue::UInt8(_) => "UInt8",
            VmValue::Float64(_) => "Float64",
            VmValue::String(_) => "String",
            VmValue::Enum(_, _, _) => "Enum",
            VmValue::Tuple(_) => "Tuple",
            VmValue::Array(_) => "Array",
            VmValue::Slice { .. } => "Slice",
            VmValue::Struct { name: _, .. } => "Struct", // We could return the actual name, but matching the signature is simpler for now
        }
    }
}

impl fmt::Display for VmValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmValue::Unit => write!(f, "()"),
            VmValue::Bool(b) => write!(f, "{}", b),
            VmValue::Int64(i) => write!(f, "{}", i),
            VmValue::UInt64(u) => write!(f, "{}", u),
            VmValue::UInt8(b) => write!(f, "{}", b),
            VmValue::Float64(v) => write!(f, "{}", v),
            VmValue::String(s) => write!(f, "{:?}", s),
            VmValue::Enum(_e, v, payloads) => {
                if payloads.is_empty() {
                    write!(f, "{}", v)
                } else {
                    write!(f, "{}(", v)?;
                    for (i, p) in payloads.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", p)?;
                    }
                    write!(f, ")")
                }
            }
            VmValue::Tuple(elements) => {
                write!(f, "(")?;
                for (i, element) in elements.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", element)?;
                }
                write!(f, ")")
            }
            VmValue::Array(arr) => {
                write!(f, "[")?;
                for (i, v) in arr.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            VmValue::Slice { parent, start, end } => {
                write!(f, "[")?;
                for i in *start..*end {
                    if i > *start {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", parent[i])?;
                }
                write!(f, "]")
            }
            VmValue::Struct { name, fields } => {
                write!(f, "{} {{ ", name)?;
                for (i, (fname, fval)) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", fname, fval)?;
                }
                write!(f, " }}")
            }
        }
    }
}
