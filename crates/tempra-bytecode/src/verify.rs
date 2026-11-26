use crate::bytecode::BytecodeModule;
use crate::constant::ConstId;
use crate::function::{
    BytecodeFunction, BytecodeSignature, BytecodeType, FunctionId, LocalId, SignatureId,
};
use crate::opcode::{stack_effect, BytecodeInstr};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Clone)]
pub enum BytecodeVerifyError {
    #[error("invalid function id: @{0}")]
    InvalidFunctionId(FunctionId),

    #[error("invalid callable signature id: #{0:?}")]
    InvalidSignatureId(SignatureId),

    #[error("missing closure signature for function @{0}")]
    MissingClosureSignature(FunctionId),

    #[error("invalid constant id: #{0}")]
    InvalidConstId(ConstId),

    #[error("invalid local id: ${0}")]
    InvalidLocalId(LocalId),

    #[error("invalid jump target: {0}")]
    InvalidJumpTarget(usize),

    #[error("stack underflow")]
    StackUnderflow,

    #[error("stack type mismatch: expected {expected:?}, found {found:?}")]
    StackTypeMismatch {
        expected: BytecodeType,
        found: BytecodeType,
    },

    #[error("stack height mismatch at merge point: expected {expected}, found {found}")]
    StackHeightMismatch { expected: usize, found: usize },

    #[error("local not initialized: ${0}")]
    LocalNotInitialized(LocalId),

    #[error("return type mismatch: expected {expected:?}, found {found:?}")]
    ReturnTypeMismatch {
        expected: BytecodeType,
        found: Option<BytecodeType>,
    },

    #[error("call arity mismatch for @{id}: expected {expected}, found {found}")]
    CallArityMismatch {
        id: FunctionId,
        expected: u16,
        found: u16,
    },

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

impl VerificationState {
    fn pop_ty(&mut self) -> Result<BytecodeType, BytecodeVerifyError> {
        self.stack.pop().ok_or(BytecodeVerifyError::StackUnderflow)
    }

