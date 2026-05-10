use std::collections::{HashMap, VecDeque};
use thiserror::Error;
use crate::bytecode::BytecodeModule;
use crate::constant::ConstId;
use crate::function::{FunctionId, LocalId, BytecodeType};
use crate::opcode::{BytecodeInstr, stack_effect};

#[derive(Debug, Error, PartialEq, Clone)]
pub enum BytecodeVerifyError {
    #[error("invalid function id: @{0}")]
    InvalidFunctionId(FunctionId),

    #[error("invalid constant id: #{0}")]
    InvalidConstId(ConstId),

    #[error("invalid local id: ${0}")]
    InvalidLocalId(LocalId),

    #[error("invalid jump target: {0}")]
    InvalidJumpTarget(usize),

    #[error("stack underflow")]
    StackUnderflow,

    #[error("stack type mismatch: expected {expected:?}, found {found:?}")]
    StackTypeMismatch { expected: BytecodeType, found: BytecodeType },

    #[error("stack height mismatch at merge point: expected {expected}, found {found}")]
    StackHeightMismatch { expected: usize, found: usize },

    #[error("local not initialized: ${0}")]
    LocalNotInitialized(LocalId),

    #[error("return type mismatch: expected {expected:?}, found {found:?}")]
    ReturnTypeMismatch { expected: BytecodeType, found: Option<BytecodeType> },

    #[error("call arity mismatch for @{id}: expected {expected}, found {found}")]
    CallArityMismatch { id: FunctionId, expected: u16, found: u16 },

    #[error("max stack exceeded: declared {declared}, required {required}")]
    MaxStackExceeded { declared: u32, required: u32 },

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Message(String),
}

#[derive(Clone, Debug, PartialEq)]
struct VerificationState {
    stack: Vec<BytecodeType>,
    locals_init: Vec<bool>,
}

pub fn verify_module(module: &BytecodeModule) -> Result<(), BytecodeVerifyError> {
    for i in 0..module.functions.len() {
        verify_function(module, FunctionId(i as u32))?;
    }
    Ok(())
}

