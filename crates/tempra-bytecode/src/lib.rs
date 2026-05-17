pub mod bytecode;
pub mod constant;
pub mod disasm;
pub mod error;
pub mod function;
pub mod opcode;
pub mod serialize;
pub mod verify;

pub use serialize::{
    decode_and_verify_artifact, decode_artifact, encode_artifact, BytecodeCacheArtifact,
    BytecodeCacheHeader, BytecodeSerializeError,
};

pub use bytecode::BytecodeModule;
pub use constant::{BytecodeConstant, ConstId};
pub use error::BytecodeError;
pub use function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
pub use opcode::{BytecodeInstr, InstrOffset, StackEffect};
pub use verify::{verify_function, verify_module, BytecodeVerifyError};

#[cfg(test)]
mod tests;