    fn peek_ty(&self) -> Result<BytecodeType, BytecodeVerifyError> {
        self.stack
            .last()
            .cloned()
            .ok_or(BytecodeVerifyError::StackUnderflow)
    }
}

fn verify_callable_type(
    module: &BytecodeModule,
    ty: BytecodeType,
) -> Result<(), BytecodeVerifyError> {
    if let BytecodeType::Callable(id) = ty {
        if module.signatures.get(id.0 as usize).is_none() {
            return Err(BytecodeVerifyError::InvalidSignatureId(id));
        }
    }
    Ok(())
}

pub fn verify_module(module: &BytecodeModule) -> Result<(), BytecodeVerifyError> {
    for signature in &module.signatures {
        if signature.params.contains(&BytecodeType::ExclusiveSlice)
            || signature.ret == BytecodeType::ExclusiveSlice
        {
            return Err(BytecodeVerifyError::Unsupported(
                "exclusive slice cannot occur in callable signature".into(),
            ));
        }
        for param in &signature.params {
            verify_callable_type(module, *param)?;
        }
        verify_callable_type(module, signature.ret)?;
    }
    for i in 0..module.functions.len() {
        verify_function(module, FunctionId(i as u32))?;
    }
    Ok(())
}

pub fn verify_function(
    module: &BytecodeModule,
    func_id: FunctionId,
) -> Result<(), BytecodeVerifyError> {
    let func = module
        .functions
        .get(func_id.0 as usize)
        .ok_or(BytecodeVerifyError::InvalidFunctionId(func_id))?;

    verify_function_direct(module, func)
}

pub fn verify_function_direct(
    module: &BytecodeModule,
    func: &BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    for ty in func
        .params
        .iter()
        .chain(func.locals.iter())
        .chain(std::iter::once(&func.ret))
    {
        verify_callable_type(module, *ty)?;
    }
    if func.inout_params.len() != func.params.len() {
        return Err(BytecodeVerifyError::Unsupported(
            "inout parameter access metadata does not match function arity".into(),
        ));
    }
    if func.is_extern && func.inout_params.iter().any(|value| *value) {
        return Err(BytecodeVerifyError::Unsupported(
            "extern inout calls require a separate trusted ABI".into(),
        ));
    }
    verify_exclusive_slice_layout(module, func)?;
    verify_read_return_layout(func)?;
    if !func.code.is_empty()
        && func.params.iter().enumerate().any(|(index, ty)| {
            *ty == BytecodeType::ReadSlice
                && (func.locals.get(index) != Some(ty) || func.inout_params[index])
        })
    {
        return Err(BytecodeVerifyError::Unsupported(
            "read slice parameter requires a matching non-exclusive local slot".into(),
        ));
    }
    if func.inout_params.iter().enumerate().any(|(index, access)| {
        *access
            && (func.locals.get(index) != func.params.get(index)
                || matches!(func.params[index], BytecodeType::Callable(_)))
    }) {
        return Err(BytecodeVerifyError::Unsupported(
            "inout parameter requires a matching non-callable local slot".into(),
        ));
    }
    if func.code.is_empty() {
        return Ok(());
    }
    verify_borrowed_frame_contract(module, func)?;
    // Every parameter occupies an initialized local. A malformed artifact must
    // fail verification before indexing the local-state vector.
    if func.params.len() > func.locals.len() {
        return Err(BytecodeVerifyError::InvalidLocalId(LocalId(
            func.locals.len() as u32,
        )));
    }

    let mut states: HashMap<usize, VerificationState> = HashMap::new();
    let mut worklist: VecDeque<usize> = VecDeque::new();

    // Initial state: params are initialized, stack is empty
    let mut initial_locals_init = vec![false; func.locals.len()];
    for init in initial_locals_init.iter_mut().take(func.params.len()) {
        *init = true;
    }

    let initial_state = VerificationState {
        stack: Vec::new(),
        locals_init: initial_locals_init,
    };

    states.insert(0, initial_state);
    worklist.push_back(0);

    let mut observed_max_stack = 0;

    while let Some(pc) = worklist.pop_front() {
        let mut state = states
            .get(&pc)
            .ok_or(BytecodeVerifyError::InvalidJumpTarget(pc))?
            .clone();

        // Process instructions starting from pc until a terminator or branch
        let mut current_pc = pc;
        while current_pc < func.code.len() {
            let instr = &func.code[current_pc];

            // Resolve signature IDs before stack effects so malformed bytecode
            // receives a structured error rather than a generic instruction error.
            if let BytecodeInstr::CallClosure(signature_id, _) = instr {
                verify_callable_type(module, BytecodeType::Callable(*signature_id))?;
            }

            // Check stack underflow and type safety
            let effect = stack_effect(instr, module, func)
                .map_err(|e| BytecodeVerifyError::Message(e.to_string()))?;

            if state.stack.len() < effect.pops as usize {
                return Err(BytecodeVerifyError::StackUnderflow);
            }

            // Perform instruction-specific checks and update stack types
            match *instr {
                BytecodeInstr::ConstUnit => state.stack.push(BytecodeType::Unit),
                BytecodeInstr::ConstBool(_) => state.stack.push(BytecodeType::Bool),
                BytecodeInstr::ConstI64(id)
                | BytecodeInstr::ConstU64(id)
                | BytecodeInstr::ConstF64(id)
                | BytecodeInstr::ConstString(id) => {
                    let constant = module
                        .constants
                        .get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidConstId(id))?;
                    let ty = match constant {
                        crate::constant::BytecodeConstant::Int64(_) => BytecodeType::Int64,
                        crate::constant::BytecodeConstant::UInt64(_) => BytecodeType::UInt64,
                        crate::constant::BytecodeConstant::Float64(_) => BytecodeType::Float64,
                        crate::constant::BytecodeConstant::String(_) => BytecodeType::String,
                        _ => {
                            return Err(BytecodeVerifyError::Unsupported(
                                "Complex constants in verifier".into(),
                            ))
                        }
                    };
                    state.stack.push(ty);
                }
                BytecodeInstr::LoadLocal(id) => {
                    if id.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(id));
                    }
                    if !state.locals_init[id.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(id));
                    }
                    state.stack.push(func.locals[id.0 as usize]);
                }
                BytecodeInstr::StoreLocal(id) => {
                    if id.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(id));
                    }
                    let val_ty = state.pop_ty()?;
                    let local_ty = func.locals[id.0 as usize];
                    if val_ty != local_ty
                        && val_ty != BytecodeType::Any
                        && local_ty != BytecodeType::Any
                    {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: local_ty,
                            found: val_ty,
                        });
                    }
                    state.locals_init[id.0 as usize] = true;
                }
                BytecodeInstr::Pop => {
                    state.pop_ty()?;
                }
                BytecodeInstr::Dup => {
                    let ty = state.peek_ty()?;
                    state.stack.push(ty);
                }
                BytecodeInstr::AddI64
                | BytecodeInstr::SubI64
                | BytecodeInstr::MulI64
                | BytecodeInstr::DivI64 => {
                    check_stack_binary(&mut state.stack, BytecodeType::Int64, BytecodeType::Int64)?;
                }
                BytecodeInstr::AddU64
                | BytecodeInstr::SubU64
                | BytecodeInstr::MulU64
                | BytecodeInstr::DivU64 => {
                    check_stack_binary(
                        &mut state.stack,
                        BytecodeType::UInt64,
                        BytecodeType::UInt64,
                    )?;
                }
                BytecodeInstr::AddF64
                | BytecodeInstr::SubF64
                | BytecodeInstr::MulF64
                | BytecodeInstr::DivF64 => {
                    check_stack_binary(
                        &mut state.stack,
                        BytecodeType::Float64,
                        BytecodeType::Float64,
                    )?;
                }
                BytecodeInstr::Eq => {
                    let rhs = state.pop_ty()?;
                    let lhs = state.pop_ty()?;
                    if lhs != rhs && lhs != BytecodeType::Any && rhs != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: lhs,
                            found: rhs,
                        });
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
                    check_stack_binary(
                        &mut state.stack,
                        BytecodeType::Float64,
                        BytecodeType::Bool,
                    )?;
                }
                BytecodeInstr::Not => {
                    let ty = state.pop_ty()?;
                    if ty != BytecodeType::Bool && ty != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Bool,
                            found: ty,
                        });
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
                    let ty = state.pop_ty()?;
                    if ty != BytecodeType::Bool && ty != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Bool,
                            found: ty,
                        });
                    }
                    propagate_state(target.0, &state, &mut states, &mut worklist, func)?;
                    // Fallthrough
                    propagate_state(current_pc + 1, &state, &mut states, &mut worklist, func)?;
                    break; // End of block
                }
                BytecodeInstr::Return => {
                    if func.ret == BytecodeType::Unit {
                        if !state.stack.is_empty() {
                            return Err(BytecodeVerifyError::ReturnTypeMismatch {
                                expected: BytecodeType::Unit,
                                found: Some(state.peek_ty()?),
                            });
                        }
                    } else {
                        if state.stack.len() != 1 {
                            return Err(BytecodeVerifyError::ReturnTypeMismatch {
                                expected: func.ret,
                                found: state.stack.last().cloned(),
                            });
                        }
                        let found = state.pop_ty()?;
                        if found != func.ret
                            && found != BytecodeType::Any
                            && func.ret != BytecodeType::Any
                        {
                            return Err(BytecodeVerifyError::ReturnTypeMismatch {
                                expected: func.ret,
                                found: Some(found),
                            });
                        }
                    }
                    break; // End of path
                }
                BytecodeInstr::Unreachable => {
                    break; // End of path
                }
                BytecodeInstr::BorrowSliceMut(id) => {
                    let ty = func
                        .locals
                        .get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidLocalId(id))?;
                    if !matches!(ty, BytecodeType::Any | BytecodeType::ExclusiveSlice)
                        || !state.locals_init[id.0 as usize]
                    {
                        return Err(BytecodeVerifyError::Unsupported(
                            "invalid exclusive slice source local".into(),
                        ));
                    }
                    for _ in 0..2 {
                        let found = state.pop_ty()?;
                        if !matches!(found, BytecodeType::Int64 | BytecodeType::Any) {
                            return Err(BytecodeVerifyError::StackTypeMismatch {
                                expected: BytecodeType::Int64,
                                found,
                            });
                        }
                    }
                    state.stack.push(BytecodeType::ExclusiveSlice);
                }
                BytecodeInstr::InoutSliceGet(_) => {
                    let found = state.pop_ty()?;
                    if !matches!(found, BytecodeType::Int64 | BytecodeType::Any) {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Int64,
                            found,
                        });
                    }
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::InoutSliceSet(_) => {
                    let value = state.pop_ty()?;
                    if !matches!(
                        value,
                        BytecodeType::Any
                            | BytecodeType::Int64
                            | BytecodeType::UInt64
                            | BytecodeType::UInt8
                            | BytecodeType::Float64
                            | BytecodeType::Bool
                    ) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "exclusive slice write requires scalar value".into(),
                        ));
                    }
                    let found = state.pop_ty()?;
                    if !matches!(found, BytecodeType::Int64 | BytecodeType::Any) {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Int64,
                            found,
                        });
                    }
                }
                BytecodeInstr::InoutSliceLen(_) => state.stack.push(BytecodeType::Int64),
                BytecodeInstr::BorrowLocal(id) | BytecodeInstr::BorrowLocalMut(id) => {
                    if id.0 as usize >= func.locals.len() {
                        return Err(BytecodeVerifyError::InvalidLocalId(id));
                    }
                    if !state.locals_init[id.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(id));
                    }
                    state.stack.push(func.locals[id.0 as usize]);
                }
                BytecodeInstr::MakeClosureRef(target_id, capture_count)
                | BytecodeInstr::MakeClosureInout(target_id, capture_count) => {
                    let target = module
                        .functions
                        .get(target_id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidFunctionId(target_id))?;
                    let hidden_count = usize::from(capture_count);
                    if hidden_count == 0 || hidden_count > target.params.len() {
                        return Err(BytecodeVerifyError::Message(
                            "borrowed closure requires a nonempty valid capture prefix".into(),
                        ));
                    }
                    let exclusive = matches!(instr, BytecodeInstr::MakeClosureInout(_, _));
                    verify_closure_access(target, hidden_count, exclusive, !exclusive)?;
                    let public_signature = crate::function::BytecodeSignature {
                        params: target.params[hidden_count..].to_vec(),
                        ret: target.ret,
                    };
                    let signature_index = module
                        .signatures
                        .iter()
                        .position(|sig| sig == &public_signature)
                        .ok_or(BytecodeVerifyError::MissingClosureSignature(target_id))?;
                    for expected in target.params[..hidden_count].iter().rev() {
                        let found = state.pop_ty()?;
                        if found != *expected {
                            return Err(BytecodeVerifyError::StackTypeMismatch {
                                expected: *expected,
                                found,
                            });
                        }
                    }
                    state
                        .stack
                        .push(BytecodeType::Callable(SignatureId(signature_index as u32)));
                }
                BytecodeInstr::MakeClosure(target_id, capture_count) => {
                    let target = module
                        .functions
                        .get(target_id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidFunctionId(target_id))?;
                    let hidden_count = usize::from(capture_count);
                    if hidden_count > target.params.len() {
                        return Err(BytecodeVerifyError::Message(format!(
                            "closure has {} captures but target accepts {} parameters",
                            hidden_count,
                            target.params.len()
                        )));
                    }
                    verify_closure_access(target, hidden_count, false, false)?;
                    let public_signature = BytecodeSignature {
                        params: target.params[hidden_count..].to_vec(),
                        ret: target.ret,
                    };
                    let signature_index = module
                        .signatures
                        .iter()
                        .position(|signature| signature == &public_signature)
                        .ok_or(BytecodeVerifyError::MissingClosureSignature(target_id))?;
                    for expected in target.params[..hidden_count].iter().rev() {
                        let found = state.pop_ty()?;
                        if found != *expected {
                            return Err(BytecodeVerifyError::StackTypeMismatch {
                                expected: *expected,
                                found,
                            });
                        }
                    }
                    state
                        .stack
                        .push(BytecodeType::Callable(SignatureId(signature_index as u32)));
                }
                BytecodeInstr::CallClosure(signature_id, argc) => {
                    let signature = module
                        .signatures
                        .get(signature_id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidSignatureId(signature_id))?;
                    if usize::from(argc) != signature.params.len() {
                        return Err(BytecodeVerifyError::Message(format!(
                            "indirect call expects {} arguments, found {}",
                            signature.params.len(),
                            argc
                        )));
                    }
                    for expected in signature.params.iter().rev() {
                        let found = state.pop_ty()?;
                        if found != *expected {
                            return Err(BytecodeVerifyError::StackTypeMismatch {
                                expected: *expected,
                                found,
                            });
                        }
                    }
                    let found = state.pop_ty()?;
                    let expected = BytecodeType::Callable(signature_id);
                    if found != expected {
                        return Err(BytecodeVerifyError::StackTypeMismatch { expected, found });
                    }
                    if signature.ret == BytecodeType::ReadSlice {
                        return Err(BytecodeVerifyError::Unsupported(
                            "indirect slice return requires a provenance contract".into(),
                        ));
                    }
                    if signature.ret != BytecodeType::Unit {
                        state.stack.push(signature.ret);
                    }
                }
                BytecodeInstr::Call(id, argc) => {
                    let target = module
                        .functions
                        .get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidFunctionId(id))?;
                    if target.params.len() != argc as usize {
                        return Err(BytecodeVerifyError::CallArityMismatch {
                            id,
                            expected: target.params.len() as u16,
                            found: argc,
                        });
                    }
                    // Check argument types; access-mode correspondence is
                    // independently certified by the borrow provenance verifier.
                    for expected_ref in target.params.iter().rev() {
                        let expected = *expected_ref;
                        let found = state.pop_ty()?;
                        if found != expected
                            && (expected == BytecodeType::ExclusiveSlice
                                || found == BytecodeType::ExclusiveSlice
                                || (expected != BytecodeType::Any && found != BytecodeType::Any))
                        {
                            return Err(BytecodeVerifyError::StackTypeMismatch { expected, found });
                        }
                    }
                    if target.ret != BytecodeType::Unit {
                        state.stack.push(target.ret);
                    }
                }
                BytecodeInstr::MakeEnum(_, _, count) => {
                    for _ in 0..count {
                        state.pop_ty()?; // pop payloads
                    }
                    state.stack.push(BytecodeType::Any); // pushes Any or Enum
                }
                BytecodeInstr::Tag => {
                    let found = state.pop_ty()?;
                    if found != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Any,
                            found,
                        });
                    }
                    state.stack.push(BytecodeType::String);
                }
                BytecodeInstr::Unwrap(_, _) => {
                    let found = state.pop_ty()?;
                    if found != BytecodeType::Any {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Any,
                            found,
                        });
                    }
                    state.stack.push(BytecodeType::Any); // pushes payload
                }
                BytecodeInstr::MakeArray(count) => {
                    for _ in 0..count {
                        state.pop_ty()?;
                    }
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::MakeTuple(count) => {
                    for _ in 0..count {
                        state.pop_ty()?;
                    }
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::GetIndex => {
                    state.pop_ty()?; // pop index
                    state.pop_ty()?; // pop object
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::Slice => {
                    for _ in 0..2 {
                        let found = state.pop_ty()?;
                        if !matches!(found, BytecodeType::Int64 | BytecodeType::Any) {
                            return Err(BytecodeVerifyError::StackTypeMismatch {
                                expected: BytecodeType::Int64,
                                found,
                            });
                        }
                    }
                    let found = state.pop_ty()?;
                    if !matches!(found, BytecodeType::ReadSlice | BytecodeType::Any) {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::ReadSlice,
                            found,
                        });
                    }
                    state.stack.push(BytecodeType::ReadSlice);
                }
                BytecodeInstr::Len | BytecodeInstr::Size => {
                    state.pop_ty()?; // pop object
                    state.stack.push(BytecodeType::Int64);
                }
                BytecodeInstr::Bytes => {
                    let found = state.pop_ty()?;
                    if !matches!(found, BytecodeType::String | BytecodeType::Any) {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::String,
                            found,
                        });
                    }
                    state.stack.push(BytecodeType::ReadSlice);
                }
                BytecodeInstr::MakeStruct(_, _, count) => {
                    for _ in 0..count {
                        state.pop_ty()?;
                    }
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::GetField(_) => {
                    state.pop_ty()?; // pop object
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::SetField(_) => {
                    state.pop_ty()?; // pop value
                    state.pop_ty()?; // pop object
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::ArrayPush => {
                    state.pop_ty()?; // pop value
                    state.pop_ty()?; // pop array
                    state.stack.push(BytecodeType::Any);
                }
                BytecodeInstr::AddI64LL(a, b) => {
                    if a.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(a));
                    }
                    if !state.locals_init[a.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(a));
                    }
                    if func.locals[a.0 as usize] != BytecodeType::Int64
                        && func.locals[a.0 as usize] != BytecodeType::Any
                    {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Int64,
                            found: func.locals[a.0 as usize],
                        });
                    }
                    if b.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(b));
                    }
                    if !state.locals_init[b.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(b));
                    }
                    if func.locals[b.0 as usize] != BytecodeType::Int64
                        && func.locals[b.0 as usize] != BytecodeType::Any
                    {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Int64,
                            found: func.locals[b.0 as usize],
                        });
                    }
                    state.stack.push(BytecodeType::Int64);
                }
                BytecodeInstr::AddI64LC(a, id) => {
                    if a.0 >= func.locals.len() as u32 {
                        return Err(BytecodeVerifyError::InvalidLocalId(a));
                    }
                    if !state.locals_init[a.0 as usize] {
                        return Err(BytecodeVerifyError::LocalNotInitialized(a));
                    }
                    if func.locals[a.0 as usize] != BytecodeType::Int64
                        && func.locals[a.0 as usize] != BytecodeType::Any
                    {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: BytecodeType::Int64,
                            found: func.locals[a.0 as usize],
                        });
                    }
                    let constant = module
                        .constants
                        .get(id.0 as usize)
                        .ok_or(BytecodeVerifyError::InvalidConstId(id))?;
                    if !matches!(constant, crate::constant::BytecodeConstant::Int64(_)) {
                        return Err(BytecodeVerifyError::Message(
                            "Expected Int64 constant".into(),
                        ));
                    }
                    state.stack.push(BytecodeType::Int64);
                }
            }

            if state.stack.len() > observed_max_stack {
                observed_max_stack = state.stack.len();
            }

            current_pc += 1;
        }
    }

    if func.max_stack == 0 && observed_max_stack > 0 {
        return Err(BytecodeVerifyError::MaxStackExceeded {
            declared: 0,
            required: observed_max_stack as u32,
        });
    }
    if func.max_stack > 0 && observed_max_stack > func.max_stack as usize {
        return Err(BytecodeVerifyError::MaxStackExceeded {
            declared: func.max_stack,
            required: observed_max_stack as u32,
        });
    }

    verify_read_return_provenance(module, func)?;
    Ok(())
}