pub fn verify_function(module: &BytecodeModule, func_id: FunctionId) -> Result<(), BytecodeVerifyError> {
    let func = module.functions.get(func_id.0 as usize)
        .ok_or(BytecodeVerifyError::InvalidFunctionId(func_id))?;

    if func.code.is_empty() {
        return Ok(());
    }

    let mut states: HashMap<usize, VerificationState> = HashMap::new();
    let mut worklist: VecDeque<usize> = VecDeque::new();

    // Initial state: params are initialized, stack is empty
    let mut initial_locals_init = vec![false; func.locals.len()];
    for i in 0..func.params.len() {
        initial_locals_init[i] = true;
    }
    
    let initial_state = VerificationState {
        stack: Vec::new(),
        locals_init: initial_locals_init,
    };

    states.insert(0, initial_state);
    worklist.push_back(0);

    let mut observed_max_stack = 0;

    while let Some(pc) = worklist.pop_front() {
        let mut state = states.get(&pc).unwrap().clone();
        
        // Process instructions starting from pc until a terminator or branch
        let mut current_pc = pc;
        while current_pc < func.code.len() {
            let instr = &func.code[current_pc];
            
            // Check stack underflow and type safety
            let effect = stack_effect(instr, module, func)
                .map_err(|e| BytecodeVerifyError::Message(e.to_string()))?;
            
            if state.stack.len() < effect.pops as usize {
                return Err(BytecodeVerifyError::StackUnderflow);
            }

            // Perform instruction-specific checks and update stack types
            match instr {
                BytecodeInstr::ConstUnit => state.stack.push(BytecodeType::Unit),
                BytecodeInstr::ConstBool(_) => state.stack.push(BytecodeType::Bool),
                BytecodeInstr::ConstI64(id) | BytecodeInstr::ConstU64(id) | 
                BytecodeInstr::ConstF64(id) | BytecodeInstr::ConstString(id) => {
                    let constant = module.constants.get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidConstId(*id))?;
                    let ty = match constant {
                        crate::constant::BytecodeConstant::Int64(_) => BytecodeType::Int64,
                        crate::constant::BytecodeConstant::UInt64(_) => BytecodeType::UInt64,
                        crate::constant::BytecodeConstant::Float64(_) => BytecodeType::Float64,
                        crate::constant::BytecodeConstant::String(_) => BytecodeType::String,
                        _ => return Err(BytecodeVerifyError::Unsupported("Complex constants in verifier".into())),
                    };
                    state.stack.push(ty);
                }
                BytecodeInstr::LoadLocal(id) => {
                    if id.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(*id));
                    }
                    if !state.locals_init[id.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(*id));
                    }
                    state.stack.push(func.locals[id.0 as usize]);
                }
                BytecodeInstr::StoreLocal(id) => {
                    if id.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(*id));
                    }
                    let val_ty = state.stack.pop().unwrap();
                    let local_ty = func.locals[id.0 as usize];
                    if val_ty != local_ty && val_ty != BytecodeType::Any && local_ty != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: local_ty, found: val_ty });
                    }
                    state.locals_init[id.0 as usize] = true;
                }
                BytecodeInstr::Pop => { state.stack.pop(); }
                BytecodeInstr::Dup => {
                    let ty = *state.stack.last().unwrap();
                    state.stack.push(ty);
                }
                BytecodeInstr::AddI64 | BytecodeInstr::SubI64 | BytecodeInstr::MulI64 | BytecodeInstr::DivI64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::Int64, BytecodeType::Int64)?;
                }
                BytecodeInstr::AddU64 | BytecodeInstr::SubU64 | BytecodeInstr::MulU64 | BytecodeInstr::DivU64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::UInt64, BytecodeType::UInt64)?;
                }
                BytecodeInstr::AddF64 | BytecodeInstr::SubF64 | BytecodeInstr::MulF64 | BytecodeInstr::DivF64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::Float64, BytecodeType::Float64)?;
                }
                BytecodeInstr::Eq => {
                    let rhs = state.stack.pop().unwrap();
                    let lhs = state.stack.pop().unwrap();
                    if lhs != rhs && lhs != BytecodeType::Any && rhs != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: lhs, found: rhs });
                    }
                    state.stack.push(BytecodeType::Bool);
                }
                BytecodeInstr::LtI64 | BytecodeInstr::GtI64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::Int64, BytecodeType::Bool)?;
                }
                BytecodeInstr::LtU64 | BytecodeInstr::GtU64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::UInt64, BytecodeType::Bool)?;
                }
                BytecodeInstr::LtF64 | BytecodeInstr::GtF64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::Float64, BytecodeType::Bool)?;
                }
                BytecodeInstr::Not => {
                    let ty = state.stack.pop().unwrap();
                    if ty != BytecodeType::Bool && ty != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: BytecodeType::Bool, found: ty });
                    }
                    state.stack.push(BytecodeType::Bool);
                }
                BytecodeInstr::And | BytecodeInstr::Or => {
                    check_stack_binary(&mut state.stack, BytecodeType::Bool, BytecodeType::Bool)?;
                }
                BytecodeInstr::Jump(target) => {
                    propagate_state(target.0, &state, &mut states, &mut worklist, func)?;
                    break; // End of block
                }
                BytecodeInstr::JumpIfFalse(target) => {
                    let ty = state.stack.pop().unwrap();
                    if ty != BytecodeType::Bool && ty != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: BytecodeType::Bool, found: ty });
                    }
                    propagate_state(target.0, &state, &mut states, &mut worklist, func)?;
                    // Fallthrough
                    propagate_state(current_pc + 1, &state, &mut states, &mut worklist, func)?;
                    break; // End of block
                }
                BytecodeInstr::Return => {
                    if func.ret == BytecodeType::Unit {
                        if !state.stack.is_empty() {
                            return Err(BytecodeVerifyError::ReturnTypeMismatch { expected: BytecodeType::Unit, found: Some(*state.stack.last().unwrap()) });
                        }
                    } else {
                        if state.stack.len() != 1 {
                             return Err(BytecodeVerifyError::ReturnTypeMismatch { expected: func.ret, found: state.stack.last().cloned() });
                        }
                        let found = state.stack.pop().unwrap();
                        if found != func.ret && found != BytecodeType::Any && func.ret != BytecodeType::Any {
                            return Err(BytecodeVerifyError::ReturnTypeMismatch { expected: func.ret, found: Some(found) });
                        }
                    }
                    break; // End of path
                }
                BytecodeInstr::Unreachable => {
                    break; // End of path
                }
                BytecodeInstr::Call(id, argc) => {
                    let target = module.functions.get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidFunctionId(*id))?;
                    if target.params.len() != *argc as usize {
                        return Err(BytecodeVerifyError::CallArityMismatch { id: *id, expected: target.params.len() as u16, found: *argc });
                    }
                    // Check argument types
                    for expected in target.params.iter().rev() {
                        let found = state.stack.pop().unwrap();
                        if found != *expected && *expected != BytecodeType::Any && found != BytecodeType::Any {
                            return Err(BytecodeVerifyError::StackTypeMismatch { expected: *expected, found });
                        }
                    }
                    if target.ret != BytecodeType::Unit {
                        state.stack.push(target.ret);
                    }
                }
                BytecodeInstr::MakeEnum(_, _, count) => {
                    for _ in 0..*count {
                        state.stack.pop().unwrap(); // pop payloads
                    }
                    state.stack.push(BytecodeType::Any); // pushes Any or Enum
                }
                BytecodeInstr::Tag => {
                    let found = state.stack.pop().unwrap();
                    if found != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: BytecodeType::Any, found });
                    }
                    state.stack.push(BytecodeType::String);
                }
                BytecodeInstr::Unwrap(_, _) => {
                    let found = state.stack.pop().unwrap();
                    if found != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected: BytecodeType::Any, found });
                    }
                    state.stack.push(BytecodeType::Any); // pushes payload
                }
            }

            if state.stack.len() > observed_max_stack {
                observed_max_stack = state.stack.len();
            }

            current_pc += 1;
        }
    }

    if func.max_stack == 0 && observed_max_stack > 0 {
        return Err(BytecodeVerifyError::MaxStackExceeded { declared: 0, required: observed_max_stack as u32 });
    }
    if func.max_stack > 0 && observed_max_stack > func.max_stack as usize {
        return Err(BytecodeVerifyError::MaxStackExceeded { declared: func.max_stack, required: observed_max_stack as u32 });
    }

    Ok(())
}

