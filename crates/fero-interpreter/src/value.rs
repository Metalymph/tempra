use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),

    Array(Vec<Value>),
    Tuple(Vec<Value>),

    Struct(StructInstance),

    Command(CommandValue),
    CommandResult(CommandResultValue),

    Result(Box<ValueResult>),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ValueResult {
    Ok(Value),
    Err(Value),
}

#[derive(Debug, Clone)]
pub struct CommandValue {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandResultValue {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct StructInstance {
    pub name: String,
    pub fields: HashMap<String, Value>,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int64(i) => write!(f, "{}", i),
            Value::Float64(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s),
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, "]")
            }
            Value::Tuple(tup) => {
                write!(f, "(")?;
                for (i, val) in tup.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val)?;
                }
                write!(f, ")")
            }
            Value::Struct(s) => {
                write!(f, "{} {{ ", s.name)?;
                let mut first = true;
                for (k, v) in &s.fields {
                    if !first { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, v)?;
                    first = false;
                }
                write!(f, " }}")
            }
            Value::Command(c) => write!(f, "Command(\"{}\", {:?})", c.program, c.args),
            Value::CommandResult(r) => write!(f, "CommandResult(exit_code={}, success={})", r.exit_code, r.success),
            Value::Result(res) => match res.as_ref() {
                ValueResult::Ok(v) => write!(f, "Ok({})", v),
                ValueResult::Err(v) => write!(f, "Err({})", v),
            },
            Value::Error(msg) => write!(f, "Error(\"{}\")", msg),
        }
    }
}
