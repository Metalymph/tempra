use std::fmt;

/// Semantic type representation for Fero v0.
///
/// This is intentionally small. It covers only the types needed for the
/// first type-checking pass. More types (generics, interfaces, effects)
/// will be added later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Bool,
    String,

    Int,
    Int8,
    Int16,
    Int32,
    Int64,

    UInt8,
    UInt16,
    UInt32,
    UInt64,

    Float32,
    Float64,

    Array(Box<Type>),
    Tuple(Vec<Type>),

    Struct(std::string::String),
    Enum(std::string::String),

    Command,
    CommandResult,

    /// Used during partial checking when a type is not yet known.
    Unknown,
    /// Sentinel for expressions that already failed type checking.
    Error,

    BuiltinError,
    Result(Box<Type>, Box<Type>),
}

impl Type {
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::Int
                | Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::UInt8
                | Type::UInt16
                | Type::UInt32
                | Type::UInt64
                | Type::Float32
                | Type::Float64
        )
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::Int
                | Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::UInt8
                | Type::UInt16
                | Type::UInt32
                | Type::UInt64
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::Float32 | Type::Float64)
    }

    pub fn is_printable(&self) -> bool {
        matches!(
            self,
            Type::Unit
                | Type::String
                | Type::Bool
                | Type::Int
                | Type::Int8
                | Type::Int16
                | Type::Int32
                | Type::Int64
                | Type::Float32
                | Type::Float64
                | Type::Array(_)
                | Type::Tuple(_)
                | Type::CommandResult
                | Type::BuiltinError
                | Type::Result(_, _)
                | Type::Struct(_)
                | Type::Enum(_)
        )
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Unit => write!(f, "Unit"),
            Type::Bool => write!(f, "Bool"),
            Type::String => write!(f, "String"),
            Type::Int => write!(f, "Int"),
            Type::Int8 => write!(f, "Int8"),
            Type::Int16 => write!(f, "Int16"),
            Type::Int32 => write!(f, "Int32"),
            Type::Int64 => write!(f, "Int64"),
            Type::UInt8 => write!(f, "UInt8"),
            Type::UInt16 => write!(f, "UInt16"),
            Type::UInt32 => write!(f, "UInt32"),
            Type::UInt64 => write!(f, "UInt64"),
            Type::Float32 => write!(f, "Float32"),
            Type::Float64 => write!(f, "Float64"),
            Type::Array(inner) => write!(f, "[{}]", inner),
            Type::Tuple(elems) => {
                write!(f, "(")?;
                for (i, t) in elems.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            Type::Struct(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
            Type::Command => write!(f, "Command"),
            Type::CommandResult => write!(f, "CommandResult"),
            Type::Unknown => write!(f, "?"),
            Type::Error => write!(f, "<error>"),
            Type::BuiltinError => write!(f, "Error"),
            Type::Result(t, e) => write!(f, "Result[{}, {}]", t, e),
        }
    }
}

/// Resolve a named type string to a semantic Type.
pub fn resolve_named_type(name: &str) -> Type {
    match name {
        "Bool" => Type::Bool,
        "String" => Type::String,
        "Int" => Type::Int,
        "Int8" => Type::Int8,
        "Int16" => Type::Int16,
        "Int32" => Type::Int32,
        "Int64" => Type::Int64,
        "UInt8" => Type::UInt8,
        "UInt16" => Type::UInt16,
        "UInt32" => Type::UInt32,
        "UInt64" => Type::UInt64,
        "Float32" => Type::Float32,
        "Float64" => Type::Float64,
        "Command" => Type::Command,
        "CommandResult" => Type::CommandResult,
        "Error" => Type::BuiltinError,
        _ => Type::Unknown, // caller decides if this is struct/enum
    }
}