fn read_return_error() -> BytecodeVerifyError {
    BytecodeVerifyError::Unsupported(
        "read slice return must derive from its declared parameter on every path".into(),
    )
}

fn verify_read_return_layout(func: &BytecodeFunction) -> Result<(), BytecodeVerifyError> {
    match (func.ret, func.return_borrow) {
        (BytecodeType::ReadSlice, Some(index)) => {
            let index = index as usize;
            if func.params.get(index) != Some(&BytecodeType::ReadSlice)
                || func.locals.get(index) != Some(&BytecodeType::ReadSlice)
                || func.inout_params.get(index) != Some(&false)
                || func.is_extern
                || func.code.is_empty()
            {
                return Err(read_return_error());
            }
        }
        (BytecodeType::ReadSlice, None) | (_, Some(_)) => return Err(read_return_error()),
        _ => {}
    }
    Ok(())
}

/// Independent must-analysis of the declared source parameter through the
/// operand stack, local slots and direct calls. No source metadata is taken
/// on faith: every function's declared read-return contract is checked.
fn verify_read_return_provenance(
    module: &BytecodeModule,
    func: &BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    let Some(parameter) = func.return_borrow else {
        return Ok(());
    };
    // The ordinary verifier has already checked all reachable stack effects,
    // argument counts, local IDs and jump targets before this pass runs.
    let exposed: HashSet<LocalId> = func
        .code
        .iter()
        .filter_map(|instr| match instr {
            BytecodeInstr::BorrowLocalMut(id) | BytecodeInstr::BorrowSliceMut(id) => Some(*id),
            _ => None,
        })
        .collect();
    #[derive(Clone)]
    struct State {
        stack: Vec<bool>,
        locals: Vec<bool>,
    }
    let mut initial = State {
        stack: vec![],
        locals: vec![false; func.locals.len()],
    };
    initial.locals[parameter as usize] = !exposed.contains(&LocalId(parameter));
    let mut states = HashMap::from([(0usize, initial)]);
    let mut pending = VecDeque::from([0usize]);
    while let Some(pc) = pending.pop_front() {
        let mut state = states[&pc].clone();
        let instr = func.code.get(pc).ok_or_else(read_return_error)?;
        let effect = stack_effect(instr, module, func)
            .map_err(|e| BytecodeVerifyError::Message(e.to_string()))?;
        let base = state
            .stack
            .len()
            .checked_sub(usize::from(effect.pops))
            .ok_or(BytecodeVerifyError::StackUnderflow)?;
        let args = state.stack.split_off(base);
        match *instr {
            BytecodeInstr::LoadLocal(id) => state.stack.push(state.locals[id.0 as usize]),
            BytecodeInstr::StoreLocal(id) => {
                state.locals[id.0 as usize] = args[0] && !exposed.contains(&id);
            }
            BytecodeInstr::Dup => state.stack.extend([args[0], args[0]]),
            BytecodeInstr::Slice => state.stack.push(args[0]),
            BytecodeInstr::Call(target, _) if effect.pushes == 1 => {
                let target = &module.functions[target.0 as usize];
                let origin = target.return_borrow.filter(|index| {
                    target.ret == BytecodeType::ReadSlice
                        && target.params.get(*index as usize) == Some(&BytecodeType::ReadSlice)
                        && target.inout_params.get(*index as usize) == Some(&false)
                });
                state.stack.push(
                    origin
                        .and_then(|index| args.get(index as usize))
                        .copied()
                        .unwrap_or(false),
                );
            }
            BytecodeInstr::Return => {
                if args.as_slice() != &[true] {
                    return Err(read_return_error());
                }
            }
            _ => state
                .stack
                .extend(std::iter::repeat_n(false, usize::from(effect.pushes))),
        }
        for next in borrow_successors(pc, func) {
            if next >= func.code.len() {
                return Err(BytecodeVerifyError::InvalidJumpTarget(next));
            }
            if let Some(existing) = states.get_mut(&next) {
                if existing.stack.len() != state.stack.len() {
                    return Err(read_return_error());
                }
                let mut changed = false;
                for (old, new) in existing
                    .stack
                    .iter_mut()
                    .zip(&state.stack)
                    .chain(existing.locals.iter_mut().zip(&state.locals))
                {
                    if *old && !*new {
                        *old = false;
                        changed = true;
                    }
                }
                if changed {
                    pending.push_back(next);
                }
            } else {
                states.insert(next, state.clone());
                pending.push_back(next);
            }
        }
    }
    Ok(())
}

