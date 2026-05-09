pub mod bytecode;
pub mod constant;
pub mod disasm;
pub mod error;
pub mod function;
pub mod opcode;

pub use bytecode::BytecodeModule;
pub use constant::{BytecodeConstant, ConstId};
pub use error::BytecodeError;
pub use function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
pub use opcode::{BytecodeInstr, InstrOffset, StackEffect};

#[cfg(test)]
mod tests;
