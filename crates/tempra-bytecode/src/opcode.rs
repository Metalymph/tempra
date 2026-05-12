use crate::constant::ConstId;
use crate::function::{FunctionId, LocalId, BytecodeFunction, BytecodeType};
use crate::bytecode::BytecodeModule;
use crate::error::BytecodeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstrOffset(pub usize);

#[derive(Debug, Clone, Copy, PartialEq)]
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
    Size,
    Bytes,
    ArrayPush,

    // Structs
    MakeStruct(ConstId, ConstId, u16), // name_id, field_list_id, field_count
    GetField(ConstId),               // field_name_id
    SetField(ConstId),               // field_name_id

    // Superinstructions
    AddI64LL(LocalId, LocalId),       // LoadLocal a; LoadLocal b; AddI64
    AddI64LC(LocalId, ConstId),       // LoadLocal a; ConstI64 b; AddI64
}

pub const BYTECODE_OPCODE_COUNT: usize = 52;

impl BytecodeInstr {
    pub fn opcode_id(&self) -> usize {
        match self {
            BytecodeInstr::ConstUnit => 0,
            BytecodeInstr::ConstBool(_) => 1,
            BytecodeInstr::ConstI64(_) => 2,
            BytecodeInstr::ConstU64(_) => 3,
            BytecodeInstr::ConstF64(_) => 4,
            BytecodeInstr::ConstString(_) => 5,
            BytecodeInstr::LoadLocal(_) => 6,
            BytecodeInstr::StoreLocal(_) => 7,
            BytecodeInstr::Pop => 8,
            BytecodeInstr::Dup => 9,
            BytecodeInstr::AddI64 => 10,
            BytecodeInstr::SubI64 => 11,
            BytecodeInstr::MulI64 => 12,
            BytecodeInstr::DivI64 => 13,
            BytecodeInstr::AddU64 => 14,
            BytecodeInstr::SubU64 => 15,
            BytecodeInstr::MulU64 => 16,
            BytecodeInstr::DivU64 => 17,
            BytecodeInstr::AddF64 => 18,
            BytecodeInstr::SubF64 => 19,
            BytecodeInstr::MulF64 => 20,
            BytecodeInstr::DivF64 => 21,
            BytecodeInstr::Eq => 22,
            BytecodeInstr::LtI64 => 23,
            BytecodeInstr::GtI64 => 24,
            BytecodeInstr::LtU64 => 25,
            BytecodeInstr::GtU64 => 26,
            BytecodeInstr::LtF64 => 27,
            BytecodeInstr::GtF64 => 28,
            BytecodeInstr::Not => 29,
            BytecodeInstr::And => 30,
            BytecodeInstr::Or => 31,
            BytecodeInstr::Jump(_) => 32,
            BytecodeInstr::JumpIfFalse(_) => 33,
            BytecodeInstr::Return => 34,
            BytecodeInstr::Unreachable => 35,
            BytecodeInstr::Call(_, _) => 36,
            BytecodeInstr::MakeEnum(_, _, _) => 37,
            BytecodeInstr::Tag => 38,
            BytecodeInstr::Unwrap(_, _) => 39,
            BytecodeInstr::MakeArray(_) => 40,
            BytecodeInstr::GetIndex => 41,
            BytecodeInstr::Slice => 42,
            BytecodeInstr::Len => 43,
            BytecodeInstr::Size => 44,
            BytecodeInstr::Bytes => 45,
            BytecodeInstr::MakeStruct(_, _, _) => 46,
            BytecodeInstr::GetField(_) => 47,
            BytecodeInstr::SetField(_) => 48,
            BytecodeInstr::ArrayPush => 49,
            BytecodeInstr::AddI64LL(_, _) => 50,
            BytecodeInstr::AddI64LC(_, _) => 51,
        }
    }

