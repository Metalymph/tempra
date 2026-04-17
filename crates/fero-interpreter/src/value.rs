use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Unit,
    Bool(bool),
    Int64(i64),
    Float64(f64),
    String(String),

    Array(Rc<Vec<Value>>),
    Slice(SliceValue),
    Tuple(Vec<Value>),

    Struct(StructInstance),

    Command(CommandValue),
    CommandResult(CommandResultValue),

    Result(Box<ValueResult>),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueResult {
    Ok(Value),
    Err(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandValue {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub check: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandResultValue {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SliceParent {
    Values(Rc<Vec<Value>>),
    Bytes(Rc<Vec<u8>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceValue {
    pub parent: SliceParent,
    pub start: usize,
    pub end: usize,
}

impl SliceValue {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn get(&self, index: usize) -> Option<Value> {
        if index < self.len() {
            match &self.parent {
                SliceParent::Values(v) => Some(v[self.start + index].clone()),
                SliceParent::Bytes(b) => Some(Value::Int64(b[self.start + index] as i64)),
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInstance {
    pub name: String,
    pub fields: HashMap<String, Value>,
}

impl Value {
    pub fn to_repr(&self) -> String {
        match self {
            Value::String(s) => format!("\"{}\"", s),
            Value::Array(arr) => {
                let elems: Vec<String> = arr.iter().map(|v| v.to_repr()).collect();
                format!("[{}]", elems.join(", "))
            }
            Value::Slice(slice) => {
                let mut elems = Vec::new();
                for i in 0..slice.len() {
                    elems.push(slice.get(i).unwrap().to_repr());
                }
                format!("[{}]", elems.join(", "))
            }
            Value::Tuple(tup) => {
                let elems: Vec<String> = tup.iter().map(|v| v.to_repr()).collect();
                format!("({})", elems.join(", "))
            }
            Value::Struct(s) => {
                let mut fields: Vec<String> = s.fields.iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_repr()))
                    .collect();
                fields.sort();
                format!("{} {{ {} }}", s.name, fields.join(", "))
            }
            _ => self.to_string(),
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Int64(i) => write!(f, "{}", i),
            Value::Float64(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s), // Direct string output for print
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val.to_repr())?;
                }
                write!(f, "]")
            }
            Value::Slice(slice) => {
                write!(f, "[")?;
                for i in 0..slice.len() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", slice.get(i).unwrap().to_repr())?;
                }
                write!(f, "]")
            }
            Value::Tuple(tup) => {
                write!(f, "(")?;
                for (i, val) in tup.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", val.to_repr())?;
                }
                write!(f, ")")
            }
            Value::Struct(s) => {
                write!(f, "{} {{ ", s.name)?;
                let mut keys: Vec<&String> = s.fields.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}: {}", k, s.fields.get(*k).unwrap().to_repr())?;
                }
                write!(f, " }}")
            }
            Value::Command(c) => {
                write!(f, "Command(\"{}\", args={:?}", c.program, c.args)?;
                if let Some(cwd) = &c.cwd {
                    write!(f, ", cwd=\"{}\"", cwd)?;
                }
                if c.check {
                    write!(f, ", check=true")?;
                }
                write!(f, ")")
            }
            Value::CommandResult(r) => write!(f, "CommandResult(exit_code={}, success={})", r.exit_code, r.success),
            Value::Result(res) => match res.as_ref() {
                ValueResult::Ok(v) => write!(f, "Ok({})", v.to_repr()),
                ValueResult::Err(v) => write!(f, "Err({})", v.to_repr()),
            },
            Value::Error(msg) => write!(f, "Error(\"{}\")", msg),
        }
    }
}
