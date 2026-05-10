pub mod bytecode;
pub mod constant;
pub mod disasm;
pub mod error;
pub mod function;
pub mod opcode;
pub mod verify;

pub use bytecode::BytecodeModule;
pub use constant::{BytecodeConstant, ConstId};
pub use error::BytecodeError;
pub use function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
pub use opcode::{BytecodeInstr, InstrOffset, StackEffect};
pub use verify::{verify_module, verify_function, BytecodeVerifyError};

#[cfg(test)]
mod tests;
