use crate::constant::{BytecodeConstant, ConstId};
use crate::function::{BytecodeFunction, BytecodeSignature, FunctionId, SignatureId};
#[derive(Debug, Clone)]
pub struct BytecodeModule {
    pub version: u32,
    pub constants: Vec<BytecodeConstant>,
    pub functions: Vec<BytecodeFunction>,
    pub signatures: Vec<BytecodeSignature>,
}

impl Default for BytecodeModule {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeModule {
    pub fn new() -> Self {
        Self {
            version: 4,
            constants: Vec::new(),
            functions: Vec::new(),
            signatures: Vec::new(),
        }
    }

    pub fn add_constant(&mut self, constant: BytecodeConstant) -> ConstId {
        let id = self.constants.len() as u32;
        self.constants.push(constant);
        ConstId(id)
    }

    /// Deduplicate callable signatures in deterministic first-use order.
    pub fn intern_signature(&mut self, signature: BytecodeSignature) -> SignatureId {
        if let Some(index) = self
            .signatures
            .iter()
            .position(|existing| existing == &signature)
        {
            return SignatureId(index as u32);
        }
        let id = SignatureId(self.signatures.len() as u32);
        self.signatures.push(signature);
        id
    }

    pub fn add_function(&mut self, function: BytecodeFunction) -> FunctionId {
        let id = self.functions.len() as u32;
        self.functions.push(function);
        FunctionId(id)
    }
}