fn check_stack_binary(stack: &mut Vec<BytecodeType>, expected_operand: BytecodeType, result: BytecodeType) -> Result<(), BytecodeVerifyError> {
    let rhs = stack.pop().unwrap();
    let lhs = stack.pop().unwrap();
    if lhs != expected_operand && lhs != BytecodeType::Any && expected_operand != BytecodeType::Any {
        return Err(BytecodeVerifyError::StackTypeMismatch { expected: expected_operand, found: lhs });
    }
    if rhs != expected_operand && rhs != BytecodeType::Any && expected_operand != BytecodeType::Any {
        return Err(BytecodeVerifyError::StackTypeMismatch { expected: expected_operand, found: rhs });
    }
    stack.push(result);
    Ok(())
}

fn propagate_state(
    target: usize,
    state: &VerificationState,
    states: &mut HashMap<usize, VerificationState>,
    worklist: &mut VecDeque<usize>,
    func: &crate::function::BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    if target >= func.code.len() {
        return Err(BytecodeVerifyError::InvalidJumpTarget(target));
    }

    if let Some(existing) = states.get_mut(&target) {
        // Merge states
        if existing.stack.len() != state.stack.len() {
            return Err(BytecodeVerifyError::StackHeightMismatch { expected: existing.stack.len(), found: state.stack.len() });
        }
        for (e, s) in existing.stack.iter_mut().zip(state.stack.iter()) {
            if *e != *s {
                if *e == BytecodeType::Any || *s == BytecodeType::Any {
                    *e = BytecodeType::Any;
                } else {
                    return Err(BytecodeVerifyError::StackTypeMismatch { expected: *e, found: *s });
                }
            }
        }

        // Merge locals initialization (intersection)
        let mut changed = false;
        for i in 0..existing.locals_init.len() {
            if existing.locals_init[i] && !state.locals_init[i] {
                existing.locals_init[i] = false;
                changed = true;
            }
        }
        if changed {
            worklist.push_back(target);
        }
    } else {
        states.insert(target, state.clone());
        worklist.push_back(target);
    }
    Ok(())
}