// Borrowed closures are frame-scoped. The CFG verifier tracks whether a
// stack/local value may carry a borrowed closure or an unconsumed borrow handle.
// This is an *artifact integrity* proof; source-level last-use and aliasing
// remain the responsibility of the ownership checker (Memory v0.4, §14).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BorrowProvenance {
    Plain,
    Handle(LocalId),
    ExclusiveHandle(LocalId),
    ExclusiveSliceHandle(LocalId),
    BorrowedClosure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BorrowState {
    stack: Vec<BorrowProvenance>,
    locals: Vec<BorrowProvenance>,
    initialized: Vec<bool>,
}

fn borrow_successors(pc: usize, func: &BytecodeFunction) -> Vec<usize> {
    match func.code[pc] {
        BytecodeInstr::Jump(target) => vec![target.0],
        BytecodeInstr::JumpIfFalse(target) => vec![target.0, pc + 1],
        BytecodeInstr::Return | BytecodeInstr::Unreachable => vec![],
        _ => vec![pc + 1],
    }
}

// A single static StoreLocal is not necessarily a single *dynamic* write:
// executing it again via a back-edge would invalidate an existing loan.
fn borrow_instruction_can_repeat(func: &BytecodeFunction, store_pc: usize) -> bool {
    let mut pending = borrow_successors(store_pc, func);
    let mut seen = vec![false; func.code.len()];
    while let Some(pc) = pending.pop() {
        if pc == store_pc {
            return true;
        }
        if pc >= func.code.len() || seen[pc] {
            continue;
        }
        seen[pc] = true;
        pending.extend(borrow_successors(pc, func));
    }
    false
}

fn merge_borrow_provenance(
    old: BorrowProvenance,
    incoming: BorrowProvenance,
) -> Result<BorrowProvenance, BytecodeVerifyError> {
    if old == incoming {
        return Ok(old);
    }
    match (old, incoming) {
        (BorrowProvenance::Plain, BorrowProvenance::BorrowedClosure)
        | (BorrowProvenance::BorrowedClosure, BorrowProvenance::Plain) => {
            Ok(BorrowProvenance::BorrowedClosure)
        }
        // A raw handle cannot be widened to a plain value or to an handle
        // from another source: doing so would lose its exact provenance.
        _ => Err(BytecodeVerifyError::Unsupported(
            "incompatible borrowed-handle provenance at bytecode CFG merge".into(),
        )),
    }
}

fn propagate_borrow_state(
    pc: usize,
    incoming: &BorrowState,
    states: &mut HashMap<usize, BorrowState>,
    worklist: &mut VecDeque<usize>,
    func: &BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    if pc >= func.code.len() {
        return Err(BytecodeVerifyError::InvalidJumpTarget(pc));
    }
    if let Some(existing) = states.get_mut(&pc) {
        if existing.stack.len() != incoming.stack.len() {
            return Err(BytecodeVerifyError::StackHeightMismatch {
                expected: existing.stack.len(),
                found: incoming.stack.len(),
            });
        }
        let mut changed = false;
        for (old, new) in existing.stack.iter_mut().zip(&incoming.stack) {
            let merged = merge_borrow_provenance(*old, *new)?;
            if *old != merged {
                *old = merged;
                changed = true;
            }
        }
        for (old, new) in existing.locals.iter_mut().zip(&incoming.locals) {
            let merged = merge_borrow_provenance(*old, *new)?;
            if *old != merged {
                *old = merged;
                changed = true;
            }
        }
        for (old, new) in existing.initialized.iter_mut().zip(&incoming.initialized) {
            if *old && !new {
                *old = false;
                changed = true;
            }
        }
        if changed {
            worklist.push_back(pc);
        }
    } else {
        states.insert(pc, incoming.clone());
        worklist.push_back(pc);
    }
    Ok(())
}

