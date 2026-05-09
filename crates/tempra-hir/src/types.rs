use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GenericParamId(pub u32);

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum ComptimeValue {
    Unit,
    Bool(bool),
    Int(i64),
    String(String),
    Type(Type),
}

impl fmt::Display for ComptimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComptimeValue::Unit => write!(f, "()"),
            ComptimeValue::Bool(b) => write!(f, "{}", b),
            ComptimeValue::Int(i) => write!(f, "{}", i),
            ComptimeValue::String(s) => write!(f, "\"{}\"", s),
            ComptimeValue::Type(t) => write!(f, "{}", t),
        }
    }
}

/// Semantic type representation for Tempra v0.
///
/// This is intentionally small. It covers only the types needed for the
/// first type-checking pass. More types (generics, interfaces, effects)
/// will be added later.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    Struct(std::string::String, Vec<Type>),
    Enum(std::string::String, Vec<Type>),

    Command,
    CommandResult,
    Regex,
    Option(Box<Type>),

    Never,
    SelfType,
    Associated(std::string::String, std::string::String), // InterfaceName, TypeName

    /// Used during partial checking when a type is not yet known.
    Unknown,
    /// Sentinel for expressions that already failed type checking.
    Error,

    BuiltinError,
    Result(Box<Type>, Box<Type>),
    /// Magic type for builtin namespaces like fs, os, env, Regex constructors.
    Namespace(std::string::String),
    Function(Vec<Type>, Box<Type>),
    /// A generic parameter placeholder (e.g. T)
    GenericParam {
        id: GenericParamId,
        name: std::string::String,
        constraint: Option<std::string::String>,
    },
    /// The type of a type value (metatype)
    Meta(Box<Type>),
    /// A projection from a generic parameter (e.g. T.Output)
    Projection {
        base: GenericParamId,
        assoc: String,
    },
    Interface(String),
    ArrayIter(Box<Type>),
    SliceIter(Box<Type>),
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

    pub fn contains_generic_param(&self) -> bool {
        match self {
            Type::Array(inner) | Type::Slice(inner) | Type::Option(inner) => {
                inner.contains_generic_param()
            }
            Type::Tuple(elems) => elems.iter().any(|e| e.contains_generic_param()),
            Type::Struct(_, args) | Type::Enum(_, args) => {
                args.iter().any(|a| a.contains_generic_param())
            }
            Type::Result(ok, err) => ok.contains_generic_param() || err.contains_generic_param(),
            Type::Function(params, ret) => {
                params.iter().any(|p| p.contains_generic_param()) || ret.contains_generic_param()
            }
            Type::GenericParam { .. } | Type::Projection { .. } => true,
            _ => false,
        }
    }

    pub fn get_generic_params(&self) -> Vec<GenericParamId> {
        let mut params = Vec::new();
        match self {
            Type::Array(inner) | Type::Slice(inner) | Type::Option(inner) => {
                params.extend(inner.get_generic_params())
            }
            Type::Tuple(elems) => {
                for e in elems {
                    params.extend(e.get_generic_params());
                }
            }
            Type::Struct(_, args) | Type::Enum(_, args) => {
                for a in args {
                    params.extend(a.get_generic_params());
                }
            }
            Type::Result(ok, err) => {
                params.extend(ok.get_generic_params());
                params.extend(err.get_generic_params());
            }
            Type::Function(p_types, ret) => {
                for p in p_types {
                    params.extend(p.get_generic_params());
                }
                params.extend(ret.get_generic_params());
            }
            Type::GenericParam { id, .. } => params.push(*id),
            Type::Projection { base, .. } => params.push(*base),
            _ => {}
        }
        params
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
                | Type::Struct(_, _)
                | Type::Enum(_, _)
                | Type::Regex
                | Type::Option(_)
                | Type::Function(_, _)
                | Type::SelfType
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
            Type::Array(inner) => write!(f, "Array({})", inner),
            Type::Slice(inner) => write!(f, "Slice({})", inner),
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
            Type::Struct(name, args) => {
                if args.is_empty() {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}(", name)?;
                    for (i, t) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", t)?;
                    }
                    write!(f, ")")
                }
            }
            Type::Enum(name, args) => {
                if args.is_empty() {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}(", name)?;
                    for (i, t) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", t)?;
                    }
                    write!(f, ")")
                }
            }
            Type::Meta(ty) => write!(f, "Type<{}>", ty),
            Type::Command => write!(f, "Command"),
            Type::CommandResult => write!(f, "CommandResult"),
            Type::Regex => write!(f, "Regex"),
            Type::Option(inner) => write!(f, "Option({})", inner),
            Type::Never => write!(f, "Never"),
            Type::Unknown => write!(f, "?"),
            Type::Error => write!(f, "<error>"),
            Type::BuiltinError => write!(f, "Error"),
            Type::Result(t, e) => write!(f, "Result({}, {})", t, e),
            Type::Namespace(n) => write!(f, "Namespace({})", n),
            Type::Function(params, ret) => {
                write!(f, "fn(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ") -> {}", ret)
            }
            Type::GenericParam { name, .. } => write!(f, "{}", name),
            Type::SelfType => write!(f, "Self"),
            Type::Associated(_, name) => write!(f, "{}", name),
            Type::Projection { base, assoc } => write!(f, "GP({:?}).{}", base, assoc),
            Type::Interface(name) => write!(f, "{}", name),
            Type::ArrayIter(inner) => write!(f, "ArrayIter({})", inner),
            Type::SliceIter(inner) => write!(f, "SliceIter({})", inner),
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
        "Option" => Type::Option(Box::new(Type::Unknown)),
        "Result" => Type::Result(Box::new(Type::Unknown), Box::new(Type::Unknown)),
        "Array" => Type::Array(Box::new(Type::Unknown)),
        "Slice" => Type::Slice(Box::new(Type::Unknown)),
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
        (Type::Meta(expected_inner), Type::Meta(actual_inner)) => {
            types_compatible(expected_inner, actual_inner)
        }
        (Type::Associated(itf1, n1), Type::Associated(itf2, n2)) => itf1 == itf2 && n1 == n2,
        (Type::Projection { base: b1, assoc: n1 }, Type::Projection { base: b2, assoc: n2 }) => {
            b1 == b2 && n1 == n2
        }
        _ => false,
    }
}