    pub fn opcode_name(&self) -> &'static str {
        match self {
            BytecodeInstr::ConstUnit => "ConstUnit",
            BytecodeInstr::ConstBool(_) => "ConstBool",
            BytecodeInstr::ConstI64(_) => "ConstI64",
            BytecodeInstr::ConstU64(_) => "ConstU64",
            BytecodeInstr::ConstF64(_) => "ConstF64",
            BytecodeInstr::ConstString(_) => "ConstString",
            BytecodeInstr::LoadLocal(_) => "LoadLocal",
            BytecodeInstr::StoreLocal(_) => "StoreLocal",
            BytecodeInstr::Pop => "Pop",
            BytecodeInstr::Dup => "Dup",
            BytecodeInstr::AddI64 => "AddI64",
            BytecodeInstr::SubI64 => "SubI64",
            BytecodeInstr::MulI64 => "MulI64",
            BytecodeInstr::DivI64 => "DivI64",
            BytecodeInstr::AddU64 => "AddU64",
            BytecodeInstr::SubU64 => "SubU64",
            BytecodeInstr::MulU64 => "MulU64",
            BytecodeInstr::DivU64 => "DivU64",
            BytecodeInstr::AddF64 => "AddF64",
            BytecodeInstr::SubF64 => "SubF64",
            BytecodeInstr::MulF64 => "MulF64",
            BytecodeInstr::DivF64 => "DivF64",
            BytecodeInstr::Eq => "Eq",
            BytecodeInstr::LtI64 => "LtI64",
            BytecodeInstr::GtI64 => "GtI64",
            BytecodeInstr::LtU64 => "LtU64",
            BytecodeInstr::GtU64 => "GtU64",
            BytecodeInstr::LtF64 => "LtF64",
            BytecodeInstr::GtF64 => "GtF64",
            BytecodeInstr::Not => "Not",
            BytecodeInstr::And => "And",
            BytecodeInstr::Or => "Or",
            BytecodeInstr::Jump(_) => "Jump",
            BytecodeInstr::JumpIfFalse(_) => "JumpIfFalse",
            BytecodeInstr::Return => "Return",
            BytecodeInstr::Unreachable => "Unreachable",
            BytecodeInstr::Call(_, _) => "Call",
            BytecodeInstr::MakeEnum(_, _, _) => "MakeEnum",
            BytecodeInstr::Tag => "Tag",
            BytecodeInstr::Unwrap(_, _) => "Unwrap",
            BytecodeInstr::MakeArray(_) => "MakeArray",
            BytecodeInstr::GetIndex => "GetIndex",
            BytecodeInstr::Slice => "Slice",
            BytecodeInstr::Len => "Len",
            BytecodeInstr::Size => "Size",
            BytecodeInstr::Bytes => "Bytes",
            BytecodeInstr::MakeStruct(_, _, _) => "MakeStruct",
            BytecodeInstr::GetField(_) => "GetField",
            BytecodeInstr::SetField(_) => "SetField",
            BytecodeInstr::ArrayPush => "ArrayPush",
            BytecodeInstr::AddI64LL(_, _) => "AddI64LL",
            BytecodeInstr::AddI64LC(_, _) => "AddI64LC",
        }
    }
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
        BytecodeInstr::Call(_id, argc) => StackEffect { pops: *argc, pushes: if module.functions[_id.0 as usize].ret == BytecodeType::Unit { 0 } else { 1 } },
        BytecodeInstr::MakeEnum(_name, _variant, argc) => StackEffect { pops: *argc, pushes: 1 },
        BytecodeInstr::Tag => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::Unwrap(_variant, _idx) => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::MakeArray(count) => StackEffect { pops: *count, pushes: 1 },
        BytecodeInstr::GetIndex => StackEffect { pops: 2, pushes: 1 },
        BytecodeInstr::Slice => StackEffect { pops: 3, pushes: 1 },
        BytecodeInstr::Len => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::Size => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::Bytes => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::ArrayPush => StackEffect { pops: 2, pushes: 1 },
        BytecodeInstr::MakeStruct(_name, _fields, count) => StackEffect { pops: *count, pushes: 1 },
        BytecodeInstr::GetField(_) => StackEffect { pops: 1, pushes: 1 },
        BytecodeInstr::SetField(_) => StackEffect { pops: 2, pushes: 1 },
        BytecodeInstr::AddI64LL(_, _) | BytecodeInstr::AddI64LC(_, _) => StackEffect { pops: 0, pushes: 1 },
    };
    Ok(effect)
}
