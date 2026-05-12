use thiserror::Error;

#[derive(Error, Debug)]
pub enum JitCompileError {
    #[error("Unsupported type: {0:?}")]
    UnsupportedType(tempra_bytecode::function::BytecodeType),
    
    #[error("Unsupported instruction: {0:?}")]
    UnsupportedInstr(tempra_bytecode::opcode::BytecodeInstr),
    
    #[error("Unsupported function signature (only Int64 params/ret supported)")]
    UnsupportedSignature,
    
    #[error("Cranelift error: {0}")]
    CraneliftError(String),
}

#[derive(Error, Debug)]
pub enum JitRuntimeError {
    #[error("Execution error: {0}")]
    Message(String),
}