/// A callback is non-escaping only when its bytecode has no path that
/// returns, embeds, or forwards it to an unverified destination. This
/// conservative analysis does not trust caller-supplied effect metadata.
/// Each direct forwarded call must itself prove the corresponding parameter.
/// Recursion is rejected until a sound SCC contract is available.
fn proves_nonescaping_callback_parameter(
    module: &BytecodeModule,
    target_id: FunctionId,
    parameter: usize,
    visiting: &mut HashSet<(u32, usize)>,
) -> bool {
    fn checked(
        module: &BytecodeModule,
        target_id: FunctionId,
        parameter: usize,
        visiting: &mut HashSet<(u32, usize)>,
    ) -> Result<(), BytecodeVerifyError> {
        let func = module
            .functions
            .get(target_id.0 as usize)
            .ok_or(BytecodeVerifyError::InvalidFunctionId(target_id))?;
        if func.is_extern
            || func.code.is_empty()
            || func.params.len() > func.locals.len()
            || !matches!(func.params.get(parameter), Some(BytecodeType::Callable(_)))
        {
            return Err(BytecodeVerifyError::Unsupported(
                "callee has no verifiable callback parameter".into(),
            ));
        }
        if !visiting.insert((target_id.0, parameter)) {
            return Err(BytecodeVerifyError::Unsupported(
                "recursive callback escape analysis requires an explicit contract".into(),
            ));
        }

        let result = (|| {
            // Bool taint tracks all values derived from this callback parameter.
            // Merge uses OR: if *any* predecessor carries it, the use must be safe.
            let mut locals = vec![false; func.locals.len()];
            locals[parameter] = true;
            let initial = (Vec::<bool>::new(), locals);
            let mut states = HashMap::new();
            let mut pending = VecDeque::new();
            states.insert(0usize, initial);
            pending.push_back(0usize);

            while let Some(pc) = pending.pop_front() {
                let (mut stack, mut locals) = states[&pc].clone();
                let instr = &func.code[pc];
                let effect = stack_effect(instr, module, func)
                    .map_err(|e| BytecodeVerifyError::Message(e.to_string()))?;
                if stack.len() < usize::from(effect.pops) {
                    return Err(BytecodeVerifyError::StackUnderflow);
                }
                match *instr {
                    BytecodeInstr::LoadLocal(id) => {
                        stack.push(
                            *locals
                                .get(id.0 as usize)
                                .ok_or(BytecodeVerifyError::InvalidLocalId(id))?,
                        );
                    }
                    BytecodeInstr::StoreLocal(id) => {
                        let value = stack.pop().ok_or(BytecodeVerifyError::StackUnderflow)?;
                        if value
                            && func
                                .inout_params
                                .get(id.0 as usize)
                                .copied()
                                .unwrap_or(false)
                        {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback escapes through an inout parameter".into(),
                            ));
                        }
                        *locals
                            .get_mut(id.0 as usize)
                            .ok_or(BytecodeVerifyError::InvalidLocalId(id))? = value;
                    }
                    BytecodeInstr::Dup => {
                        let value = *stack.last().ok_or(BytecodeVerifyError::StackUnderflow)?;
                        stack.push(value);
                    }
                    BytecodeInstr::Pop => {
                        stack.pop();
                    }
                    BytecodeInstr::CallClosure(signature_id, argc) => {
                        for _ in 0..argc {
                            if stack.pop() != Some(false) {
                                return Err(BytecodeVerifyError::Unsupported(
                                    "callback forwarded into an indirect call".into(),
                                ));
                            }
                        }
                        let callback = stack.pop().ok_or(BytecodeVerifyError::StackUnderflow)?;
                        let signature = module
                            .signatures
                            .get(signature_id.0 as usize)
                            .ok_or(BytecodeVerifyError::InvalidSignatureId(signature_id))?;
                        if callback && matches!(signature.ret, BytecodeType::Callable(_)) {
                            return Err(BytecodeVerifyError::Unsupported(
                                "borrowed callback could return another callback".into(),
                            ));
                        }
                        stack.extend(std::iter::repeat_n(false, usize::from(effect.pushes)));
                    }
                    BytecodeInstr::Call(callee, argc) => {
                        let target = module
                            .functions
                            .get(callee.0 as usize)
                            .ok_or(BytecodeVerifyError::InvalidFunctionId(callee))?;
                        if usize::from(argc) != target.params.len() {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback forwarding arity mismatch".into(),
                            ));
                        }
                        let mut forwarded = false;
                        for index in (0..usize::from(argc)).rev() {
                            let tainted = stack.pop().ok_or(BytecodeVerifyError::StackUnderflow)?;
                            if tainted {
                                forwarded = true;
                                if !checked(module, callee, index, visiting).is_ok() {
                                    return Err(BytecodeVerifyError::Unsupported(
                                        "callback forwarded to an escaping callee".into(),
                                    ));
                                }
                            }
                        }
                        if forwarded && matches!(target.ret, BytecodeType::Callable(_)) {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback might be returned by a direct callee".into(),
                            ));
                        }
                        stack.extend(std::iter::repeat_n(false, usize::from(effect.pushes)));
                    }
                    BytecodeInstr::Return => {
                        if stack.iter().any(|value| *value) {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback returned from its frame".into(),
                            ));
                        }
                    }
                    BytecodeInstr::JumpIfFalse(_) => {
                        if stack.pop() != Some(false) {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback used as a branch condition".into(),
                            ));
                        }
                    }
                    BytecodeInstr::Jump(_) | BytecodeInstr::Unreachable => {}
                    BytecodeInstr::BorrowLocal(id)
                    | BytecodeInstr::BorrowLocalMut(id)
                    | BytecodeInstr::BorrowSliceMut(id) => {
                        if *locals
                            .get(id.0 as usize)
                            .ok_or(BytecodeVerifyError::InvalidLocalId(id))?
                        {
                            return Err(BytecodeVerifyError::Unsupported(
                                "callback captured through a borrow handle".into(),
                            ));
                        }
                        for _ in 0..effect.pops {
                            if stack.pop() != Some(false) {
                                return Err(BytecodeVerifyError::Unsupported(
                                    "callback used as borrow bound".into(),
                                ));
                            }
                        }
                        stack.push(false);
                    }
                    _ => {
                        for _ in 0..effect.pops {
                            if stack.pop() != Some(false) {
                                return Err(BytecodeVerifyError::Unsupported(
                                    "callback escapes through an unsupported instruction".into(),
                                ));
                            }
                        }
                        stack.extend(std::iter::repeat_n(false, usize::from(effect.pushes)));
                    }
                }

                for successor in borrow_successors(pc, func) {
                    if successor >= func.code.len() {
                        return Err(BytecodeVerifyError::InvalidJumpTarget(successor));
                    }
                    if let Some((old_stack, old_locals)) = states.get_mut(&successor) {
                        if old_stack.len() != stack.len() || old_locals.len() != locals.len() {
                            return Err(BytecodeVerifyError::StackHeightMismatch {
                                expected: old_stack.len(),
                                found: stack.len(),
                            });
                        }
                        let mut changed = false;
                        for (old, new) in old_stack.iter_mut().zip(&stack) {
                            if !*old && *new {
                                *old = true;
                                changed = true;
                            }
                        }
                        for (old, new) in old_locals.iter_mut().zip(&locals) {
                            if !*old && *new {
                                *old = true;
                                changed = true;
                            }
                        }
                        if changed {
                            pending.push_back(successor);
                        }
                    } else {
                        states.insert(successor, (stack.clone(), locals.clone()));
                        pending.push_back(successor);
                    }
                }
            }
            Ok(())
        })();
        visiting.remove(&(target_id.0, parameter));
        result
    }

    checked(module, target_id, parameter, visiting).is_ok()
}

