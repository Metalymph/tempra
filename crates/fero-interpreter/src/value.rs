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
}

#[derive(Debug, Clone)]
pub struct StructInstance {
    pub name: String,
    pub fields: HashMap<String, Value>,
}