impl Type {
    pub fn contains_self(&self) -> bool {
        match self {
            Type::SelfType => true,
            Type::Array(inner) | Type::Slice(inner) | Type::Option(inner) => inner.contains_self(),
            Type::Tuple(elems) => elems.iter().any(|e| e.contains_self()),
            Type::Struct(_, args) | Type::Enum(_, args) => args.iter().any(|a| a.contains_self()),
            Type::Result(ok, err) => ok.contains_self() || err.contains_self(),
            Type::Function(params, ret) => {
                params.iter().any(|p| p.contains_self()) || ret.contains_self()
            }
            _ => false,
        }
    }

    pub fn get_name(&self) -> String {
        match self {
            Type::Struct(name, _) => name.clone(),
            Type::Enum(name, _) => name.clone(),
            Type::Interface(name) => name.clone(),
            Type::ArrayIter(_) => "ArrayIter".to_string(),
            Type::SliceIter(_) => "SliceIter".to_string(),
            _ => "Unknown".to_string(),
        }
    }
}

pub fn substitute_self(ty: &Type, self_ty: &Type) -> Type {
    match ty {
        Type::SelfType => self_ty.clone(),
        Type::Array(inner) => Type::Array(Box::new(substitute_self(inner, self_ty))),
        Type::Slice(inner) => Type::Slice(Box::new(substitute_self(inner, self_ty))),
        Type::ArrayIter(inner) => Type::ArrayIter(Box::new(substitute_self(inner, self_ty))),
        Type::SliceIter(inner) => Type::SliceIter(Box::new(substitute_self(inner, self_ty))),
        Type::Option(inner) => Type::Option(Box::new(substitute_self(inner, self_ty))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_self(ok, self_ty)),
            Box::new(substitute_self(err, self_ty)),
        ),
        Type::Tuple(elems) => {
            Type::Tuple(elems.iter().map(|e| substitute_self(e, self_ty)).collect())
        }
        Type::Struct(name, args) => Type::Struct(
            name.clone(),
            args.iter().map(|a| substitute_self(a, self_ty)).collect(),
        ),
        Type::Enum(name, args) => Type::Enum(
            name.clone(),
            args.iter().map(|a| substitute_self(a, self_ty)).collect(),
        ),
        Type::Function(params, ret) => Type::Function(
            params.iter().map(|p| substitute_self(p, self_ty)).collect(),
            Box::new(substitute_self(ret, self_ty)),
        ),
        Type::Meta(inner) => Type::Meta(Box::new(substitute_self(inner, self_ty))),
        _ => ty.clone(),
    }
}
