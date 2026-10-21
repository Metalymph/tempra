use crate::constant::{BytecodeConstant, ConstId};
use crate::function::{BytecodeFunction, FunctionId};
#[derive(Debug, Clone)]
pub struct BytecodeModule {
    pub version: u32,
    pub constants: Vec<BytecodeConstant>,
    pub functions: Vec<BytecodeFunction>,
}

impl Default for BytecodeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeModule {
    pub fn new() -> Self {
        Self {
            version: 1,
            constants: Vec::new(),
            functions: Vec::new(),
        }
    }

    pub fn add_constant(&mut self, constant: BytecodeConstant) -> ConstId {
        let id = self.constants.len() as u32;
        self.constants.push(constant);
        ConstId(id)
    }

    pub fn add_function(&mut self, function: BytecodeFunction) -> FunctionId {
        let id = self.functions.len() as u32;
        self.functions.push(function);
        FunctionId(id)
    }
}