/// Incoming exclusive slices are not ordinary local values. Only the dedicated
/// opcodes may name their parameter slots, even in unreachable instructions.
fn verify_exclusive_slice_layout(
    module: &BytecodeModule,
    func: &BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    let exclusive =
        |local: LocalId| func.locals.get(local.0 as usize) == Some(&BytecodeType::ExclusiveSlice);
    let slice_error = || {
        BytecodeVerifyError::Unsupported(
            "exclusive slice descriptor violates direct parameter contract".into(),
        )
    };
    let is_scalar = |ty: &BytecodeType| {
        matches!(
            ty,
            BytecodeType::Bool
                | BytecodeType::Int64
                | BytecodeType::UInt64
                | BytecodeType::UInt8
                | BytecodeType::Float64
        )
    };
    if func.ret == BytecodeType::ExclusiveSlice {
        return Err(slice_error());
    }
    if func.params.contains(&BytecodeType::ExclusiveSlice) {
        if func.is_extern
            || func.code.is_empty()
            || (func.ret != BytecodeType::Unit && !is_scalar(&func.ret))
            || func
                .params
                .iter()
                .any(|ty| *ty != BytecodeType::ExclusiveSlice && !is_scalar(ty))
        {
            return Err(slice_error());
        }
    }
    for (index, ty) in func.params.iter().enumerate() {
        if *ty == BytecodeType::ExclusiveSlice
            && (!func.inout_params[index] || func.locals.get(index) != Some(ty))
        {
            return Err(slice_error());
        }
    }
    for (index, ty) in func.locals.iter().enumerate() {
        if *ty == BytecodeType::ExclusiveSlice && func.params.get(index) != Some(ty) {
            return Err(slice_error());
        }
    }
    for instr in &func.code {
        match *instr {
            BytecodeInstr::LoadLocal(id)
            | BytecodeInstr::StoreLocal(id)
            | BytecodeInstr::BorrowLocal(id)
            | BytecodeInstr::BorrowLocalMut(id)
            | BytecodeInstr::AddI64LC(id, _)
                if exclusive(id) =>
            {
                return Err(slice_error())
            }
            BytecodeInstr::AddI64LL(a, b) if exclusive(a) || exclusive(b) => {
                return Err(slice_error())
            }
            BytecodeInstr::InoutSliceGet(id)
            | BytecodeInstr::InoutSliceSet(id)
            | BytecodeInstr::InoutSliceLen(id)
                if !exclusive(id) =>
            {
                return Err(slice_error())
            }
            BytecodeInstr::MakeClosure(target, _)
            | BytecodeInstr::MakeClosureRef(target, _)
            | BytecodeInstr::MakeClosureInout(target, _) => {
                if module
                    .functions
                    .get(target.0 as usize)
                    .is_some_and(|f| f.params.contains(&BytecodeType::ExclusiveSlice))
                {
                    return Err(slice_error());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// The current callable ABI contains only public by-value arguments. Hidden
/// exclusive captures must agree with the target's direct parameter metadata.
fn verify_closure_access(
    target: &BytecodeFunction,
    hidden: usize,
    exclusive: bool,
    read_capture: bool,
) -> Result<(), BytecodeVerifyError> {
    if target.inout_params.len() != target.params.len()
        || hidden > target.params.len()
        || target
            .inout_params
            .iter()
            .enumerate()
            .any(|(index, access)| *access != (exclusive && index < hidden))
        || target.params.contains(&BytecodeType::ExclusiveSlice)
    {
        return Err(BytecodeVerifyError::Unsupported(
            "closure capture access must match target metadata; public inout is unsupported".into(),
        ));
    }
    if target.ret == BytecodeType::ReadSlice {
        return Err(BytecodeVerifyError::Unsupported(
            "indirect slice return requires a provenance contract".into(),
        ));
    }
    if read_capture
        && target.code.iter().any(|instr| {
            matches!(instr, BytecodeInstr::StoreLocal(id) | BytecodeInstr::BorrowLocalMut(id)
            | BytecodeInstr::BorrowSliceMut(id) if (id.0 as usize) < hidden)
        })
    {
        return Err(BytecodeVerifyError::Unsupported(
            "ref closure target cannot mutate or exclusively reborrow a hidden capture".into(),
        ));
    }
    Ok(())
}

fn pending_exclusive(stack: &[BorrowProvenance], local: LocalId) -> bool {
    stack.iter().any(|kind| matches!(kind, BorrowProvenance::ExclusiveHandle(id) | BorrowProvenance::ExclusiveSliceHandle(id) if *id == local))
}

fn verify_borrowed_frame_contract(
    module: &BytecodeModule,
    func: &BytecodeFunction,
) -> Result<(), BytecodeVerifyError> {
    let has_borrows = func.inout_params.iter().any(|access| *access)
        || func.code.iter().any(|instruction| match instruction {
            BytecodeInstr::BorrowLocal(_)
            | BytecodeInstr::MakeClosureRef(_, _)
            | BytecodeInstr::BorrowLocalMut(_)
            | BytecodeInstr::MakeClosureInout(_, _)
            | BytecodeInstr::BorrowSliceMut(_)
            | BytecodeInstr::InoutSliceGet(_)
            | BytecodeInstr::InoutSliceSet(_)
            | BytecodeInstr::InoutSliceLen(_) => true,
            BytecodeInstr::Call(target, _) => {
                module.functions.get(target.0 as usize).is_some_and(|f| {
                    f.inout_params.iter().any(|access| *access)
                        || f.params.contains(&BytecodeType::ExclusiveSlice)
                })
            }
            _ => false,
        });
    if !has_borrows {
        return Ok(());
    }
    if func.params.len() > func.locals.len() {
        return Err(BytecodeVerifyError::InvalidLocalId(LocalId(
            func.locals.len() as u32,
        )));
    }

    // Retain the 2B.1 single-write owner contract, but prove that this write
    // cannot execute more than once along any CFG path (including loops).
    let mut source_slots = vec![false; func.locals.len()];
    let mut writes: Vec<Vec<usize>> = vec![Vec::new(); func.locals.len()];
    for (pc, instr) in func.code.iter().enumerate() {
        match instr {
            BytecodeInstr::BorrowLocal(source)
            | BytecodeInstr::BorrowLocalMut(source)
            | BytecodeInstr::BorrowSliceMut(source) => {
                let index = source.0 as usize;
                if index >= source_slots.len() {
                    return Err(BytecodeVerifyError::InvalidLocalId(*source));
                }
                source_slots[index] = true;
            }
            BytecodeInstr::StoreLocal(local) => {
                let index = local.0 as usize;
                if index >= writes.len() {
                    return Err(BytecodeVerifyError::InvalidLocalId(*local));
                }
                writes[index].push(pc);
            }
            _ => {}
        }
    }
    // Conservative exclusivity: no read borrow and exclusive borrow of the
    // same owner in this frame; no second exclusive borrow site.
    let mut exclusive_sources = HashSet::new();
    let mut read_sources = HashSet::new();
    let mut exclusive_borrow_sites: HashMap<LocalId, Vec<usize>> = HashMap::new();
    for (pc, instruction) in func.code.iter().enumerate() {
        match instruction {
            BytecodeInstr::BorrowLocal(source) => {
                read_sources.insert(source.0);
            }
            BytecodeInstr::BorrowLocalMut(source) | BytecodeInstr::BorrowSliceMut(source) => {
                // Multiple sequential callsites may borrow the same owner.
                // Concurrent aliasing is checked by CFG handle provenance and
                // again by the runtime's active exclusive-loan table.
                exclusive_sources.insert(source.0);
                exclusive_borrow_sites.entry(*source).or_default().push(pc);
            }
            _ => {}
        }
    }
    if !exclusive_sources.is_disjoint(&read_sources) {
        return Err(BytecodeVerifyError::Unsupported(
            "ref and inout borrow the same local in a frame".into(),
        ));
    }
    for (index, is_source) in source_slots.into_iter().enumerate() {
        if !is_source {
            continue;
        }
        // An inout parameter is already an exclusive loan from its caller.
        // Reborrowing it for a direct inout call does not require a fresh
        // single-write local owner. Read borrows are not upgraded here.
        if index < func.params.len()
            && func.inout_params[index]
            && exclusive_sources.contains(&(index as u32))
            && !read_sources.contains(&(index as u32))
        {
            continue;
        }
        if index < func.params.len()
            || matches!(func.locals[index], BytecodeType::Callable(_))
            || writes[index].len() != 1
            || borrow_instruction_can_repeat(func, writes[index][0])
        {
            return Err(BytecodeVerifyError::Unsupported(format!(
                "borrowed source local ${index} requires one non-repeating initialization outside a cycle"
            )));
        }
    }

    let mut initial_init = vec![false; func.locals.len()];
    for item in initial_init.iter_mut().take(func.params.len()) {
        *item = true;
    }
    let initial = BorrowState {
        stack: Vec::new(),
        locals: vec![BorrowProvenance::Plain; func.locals.len()],
        initialized: initial_init,
    };
    let mut states = HashMap::new();
    let mut worklist = VecDeque::new();
    states.insert(0, initial);
    worklist.push_back(0);

    while let Some(pc) = worklist.pop_front() {
        let mut state = states
            .get(&pc)
            .ok_or(BytecodeVerifyError::InvalidJumpTarget(pc))?
            .clone();
        let instr = &func.code[pc];
        let effect = stack_effect(instr, module, func)
            .map_err(|err| BytecodeVerifyError::Message(err.to_string()))?;
        if state.stack.len() < usize::from(effect.pops) {
            return Err(BytecodeVerifyError::StackUnderflow);
        }

        match *instr {
            BytecodeInstr::BorrowLocal(source) => {
                let index = source.0 as usize;
                if !state.initialized[index] {
                    return Err(BytecodeVerifyError::LocalNotInitialized(source));
                }
                if state.locals[index] != BorrowProvenance::Plain {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrowed source must own a plain value".into(),
                    ));
                }
                state.stack.push(BorrowProvenance::Handle(source));
            }
            BytecodeInstr::BorrowSliceMut(source) => {
                let index = source.0 as usize;
                if !state.initialized[index]
                    || state.locals[index] != BorrowProvenance::Plain
                    || pending_exclusive(&state.stack, source)
                {
                    return Err(BytecodeVerifyError::Unsupported(
                        "invalid or duplicate exclusive slice source".into(),
                    ));
                }
                for _ in 0..2 {
                    if state.stack.pop() != Some(BorrowProvenance::Plain) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "exclusive slice bounds must be ordinary values".into(),
                        ));
                    }
                }
                state
                    .stack
                    .push(BorrowProvenance::ExclusiveSliceHandle(source));
            }
            BytecodeInstr::InoutSliceGet(source)
            | BytecodeInstr::InoutSliceSet(source)
            | BytecodeInstr::InoutSliceLen(source) => {
                if pending_exclusive(&state.stack, source) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "exclusive slice used while its subloan is pending".into(),
                    ));
                }
                for _ in 0..effect.pops {
                    if state.stack.pop() != Some(BorrowProvenance::Plain) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "exclusive slice operands must be ordinary values".into(),
                        ));
                    }
                }
                state.stack.extend(std::iter::repeat_n(
                    BorrowProvenance::Plain,
                    usize::from(effect.pushes),
                ));
            }
            BytecodeInstr::BorrowLocalMut(source) => {
                let index = source.0 as usize;
                if !state.initialized[index] || state.locals[index] != BorrowProvenance::Plain {
                    return Err(BytecodeVerifyError::Unsupported(
                        "exclusive borrowed source must be initialized and own a plain value"
                            .into(),
                    ));
                }
                if pending_exclusive(&state.stack, source) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "duplicate simultaneous exclusive borrow of the same local".into(),
                    ));
                }
                state.stack.push(BorrowProvenance::ExclusiveHandle(source));
            }
            BytecodeInstr::MakeClosureRef(target_id, count)
            | BytecodeInstr::MakeClosureInout(target_id, count) => {
                if count == 0 {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrowed closure with zero captures is invalid".into(),
                    ));
                }
                let target = module
                    .functions
                    .get(target_id.0 as usize)
                    .ok_or(BytecodeVerifyError::InvalidFunctionId(target_id))?;
                if target.is_extern || usize::from(count) > target.params.len() {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrowed closure target must be a defined function with enough parameters"
                            .into(),
                    ));
                }
                let exclusive = matches!(instr, BytecodeInstr::MakeClosureInout(_, _));
                let mut handles = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let source = match state.stack.pop() {
                        Some(BorrowProvenance::Handle(local)) if !exclusive => local,
                        Some(BorrowProvenance::ExclusiveHandle(local)) if exclusive => local,
                        _ => {
                            return Err(BytecodeVerifyError::Unsupported(
                                "borrowed closure requires handles of its declared access mode"
                                    .into(),
                            ));
                        }
                    };
                    if exclusive && handles.contains(&source) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "exclusive closure captures the same local more than once".into(),
                        ));
                    }
                    handles.push(source);
                }
                handles.reverse();
                if exclusive {
                    for source in &handles {
                        if (source.0 as usize) < func.params.len() {
                            return Err(BytecodeVerifyError::Unsupported(
                                "exclusive reborrow into a closure is not yet supported".into(),
                            ));
                        }
                        let sites = exclusive_borrow_sites.get(source).ok_or_else(|| {
                            BytecodeVerifyError::Unsupported(
                                "exclusive closure capture has no borrow site".into(),
                            )
                        })?;

                        // Direct inout calls may borrow sequentially.
                        // Exclusive closure captures require a stronger
                        // non-repeating contract until liveness is proven.
                        if sites.len() != 1 || borrow_instruction_can_repeat(func, pc) {
                            return Err(BytecodeVerifyError::Unsupported(
                                "exclusive closure capture requires one non-repeating borrow site"
                                    .into(),
                            ));
                        }
                    }
                }
                for (local, expected) in handles.iter().zip(&target.params) {
                    if func.locals[local.0 as usize] != *expected {
                        return Err(BytecodeVerifyError::StackTypeMismatch {
                            expected: *expected,
                            found: func.locals[local.0 as usize],
                        });
                    }
                }
                state.stack.push(BorrowProvenance::BorrowedClosure);
            }
            BytecodeInstr::LoadLocal(local) => {
                // A pending exclusive call operand forbids ordinary reads
                // of its source until the direct call consumes the handle.
                if pending_exclusive(&state.stack, local) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "cannot read a local while an exclusive argument is pending".into(),
                    ));
                }
                let index = local.0 as usize;
                if index >= state.locals.len() {
                    return Err(BytecodeVerifyError::InvalidLocalId(local));
                }
                if !state.initialized[index] {
                    return Err(BytecodeVerifyError::LocalNotInitialized(local));
                }
                state.stack.push(state.locals[index]);
            }
            BytecodeInstr::AddI64LL(a, b) => {
                if pending_exclusive(&state.stack, a) || pending_exclusive(&state.stack, b) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "cannot read a local while an exclusive argument is pending".into(),
                    ));
                }
                state.stack.push(BorrowProvenance::Plain);
            }
            BytecodeInstr::AddI64LC(a, _) => {
                if pending_exclusive(&state.stack, a) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "cannot read a local while an exclusive argument is pending".into(),
                    ));
                }
                state.stack.push(BorrowProvenance::Plain);
            }
            BytecodeInstr::StoreLocal(local) => {
                if pending_exclusive(&state.stack, local) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "cannot mutate a local while an exclusive argument is pending".into(),
                    ));
                }
                let index = local.0 as usize;
                if index >= state.locals.len() {
                    return Err(BytecodeVerifyError::InvalidLocalId(local));
                }
                let value = state
                    .stack
                    .pop()
                    .ok_or(BytecodeVerifyError::StackUnderflow)?;
                if matches!(
                    value,
                    BorrowProvenance::Handle(_)
                        | BorrowProvenance::ExclusiveHandle(_)
                        | BorrowProvenance::ExclusiveSliceHandle(_)
                ) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrow handle cannot be stored as an ordinary value".into(),
                    ));
                }
                if func.inout_params.get(index).copied().unwrap_or(false)
                    && value != BorrowProvenance::Plain
                {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrowed callable cannot escape through an inout parameter".into(),
                    ));
                }
                state.initialized[index] = true;
                state.locals[index] = value;
            }
            BytecodeInstr::Dup => {
                let top = *state
                    .stack
                    .last()
                    .ok_or(BytecodeVerifyError::StackUnderflow)?;
                if matches!(
                    top,
                    BorrowProvenance::Handle(_)
                        | BorrowProvenance::ExclusiveHandle(_)
                        | BorrowProvenance::ExclusiveSliceHandle(_)
                ) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "raw borrow handle cannot be duplicated".into(),
                    ));
                }
                state.stack.push(top);
            }
            BytecodeInstr::Pop => {
                if matches!(
                    state.stack.pop(),
                    Some(
                        BorrowProvenance::Handle(_)
                            | BorrowProvenance::ExclusiveHandle(_)
                            | BorrowProvenance::ExclusiveSliceHandle(_)
                    )
                ) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "raw borrow handle cannot be discarded".into(),
                    ));
                }
            }
            BytecodeInstr::CallClosure(_, argc) => {
                for _ in 0..argc {
                    if state.stack.pop() != Some(BorrowProvenance::Plain) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "borrowed callable cannot escape as a call argument".into(),
                        ));
                    }
                }
                if matches!(
                    state.stack.pop(),
                    Some(
                        BorrowProvenance::Handle(_)
                            | BorrowProvenance::ExclusiveHandle(_)
                            | BorrowProvenance::ExclusiveSliceHandle(_)
                    )
                ) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "raw borrow handle is not callable".into(),
                    ));
                }
                for _ in 0..effect.pushes {
                    state.stack.push(BorrowProvenance::Plain);
                }
            }
            BytecodeInstr::Call(target_id, argc) => {
                // Borrowed closures can cross a direct call boundary only when
                // the callee bytecode proves that parameter non-escaping. The
                // proof is independent of (and cannot trust) source metadata.
                let target = module
                    .functions
                    .get(target_id.0 as usize)
                    .ok_or(BytecodeVerifyError::InvalidFunctionId(target_id))?;
                if usize::from(argc) != target.params.len() {
                    return Err(BytecodeVerifyError::CallArityMismatch {
                        id: target_id,
                        expected: target.params.len() as u16,
                        found: argc,
                    });
                }
                for index in (0..usize::from(argc)).rev() {
                    let exclusive = target.inout_params.get(index).copied().unwrap_or(false);
                    let argument = state.stack.pop();
                    match (exclusive, argument) {
                        (true, Some(BorrowProvenance::ExclusiveHandle(_)))
                            if target.params[index] != BytecodeType::ExclusiveSlice => {}
                        (true, Some(BorrowProvenance::ExclusiveSliceHandle(_)))
                            if target.params[index] == BytecodeType::ExclusiveSlice => {}
                        (false, Some(BorrowProvenance::Plain)) => {}
                        (false, Some(BorrowProvenance::BorrowedClosure))
                            if proves_nonescaping_callback_parameter(
                                module,
                                target_id,
                                index,
                                &mut HashSet::new(),
                            ) => {}
                        _ => {
                            return Err(BytecodeVerifyError::Unsupported(format!(
                                "argument {index} violates the verified inout/non-escape contract for @{target_id}"
                            )));
                        }
                    }
                }
                for _ in 0..effect.pushes {
                    state.stack.push(BorrowProvenance::Plain);
                }
            }
            BytecodeInstr::JumpIfFalse(_) => {
                if state.stack.pop() != Some(BorrowProvenance::Plain) {
                    return Err(BytecodeVerifyError::Unsupported(
                        "branch condition cannot carry a borrowed closure or handle".into(),
                    ));
                }
            }
            BytecodeInstr::Return => {
                if state
                    .stack
                    .iter()
                    .any(|kind| *kind != BorrowProvenance::Plain)
                {
                    return Err(BytecodeVerifyError::Unsupported(
                        "borrowed closure/handle may not be returned".into(),
                    ));
                }
                for _ in 0..effect.pops {
                    state.stack.pop();
                }
            }
            BytecodeInstr::Unreachable => {
                // Trap terminates the frame; no outgoing edges exist.
            }
            _ => {
                for _ in 0..effect.pops {
                    if state.stack.pop() != Some(BorrowProvenance::Plain) {
                        return Err(BytecodeVerifyError::Unsupported(
                            "borrowed closure/handle escapes via unsupported bytecode operation"
                                .into(),
                        ));
                    }
                }
                for _ in 0..effect.pushes {
                    state.stack.push(BorrowProvenance::Plain);
                }
            }
        }

        for successor in borrow_successors(pc, func) {
            propagate_borrow_state(successor, &state, &mut states, &mut worklist, func)?;
        }
    }

    Ok(())
}

