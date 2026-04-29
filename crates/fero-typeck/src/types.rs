use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericParamId(pub u32);

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
    Slice(Box<Type>),
    Tuple(Vec<Type>),

    Struct(std::string::String),
    Enum(std::string::String),

    Command,
    CommandResult,
    Regex,
    Option(Box<Type>),

    Never,

    /// Used during partial checking when a type is not yet known.
    Unknown,
    /// Sentinel for expressions that already failed type checking.
    Error,

    BuiltinError,
    Result(Box<Type>, Box<Type>),
    /// Magic type for builtin namespaces like fs, os, env, Regex constructors.
    Namespace(std::string::String),
    /// A generic parameter placeholder (e.g. T)
    GenericParam { id: GenericParamId, name: std::string::String },
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
                | Type::UInt8
                | Type::UInt16
                | Type::UInt32
                | Type::UInt64
                | Type::Float32
                | Type::Float64
                | Type::Array(_)
                | Type::Slice(_)
                | Type::Tuple(_)
                | Type::CommandResult
                | Type::BuiltinError
                | Type::Result(_, _)
                | Type::Struct(_)
                | Type::Enum(_)
                | Type::Regex
                | Type::Option(_)
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
            Type::Slice(inner) => write!(f, "Slice[{}]", inner),
            Type::Tuple(elems) => {
                write!(f, "(")?;
                for (i, t) in elems.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            Type::Struct(name) => write!(f, "{}", name),
            Type::Enum(name) => write!(f, "{}", name),
            Type::Command => write!(f, "Command"),
            Type::CommandResult => write!(f, "CommandResult"),
            Type::Regex => write!(f, "Regex"),
            Type::Option(inner) => write!(f, "Option[{}]", inner),
            Type::Never => write!(f, "Never"),
            Type::Unknown => write!(f, "?"),
            Type::Error => write!(f, "<error>"),
            Type::BuiltinError => write!(f, "Error"),
            Type::Result(t, e) => write!(f, "Result[{}, {}]", t, e),
            Type::Namespace(n) => write!(f, "Namespace({})", n),
            Type::GenericParam { name, .. } => write!(f, "{}", name),
        }
    }
}

/// Resolve a named type string to a semantic Type.
pub fn resolve_named_type(name: &str) -> Type {
    match name {
        "Unit" => Type::Unit,
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
        "Regex" => Type::Regex,
        "Error" => Type::BuiltinError,
        "Never" => Type::Never,
        _ => Type::Unknown, // caller decides if this is struct/enum
    }
}

pub fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual {
        return true;
    }

    if *expected == Type::Unknown || *actual == Type::Unknown {
        return true;
    }

    // Never is the bottom type: it is compatible with any expected type.
    if *actual == Type::Never {
        return true;
    }

    let both_int = (*expected == Type::Int || *expected == Type::Int64)
        && (*actual == Type::Int || *actual == Type::Int64);
    if both_int {
        return true;
    }

    match (expected, actual) {
        (Type::Option(expected_inner), Type::Option(actual_inner)) => {
            if **actual_inner == Type::Unknown {
                true
            } else {
                types_compatible(expected_inner, actual_inner)
            }
        }
        (Type::Result(expected_ok, expected_err), Type::Result(actual_ok, actual_err)) => {
            let ok_ok = if **actual_ok == Type::Unknown {
                true
            } else {
                types_compatible(expected_ok, actual_ok)
            };
            let err_ok = if **actual_err == Type::Unknown {
                true
            } else {
                types_compatible(expected_err, actual_err)
            };
            ok_ok && err_ok
        }
        _ => false,
    }
}
