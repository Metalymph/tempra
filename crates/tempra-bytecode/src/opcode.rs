use crate::constant::ConstId;
use crate::function::{FunctionId, LocalId, BytecodeFunction, BytecodeType};
use crate::bytecode::BytecodeModule;
use crate::error::BytecodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrOffset(pub usize);

#[derive(Debug, Clone, PartialEq)]
pub enum BytecodeInstr {
    // Constants
    ConstUnit,
    ConstBool(bool),
    ConstI64(ConstId),
    ConstU64(ConstId),
    ConstF64(ConstId),
    ConstString(ConstId),

    // Locals
    LoadLocal(LocalId),
    StoreLocal(LocalId),

    // Stack
    Pop,
    Dup,

    // Arithmetic
    AddI64, SubI64, MulI64, DivI64,
    AddU64, SubU64, MulU64, DivU64,
    AddF64, SubF64, MulF64, DivF64,

    // Comparison
    Eq,
    LtI64, GtI64,
    LtU64, GtU64,
    LtF64, GtF64,

    // Bool
    Not, And, Or,

    // Control flow
    Jump(InstrOffset),
    JumpIfFalse(InstrOffset),
    Return,
    Unreachable,

    // Calls
    Call(FunctionId, u16), // ID, argc

    // Enum-like
    MakeEnum(ConstId, ConstId, u16), // enum_name, variant_name, payload_count
    Tag,
    Unwrap(ConstId, u16), // variant_name, payload_index

    // Array/Slice
    MakeArray(u16), // count
    GetIndex,
    Slice,
    Len,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackEffect {
    pub pops: u16,
    pub pushes: u16,
}

pub fn stack_effect(
    instr: &BytecodeInstr,
    module: &BytecodeModule,
    func: &BytecodeFunction,
) -> Result<StackEffect, BytecodeError> {
    let effect = match instr {
        BytecodeInstr::ConstUnit | BytecodeInstr::ConstBool(_) | BytecodeInstr::ConstI64(_) |
        BytecodeInstr::ConstU64(_) | BytecodeInstr::ConstF64(_) | BytecodeInstr::ConstString(_) => {
            StackEffect { pops: 0, pushes: 1 }
        }
        BytecodeInstr::LoadLocal(_) => StackEffect { pops: 0, pushes: 1 },
        BytecodeInstr::StoreLocal(_) => StackEffect { pops: 1, pushes: 0 },
        BytecodeInstr::Pop => StackEffect { pops: 1, pushes: 0 },
        BytecodeInstr::Dup => StackEffect { pops: 1, pushes: 2 },
        BytecodeInstr::AddI64 | BytecodeInstr::SubI64 | BytecodeInstr::MulI64 | BytecodeInstr::DivI64 |
        BytecodeInstr::AddU64 | BytecodeInstr::SubU64 | BytecodeInstr::MulU64 | BytecodeInstr::DivU64 |
        BytecodeInstr::AddF64 | BytecodeInstr::SubF64 | BytecodeInstr::MulF64 | BytecodeInstr::DivF64 => {
            StackEffect { pops: 2, pushes: 1 }
        }
        BytecodeInstr::Eq | BytecodeInstr::LtI64 | BytecodeInstr::GtI64 |
        BytecodeInstr::LtU64 | BytecodeInstr::GtU64 | BytecodeInstr::LtF64 | BytecodeInstr::GtF64 |
        BytecodeInstr::And | BytecodeInstr::Or => {
            StackEffect { pops: 2, pushes: 1 }
        }
        BytecodeInstr::Not => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::Jump(_) => StackEffect { pops: 0, pushes: 0 },
        BytecodeInstr::JumpIfFalse(_) => StackEffect { pops: 1, pushes: 0 },
        BytecodeInstr::Unreachable => StackEffect { pops: 0, pushes: 0 },
        BytecodeInstr::Return => {
            let pops = if func.ret == BytecodeType::Unit { 0 } else { 1 };
            StackEffect { pops, pushes: 0 }
        }
        BytecodeInstr::Call(func_id, argc) => {
            let target = module.functions.get(func_id.0 as usize)
                .ok_or_else(|| BytecodeError::InvalidFunctionId(*func_id))?;
            let pushes = if target.ret == BytecodeType::Unit { 0 } else { 1 };
            StackEffect { pops: *argc, pushes }
        }
        BytecodeInstr::MakeEnum(_, _, count) => StackEffect { pops: *count, pushes: 1 },
        BytecodeInstr::Tag => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::Unwrap(_, _) => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::MakeArray(count) => StackEffect { pops: *count, pushes: 1 },
        BytecodeInstr::GetIndex => StackEffect { pops: 2, pushes: 1 },
        BytecodeInstr::Slice => StackEffect { pops: 3, pushes: 1 },
        BytecodeInstr::Len => StackEffect { pops: 1, pushes: 1 },
    };
    Ok(effect)
}