fn check_stack_binary(
    stack: &mut Vec<BytecodeType>,
    expected_operand: BytecodeType,
    result: BytecodeType,
) -> Result<(), BytecodeVerifyError> {
    let rhs = stack.pop().unwrap();
    let lhs = stack.pop().unwrap();
    if lhs != expected_operand && lhs != BytecodeType::Any && expected_operand != BytecodeType::Any
    {
        return Err(BytecodeVerifyError::StackTypeMismatch {
            expected: expected_operand,
            found: lhs,
        });
    }
    if rhs != expected_operand && rhs != BytecodeType::Any && expected_operand != BytecodeType::Any
    {
        return Err(BytecodeVerifyError::StackTypeMismatch {
            expected: expected_operand,
            found: rhs,
        });
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
        // Merge states. Revisit successors when a late edge widens stack types.
        if existing.stack.len() != state.stack.len() {
            return Err(BytecodeVerifyError::StackHeightMismatch {
                expected: existing.stack.len(),
                found: state.stack.len(),
            });
        }
        let mut changed = false;
        for (e, s) in existing.stack.iter_mut().zip(state.stack.iter()) {
            if *e != *s {
                if *e == BytecodeType::Any || *s == BytecodeType::Any {
                    if *e != BytecodeType::Any {
                        *e = BytecodeType::Any;
                        changed = true;
                    }
                } else {
                    return Err(BytecodeVerifyError::StackTypeMismatch {
                        expected: *e,
                        found: *s,
                    });
                }
            }
        }

        // Merge locals initialization (intersection)
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
