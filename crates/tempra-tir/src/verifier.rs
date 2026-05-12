use crate::ir::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq)]
pub enum VerifierError {
    MissingEntryBlock,
    InvalidBlockId(TirBlockId),
    UndeclaredValue(TirValueId),
    TypeMismatch { expected: TirType, actual: TirType },
    MissingReturn(TirBlockId),
    UndeclaredSlot(TirSlotId),
    InvalidTerminator,
    InvalidBranchTarget(TirBlockId),
    FunctionNotFound(TirFunctionId),
    GenericError(String),
}

pub fn verify_module(module: &TirModule) -> Result<(), Vec<VerifierError>> {
    let mut errors = Vec::new();
    let mut functions = HashMap::new();
    
    for func in &module.functions {
        functions.insert(func.id, func);
    }

    for func in &module.functions {
        if let Err(mut errs) = verify_function(func, &functions) {
            errors.append(&mut errs);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn verify_function(func: &TirFunction, functions: &HashMap<TirFunctionId, &TirFunction>) -> Result<(), Vec<VerifierError>> {
    let mut errors = Vec::new();

    if func.blocks.is_empty() {
        // Declaration only, nothing more to verify here
        return Ok(());
    }

    let mut declared_blocks = HashSet::new();
    for block in &func.blocks {
        declared_blocks.insert(block.id);
    }

    let mut value_types: HashMap<TirValueId, Option<TirType>> = HashMap::new();
    let mut slot_types: HashMap<TirSlotId, TirType> = HashMap::new();

    for (val_id, ty) in &func.params {
        value_types.insert(*val_id, Some(ty.clone()));
    }

    for block in &func.blocks {
        for instr in &block.instrs {
            match instr {
                TirInstr::Const(res, val) => {
                    let ty = match val {
                        TirConst::Unit => TirType::Unit,
                        TirConst::Bool(_) => TirType::Bool,
                        TirConst::Int64(_) => TirType::Int64,
                        TirConst::UInt64(_) => TirType::UInt64,
                        TirConst::Float64(_) => TirType::Float64,
                        TirConst::String(_) => TirType::String,
                    };
                    value_types.insert(*res, Some(ty));
                }
                TirInstr::Make(res, ty, _variant, args) => {
                    for arg in args {
                        if !value_types.contains_key(arg) {
                            errors.push(VerifierError::UndeclaredValue(*arg));
                        }
                    }
                    value_types.insert(*res, Some(ty.clone()));
                }
                TirInstr::MakeStruct(res, _name, fields) => {
                    for (_name, val) in fields {
                        if !value_types.contains_key(val) {
                            errors.push(VerifierError::UndeclaredValue(*val));
                        }
                    }
                    value_types.insert(*res, None); // TODO: Track actual struct type
                }
                TirInstr::Get(res, target, _key) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    // Type is unknown without full layout
                    value_types.insert(*res, None); 
                }
                TirInstr::Set(res, target, _key, val) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    if !value_types.contains_key(val) {
                        errors.push(VerifierError::UndeclaredValue(*val));
                    }
                    value_types.insert(*res, None);
                }
                TirInstr::Tag(res, target) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    value_types.insert(*res, Some(TirType::Tag));
                }
                TirInstr::Unwrap(res, target, _variant, _index) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    value_types.insert(*res, None); // Type unknown
                }
                TirInstr::Slot(slot, ty) => {
                    slot_types.insert(*slot, ty.clone());
                }
                TirInstr::Load(res, slot) => {
                    if let Some(ty) = slot_types.get(slot) {
                        value_types.insert(*res, Some(ty.clone()));
                    } else {
                        errors.push(VerifierError::UndeclaredSlot(*slot));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::Store(slot, val) => {
                    let val_ty = value_types.get(val);
                    if val_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*val));
                    }
                    let slot_ty = slot_types.get(slot);
                    if slot_ty.is_none() {
                        errors.push(VerifierError::UndeclaredSlot(*slot));
                    }

                    if let (Some(Some(v_ty)), Some(s_ty)) = (val_ty, slot_ty) {
                        if v_ty != s_ty {
                            errors.push(VerifierError::TypeMismatch {
                                expected: s_ty.clone(),
                                actual: v_ty.clone(),
                            });
                        }
                    }
                }
                TirInstr::Call(res, target_func, args) => {
                    for arg in args {
                        if !value_types.contains_key(arg) {
                            errors.push(VerifierError::UndeclaredValue(*arg));
                        }
                    }
                    if let Some(f) = functions.get(target_func) {
                        if args.len() == f.params.len() {
                            for (arg, (_param_val, param_ty)) in args.iter().zip(&f.params) {
                                if let Some(Some(a_ty)) = value_types.get(arg) {
                                    if a_ty != param_ty {
                                        errors.push(VerifierError::TypeMismatch {
                                            expected: param_ty.clone(),
                                            actual: a_ty.clone(),
                                        });
                                    }
                                }
                            }
                        } else {
                            errors.push(VerifierError::GenericError(format!("Call arg count mismatch for {:?}", target_func)));
                        }
                        value_types.insert(*res, Some(f.return_type.clone()));
                    } else {
                        errors.push(VerifierError::FunctionNotFound(*target_func));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::Add(res, a, b) | TirInstr::Sub(res, a, b) | TirInstr::Mul(res, a, b) | TirInstr::Div(res, a, b) => {
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*a)); }
                    if b_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*b)); }

                    let mut res_ty = None;
                    if let (Some(Some(at)), Some(Some(bt))) = (a_ty, b_ty) {
                        if at != bt {
                            errors.push(VerifierError::TypeMismatch { expected: at.clone(), actual: bt.clone() });
                        }
                        if *at == TirType::Int64 || *at == TirType::UInt64 || *at == TirType::Float64 {
                            res_ty = Some(at.clone());
                        } else {
                            errors.push(VerifierError::GenericError(format!("Invalid type for arithmetic: {:?}", at)));
                            res_ty = Some(at.clone());
                        }
                    } else if let Some(Some(at)) = a_ty {
                        res_ty = Some(at.clone());
                    }
                    value_types.insert(*res, res_ty);
                }
                TirInstr::Eq(res, a, b) | TirInstr::Lt(res, a, b) | TirInstr::Gt(res, a, b) => {
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*a)); }
                    if b_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*b)); }

                    if let (Some(Some(at)), Some(Some(bt))) = (a_ty, b_ty) {
                        if at != bt {
                            errors.push(VerifierError::TypeMismatch { expected: at.clone(), actual: bt.clone() });
                        }
                    }
                    value_types.insert(*res, Some(TirType::Bool));
                }
                TirInstr::And(res, a, b) | TirInstr::Or(res, a, b) => {
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*a)); }
                    if b_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*b)); }
                    
                    if let Some(Some(at)) = a_ty {
                        if *at != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch { expected: TirType::Bool, actual: at.clone() });
                        }
                    }
                    if let Some(Some(bt)) = b_ty {
                        if *bt != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch { expected: TirType::Bool, actual: bt.clone() });
                        }
                    }
                    value_types.insert(*res, Some(TirType::Bool));
                }
                TirInstr::Not(res, a) => {
                    let a_ty = value_types.get(a);
                    if a_ty.is_none() { errors.push(VerifierError::UndeclaredValue(*a)); }
                    if let Some(Some(at)) = a_ty {
                        if *at != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch { expected: TirType::Bool, actual: at.clone() });
                        }
                    }
                    value_types.insert(*res, Some(TirType::Bool));
                }
                TirInstr::GetIndex(res, object, index) => {
                    if !value_types.contains_key(object) {
                        errors.push(VerifierError::UndeclaredValue(*object));
                    }
                    if !value_types.contains_key(index) {
                        errors.push(VerifierError::UndeclaredValue(*index));
                    }
                    value_types.insert(*res, None);
                }
                TirInstr::Slice(res, object, start, end) => {
                    if !value_types.contains_key(object) {
                        errors.push(VerifierError::UndeclaredValue(*object));
                    }
                    if !value_types.contains_key(start) {
                        errors.push(VerifierError::UndeclaredValue(*start));
                    }
                    if !value_types.contains_key(end) {
                        errors.push(VerifierError::UndeclaredValue(*end));
                    }
                    value_types.insert(*res, None);
                }
                TirInstr::Len(res, object) => {
                    if !value_types.contains_key(object) {
                        errors.push(VerifierError::UndeclaredValue(*object));
                    }
                    value_types.insert(*res, Some(TirType::Int64));
                }
                TirInstr::Size(res, object) => {
                    if !value_types.contains_key(object) {
                        errors.push(VerifierError::UndeclaredValue(*object));
                    }
                    value_types.insert(*res, Some(TirType::Int64));
                }
                TirInstr::Bytes(res, object) => {
                    if !value_types.contains_key(object) {
                        errors.push(VerifierError::UndeclaredValue(*object));
                    }
                    value_types.insert(*res, Some(TirType::Slice(Box::new(TirType::UInt8))));
                }
                TirInstr::ArrayPush(res, array, value) => {
                    if !value_types.contains_key(array) {
                        errors.push(VerifierError::UndeclaredValue(*array));
                    }
                    if !value_types.contains_key(value) {
                        errors.push(VerifierError::UndeclaredValue(*value));
                    }
                    // The result has the same type as the input array
                    let array_ty = value_types.get(array).cloned().flatten();
                    value_types.insert(*res, array_ty);
                }
            }
        }

        match &block.terminator {
            Some(TirTerminator::Br(target)) => {
                if !declared_blocks.contains(target) {
                    errors.push(VerifierError::InvalidBranchTarget(*target));
                }
            }
            Some(TirTerminator::CondBr(cond, t_target, f_target)) => {
                if let Some(Some(cond_ty)) = value_types.get(cond) {
                    if *cond_ty != TirType::Bool {
                        errors.push(VerifierError::TypeMismatch { expected: TirType::Bool, actual: cond_ty.clone() });
                    }
                } else if !value_types.contains_key(cond) {
                    errors.push(VerifierError::UndeclaredValue(*cond));
                }

                if !declared_blocks.contains(t_target) {
                    errors.push(VerifierError::InvalidBranchTarget(*t_target));
                }
                if !declared_blocks.contains(f_target) {
                    errors.push(VerifierError::InvalidBranchTarget(*f_target));
                }
            }
            Some(TirTerminator::Ret(val_opt)) => {
                if let Some(val) = val_opt {
                    if let Some(Some(val_ty)) = value_types.get(val) {
                        if *val_ty != func.return_type {
                            errors.push(VerifierError::TypeMismatch { expected: func.return_type.clone(), actual: val_ty.clone() });
                        }
                    } else if !value_types.contains_key(val) {
                        errors.push(VerifierError::UndeclaredValue(*val));
                    }
                } else {
                    if func.return_type != TirType::Unit {
                        errors.push(VerifierError::TypeMismatch { expected: func.return_type.clone(), actual: TirType::Unit });
                    }
                }
            }
            Some(TirTerminator::Unreachable) => {}
            None => {
                errors.push(VerifierError::MissingReturn(block.id));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
