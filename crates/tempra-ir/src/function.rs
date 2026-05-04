use crate::instr::Instr;

/// A single function in the Tempra IR.
#[derive(Debug)]
pub struct Function {
    /// The fully-qualified name of the function.
    pub name: String,
    /// The instruction stream that forms this function's body.
    pub body: Vec<Instr>,
}
