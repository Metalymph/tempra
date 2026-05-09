use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tempra_hir::*;

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Int64(i64),
    UInt8(u8),
    Float64(f64),
    String(String),

    Array(Rc<RefCell<Vec<Value>>>),
    Slice(SliceValue),
    Tuple(Vec<Value>),

    Struct(StructInstance),

    Command(CommandValue),
    CommandResult(CommandResultValue),

    Result(Box<ValueResult>),
    Option(OptionValue),
    Error(String),
    Regex(RegexValue),
    Enum(Box<EnumValue>),
    Closure(Rc<ClosureValue>),
    Cell(Rc<RefCell<Value>>),
    Namespace(String),
    Type(Type),
    ArrayIter(ArrayIterValue),
    SliceIter(SliceIterValue),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumValue {
    pub enum_name: String,
    pub variant_name: String,
    pub payloads: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueResult {
    Ok(Value),
    Err(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Some(Box<Value>),
    None,
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
    Values(Rc<RefCell<Vec<Value>>>),
    Bytes(Rc<Vec<u8>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceValue {
    pub parent: SliceParent,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayIterValue {
    pub array: Rc<RefCell<Vec<Value>>>,
    pub index: Rc<RefCell<usize>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceIterValue {
    pub slice: SliceValue,
    pub index: Rc<RefCell<usize>>,
}

impl SliceValue {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn get(&self, index: usize) -> Option<Value> {
        if index < self.len() {
            match &self.parent {
                SliceParent::Values(v) => Some(v.borrow()[self.start + index].clone()),
                SliceParent::Bytes(b) => Some(Value::UInt8(b[self.start + index])),
            }
        } else {
            None
        }
    }

    pub fn to_vec(&self) -> Vec<Value> {
        let mut v = Vec::with_capacity(self.len());
        for i in 0..self.len() {
            v.push(self.get(i).unwrap());
        }
        v
    }
}

/// The structured Regex IR — a real value type, not just a text pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum RegexValue {
    Text(String),
    Any,
    Seq(Vec<RegexValue>),
    Alt(Vec<RegexValue>),
    Optional(Box<RegexValue>),
    ZeroOrMore(Box<RegexValue>),
    OneOrMore(Box<RegexValue>),
    Start,
    End,
    Digit,
    Space,
    Word,
}

impl RegexValue {
    pub fn is_match(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        for pos in 0..=chars.len() {
            if self.match_at(&chars, pos).is_some() {
                return true;
            }
        }
        false
    }

    pub fn replace_all(&self, text: &str, replacement: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::new();
        let mut scan_pos = 0usize;
        let mut emit_pos = 0usize;

        while scan_pos <= chars.len() {
            match self.match_at(&chars, scan_pos) {
                Some(end) if end > scan_pos => {
                    for ch in &chars[emit_pos..scan_pos] {
                        result.push(*ch);
                    }
                    result.push_str(replacement);
                    emit_pos = end;
                    scan_pos = end;
                }
                Some(_end) => {
                    // Zero-width match rule:
                    // emit one replacement at this position, then advance by at least one scalar.
                    for ch in &chars[emit_pos..scan_pos] {
                        result.push(*ch);
                    }
                    result.push_str(replacement);
                    if scan_pos < chars.len() {
                        result.push(chars[scan_pos]);
                        emit_pos = scan_pos + 1;
                        scan_pos += 1;
                    } else {
                        emit_pos = scan_pos;
                        scan_pos += 1;
                    }
                }
                None => {
                    scan_pos += 1;
                }
            }
        }

        for ch in &chars[emit_pos..] {
            result.push(*ch);
        }
        result
    }

    fn match_at(&self, chars: &[char], pos: usize) -> Option<usize> {
        Self::match_here(self, chars, pos)
    }

    fn match_here(re: &RegexValue, chars: &[char], pos: usize) -> Option<usize> {
        match re {
            RegexValue::Text(s) => {
                let tc: Vec<char> = s.chars().collect();
                if pos + tc.len() > chars.len() {
                    return None;
                }
                for (i, &c) in tc.iter().enumerate() {
                    if chars[pos + i] != c {
                        return None;
                    }
                }
                Some(pos + tc.len())
            }
            RegexValue::Any => {
                if pos < chars.len() {
                    Some(pos + 1)
                } else {
                    None
                }
            }
            RegexValue::Seq(parts) => {
                let mut cur = pos;
                for p in parts {
                    cur = Self::match_here(p, chars, cur)?;
                }
                Some(cur)
            }
            RegexValue::Alt(alts) => {
                for a in alts {
                    if let Some(end) = Self::match_here(a, chars, pos) {
                        return Some(end);
                    }
                }
                None
            }
            RegexValue::Optional(inner) => Some(Self::match_here(inner, chars, pos).unwrap_or(pos)),
            RegexValue::ZeroOrMore(inner) => {
                let mut cur = pos;
                loop {
                    match Self::match_here(inner, chars, cur) {
                        Some(next) if next > cur => cur = next,
                        _ => break,
                    }
                }
                Some(cur)
            }
            RegexValue::OneOrMore(inner) => {
                let first = Self::match_here(inner, chars, pos)?;
                let mut cur = first;
                loop {
                    match Self::match_here(inner, chars, cur) {
                        Some(next) if next > cur => cur = next,
                        _ => break,
                    }
                }
                Some(cur)
            }
            RegexValue::Start => {
                if pos == 0 {
                    Some(0)
                } else {
                    None
                }
            }
            RegexValue::End => {
                if pos == chars.len() {
                    Some(pos)
                } else {
                    None
                }
            }
            RegexValue::Digit => {
                if pos < chars.len() && chars[pos].is_ascii_digit() {
                    Some(pos + 1)
                } else {
                    None
                }
            }
            RegexValue::Space => {
                if pos < chars.len() && chars[pos].is_whitespace() {
                    Some(pos + 1)
                } else {
                    None
                }
            }
            RegexValue::Word => {
                if pos < chars.len() && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
                    Some(pos + 1)
                } else {
                    None
                }
            }
        }
    }
}

impl std::fmt::Display for RegexValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegexValue::Text(s) => write!(f, "Regex.text({:?})", s),
            RegexValue::Any => write!(f, "Regex.any()"),
            RegexValue::Seq(parts) => {
                write!(f, "Regex.seq(")?;
                for (i, p) in parts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", p)?;
                }
                write!(f, ")")
            }
            RegexValue::Alt(alts) => {
                write!(f, "Regex.alt(")?;
                for (i, a) in alts.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", a)?;
                }
                write!(f, ")")
            }
            RegexValue::Optional(inner) => write!(f, "Regex.optional({})", inner),
            RegexValue::ZeroOrMore(inner) => write!(f, "Regex.zero_or_more({})", inner),
            RegexValue::OneOrMore(inner) => write!(f, "Regex.one_or_more({})", inner),
            RegexValue::Start => write!(f, "Regex.start()"),
            RegexValue::End => write!(f, "Regex.end()"),
            RegexValue::Digit => write!(f, "Regex.digit()"),
            RegexValue::Space => write!(f, "Regex.space()"),
            RegexValue::Word => write!(f, "Regex.word()"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInstance {
    pub name: String,
    pub fields: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ClosureValue {
    pub params: Vec<HirParameter>,
    pub body: HirBlock,
    pub captures: HashMap<String, Value>,
    pub is_recursive: bool,
}

impl PartialEq for ClosureValue {
    fn eq(&self, _other: &Self) -> bool {
        false // Closures are not comparable
    }
}

impl Value {
    /// Returns a human-readable type name for use in runtime error messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "Unit",
            Value::Bool(_) => "Bool",
            Value::Int64(_) => "Int",
            Value::UInt8(_) => "UInt8",
            Value::Float64(_) => "Float",
            Value::String(_) => "String",
            Value::Array(_) => "Array",
            Value::Slice(_) => "Slice",
            Value::Tuple(_) => "Tuple",
            Value::Struct(_) => "Struct",
            Value::Command(_) => "Command",
            Value::CommandResult(_) => "CommandResult",
            Value::Result(_) => "Result",
            Value::Option(_) => "Option",
            Value::Error(_) => "Error",
            Value::Regex(_) => "Regex",
            Value::Enum(_) => "Enum",
            Value::Closure(_) => "Function",
            Value::Cell(c) => c.borrow().type_name(),
            Value::Namespace(_) => "Namespace",
            Value::Type(_) => "Type",
            Value::ArrayIter(_) => "ArrayIter",
            Value::SliceIter(_) => "SliceIter",
        }
    }

    pub fn to_repr(&self) -> String {
        match self {
            Value::String(s) => format!("\"{}\"", s),
            Value::Array(arr) => {
                let elems: Vec<String> = arr.borrow().iter().map(|v| v.to_repr()).collect();
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
                let mut fields: Vec<String> = s
                    .fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v.to_repr()))
                    .collect();
                fields.sort();
                format!("{} {{ {} }}", s.name, fields.join(", "))
            }
            Value::Enum(e) => {
                if e.payloads.is_empty() {
                    e.variant_name.clone()
                } else {
                    let elems: Vec<String> = e.payloads.iter().map(|v| v.to_repr()).collect();
                    format!("{}({})", e.variant_name, elems.join(", "))
                }
            }
            Value::Closure(_) => "fn(...)".to_string(),
            Value::Namespace(n) => n.clone(),
            Value::ArrayIter(_) => "ArrayIter(...)".to_string(),
            Value::SliceIter(_) => "SliceIter(...)".to_string(),
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
            Value::UInt8(i) => write!(f, "{}", i),
            Value::Float64(fl) => write!(f, "{}", fl),
            Value::String(s) => write!(f, "{}", s), // Direct string output for print
            Value::Array(arr) => {
                write!(f, "[")?;
                for (i, val) in arr.borrow().iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val.to_repr())?;
                }
                write!(f, "]")
            }
            Value::Slice(slice) => {
                write!(f, "[")?;
                for i in 0..slice.len() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", slice.get(i).unwrap().to_repr())?;
                }
                write!(f, "]")
            }
            Value::Tuple(tup) => {
                write!(f, "(")?;
                for (i, val) in tup.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", val.to_repr())?;
                }
                write!(f, ")")
            }
            Value::Struct(s) => {
                write!(f, "{} {{ ", s.name)?;
                let mut keys: Vec<&String> = s.fields.keys().collect();
                keys.sort();
                for (i, k) in keys.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
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
            Value::CommandResult(r) => write!(
                f,
                "CommandResult(exit_code={}, success={})",
                r.exit_code, r.success
            ),
            Value::Result(res) => match res.as_ref() {
                ValueResult::Ok(v) => write!(f, "Ok({})", v.to_repr()),
                ValueResult::Err(v) => write!(f, "Err({})", v.to_repr()),
            },
            Value::Error(msg) => write!(f, "Error(\"{}\")", msg),
            Value::Regex(re) => write!(f, "{}", re),
            Value::Option(opt) => match opt {
                OptionValue::Some(v) => write!(f, "Some({})", v.to_repr()),
                OptionValue::None => write!(f, "None"),
            },
            Value::Enum(e) => {
                if e.payloads.is_empty() {
                    write!(f, "{}", e.variant_name)
                } else {
                    write!(f, "{}(", e.variant_name)?;
                    for (i, p) in e.payloads.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", p.to_repr())?;
                    }
                    write!(f, ")")
                }
            }
            Value::Closure(_) => write!(f, "fn(...)"),
            Value::Cell(c) => write!(f, "Ref({})", c.borrow().to_repr()),
            Value::Namespace(n) => write!(f, "{}", n),
            Value::Type(t) => write!(f, "{}", t),
            Value::ArrayIter(_) => write!(f, "ArrayIter(...)"),
            Value::SliceIter(_) => write!(f, "SliceIter(...)"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Int64(a), Value::Int64(b)) => a == b,
            (Value::UInt8(a), Value::UInt8(b)) => a == b,
            (Value::Float64(a), Value::Float64(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => *a.borrow() == *b.borrow(),
            (Value::Slice(a), Value::Slice(b)) => a == b,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Struct(a), Value::Struct(b)) => a == b,
            (Value::Command(a), Value::Command(b)) => a == b,
            (Value::CommandResult(a), Value::CommandResult(b)) => a == b,
            (Value::Result(a), Value::Result(b)) => a == b,
            (Value::Option(a), Value::Option(b)) => a == b,
            (Value::Error(a), Value::Error(b)) => a == b,
            (Value::Regex(a), Value::Regex(b)) => a == b,
            (Value::Enum(a), Value::Enum(b)) => a == b,
            (Value::Closure(a), Value::Closure(b)) => Rc::ptr_eq(a, b),
            (Value::Cell(a), Value::Cell(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Namespace(a), Value::Namespace(b)) => a == b,
            (Value::Type(a), Value::Type(b)) => a == b,
            (Value::ArrayIter(a), Value::ArrayIter(b)) => a == b,
            (Value::SliceIter(a), Value::SliceIter(b)) => a == b,
            _ => false,
        }
    }
}
