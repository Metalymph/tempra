use crate::value::Value;

#[derive(Debug)]
pub enum ControlFlow {
    Value(Value),
    Return(Value),
    Break,
    Continue,
}
