use crate::ir::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, PartialEq)]
pub enum VerifierError {
    MissingEntryBlock,
    InvalidBlockId(TirBlockId),
    UndeclaredValue(TirValueId),
    UnresolvedValueType(TirValueId),
    InvalidArithmeticType(TirType),
    InvalidComparisonType(TirType),
    StructFieldTypeConflict { struct_name: String, field: String },
    TypeMismatch { expected: TirType, actual: TirType },
    MissingReturn(TirBlockId),
    UndeclaredSlot(TirSlotId),
    InvalidTerminator,
    InvalidBranchTarget(TirBlockId),
    FunctionNotFound(TirFunctionId),
    GenericError(String),
}

pub struct VerifiedTypes {
    pub functions: HashMap<TirFunctionId, HashMap<TirValueId, TirType>>,
}

pub fn verify_module(module: &TirModule) -> Result<(), Vec<VerifierError>> {
    verify_module_with_types(module).map(|_| ())
}

pub fn verify_module_with_types(module: &TirModule) -> Result<VerifiedTypes, Vec<VerifierError>> {
    let mut functions = HashMap::new();

    for func in &module.functions {
        functions.insert(func.id, func);
    }

    // Struct field types are not carried explicitly by TIR. Infer them from
    // typed constructions and propagate until stable so field access in a
    // different function (for example an `extend` method) is still typed.
    let mut struct_fields: StructFieldTypes = HashMap::new();
    let max_passes = 128;
    for pass in 0..max_passes {
        let known_fields = struct_fields.clone();
        let mut discovered = known_fields.clone();
        let mut errors = Vec::new();
        let mut function_types = HashMap::new();

        for func in &module.functions {
            match verify_function_inner(func, &functions, &known_fields, &mut discovered) {
                Ok(value_types) => {
                    let known_types = value_types
                        .into_iter()
                        .filter_map(|(value, ty)| ty.map(|ty| (value, ty)))
                        .collect();
                    function_types.insert(func.id, known_types);
                }
                Err(mut errs) => errors.append(&mut errs),
            }
        }

        if discovered == struct_fields {
            return if errors.is_empty() {
                Ok(VerifiedTypes {
                    functions: function_types,
                })
            } else {
                Err(errors)
            };
        }
        struct_fields = discovered;

        if pass + 1 == max_passes {
            return Err(vec![VerifierError::GenericError(
                "struct field type inference did not converge".into(),
            )]);
        }
    }

    unreachable!("bounded struct field inference loop always returns")
}

pub fn verify_function(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
) -> Result<(), Vec<VerifierError>> {
    let mut discovered = HashMap::new();
    verify_function_inner(func, functions, &HashMap::new(), &mut discovered).map(|_| ())
}

type StructFieldTypes = HashMap<String, HashMap<String, TirType>>;

fn resolve_get_type(
    target: Option<&TirType>,
    key: &TirKey,
    struct_fields: &StructFieldTypes,
) -> Option<TirType> {
    let target = target?;
    match (target, key) {
        (TirType::Struct(name), TirKey::Field(field))
            if name == "$ArrayIter" || name == "$SliceIter" =>
        {
            // Compiler-internal iterator structs are instantiated at several
            // element types inside one function. Only `idx` has a stable type;
            // `data` remains intentionally unresolved rather than being
            // collapsed onto one instantiation.
            (field == "idx").then_some(TirType::Int64)
        }
        (TirType::Struct(name), TirKey::Field(field)) => {
            struct_fields.get(name)?.get(field).cloned()
        }
        (TirType::Array(inner), TirKey::Index(_)) | (TirType::Slice(inner), TirKey::Index(_)) => {
            Some((**inner).clone())
        }
        (TirType::String, TirKey::Index(_)) => Some(TirType::UInt8),
        (TirType::Tuple(elements), TirKey::Index(index)) => elements.get(*index).cloned(),
        _ => None,
    }
}

fn verify_function_inner(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
    struct_fields: &StructFieldTypes,
    discovered_struct_fields: &mut StructFieldTypes,
) -> Result<HashMap<TirValueId, Option<TirType>>, Vec<VerifierError>> {
    let mut errors = Vec::new();

    if func.blocks.is_empty() {
        // Declaration only, nothing more to verify here
        return Ok(HashMap::new());
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
                TirInstr::MakeStruct(res, name, fields) => {
                    let internal = name.starts_with('$');
                    // Start from the current pass's accumulated layout so a
                    // later construction cannot discard fields discovered by
                    // an earlier construction in the same pass. `discovered`
                    // is seeded with the previous pass's map, so this also
                    // preserves cross-pass propagation.
                    let mut inferred_fields = if internal {
                        HashMap::new()
                    } else {
                        discovered_struct_fields
                            .get(name)
                            .cloned()
                            .unwrap_or_default()
                    };

                    for (field_name, val) in fields {
                        if !value_types.contains_key(val) {
                            errors.push(VerifierError::UndeclaredValue(*val));
                            continue;
                        }
                        if internal {
                            continue;
                        }
                        let Some(Some(field_ty)) = value_types.get(val) else {
                            // An unresolved value provides no layout
                            // information. It must not create a conflict; the
                            // value is reported separately if it is used in a
                            // context that requires a type.
                            continue;
                        };

                        match inferred_fields.get(field_name) {
                            None => {
                                inferred_fields.insert(field_name.clone(), field_ty.clone());
                            }
                            Some(existing) if existing == field_ty => {}
                            Some(_) => {
                                // Keep the established type so the inferred
                                // layout cannot flip between passes. The
                                // conflict is re-detected on every pass, so it
                                // is still present in the converging pass.
                                errors.push(VerifierError::StructFieldTypeConflict {
                                    struct_name: name.clone(),
                                    field: field_name.clone(),
                                });
                            }
                        }
                    }

                    // Internal iterator structs are polymorphic over the
                    // element type, so their field layouts cannot be merged
                    // into one nominal shape.
                    if !internal {
                        discovered_struct_fields.insert(name.clone(), inferred_fields);
                    }
                    value_types.insert(*res, Some(TirType::Struct(name.clone())));
                }
                TirInstr::Get(res, target, key) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    let target_ty = value_types.get(target).and_then(|ty| ty.as_ref());
                    value_types.insert(*res, resolve_get_type(target_ty, key, struct_fields));
                }
                TirInstr::Set(res, target, _key, val) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    if !value_types.contains_key(val) {
                        errors.push(VerifierError::UndeclaredValue(*val));
                    }
                    value_types.insert(*res, value_types.get(target).cloned().flatten());
                }
                TirInstr::Tag(res, target) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    // Tag extracts the variant name, represented as a String by
                    // the current bytecode lowerer and VM. Comparisons still
                    // require identical operand types; there is no tag coercion.
                    value_types.insert(*res, Some(TirType::String));
                }
                TirInstr::Unwrap(res, target, variant, index) => {
                    if !value_types.contains_key(target) {
                        errors.push(VerifierError::UndeclaredValue(*target));
                    }
                    let target_ty = value_types.get(target).cloned().flatten();
                    let unwrapped_ty = match target_ty {
                        Some(TirType::Option(inner)) => Some(*inner),
                        Some(TirType::Result(ok, err)) => match variant.as_str() {
                            "Ok" => Some(*ok),
                            "Err" => Some(*err),
                            _ => match *index {
                                0 => Some(*ok),
                                1 => Some(*err),
                                _ => None,
                            },
                        },
                        _ => None,
                    };
                    value_types.insert(*res, unwrapped_ty);
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
                            errors.push(VerifierError::GenericError(format!(
                                "Call arg count mismatch for {:?}",
                                target_func
                            )));
                        }
                        value_types.insert(*res, Some(f.return_type.clone()));
                    } else {
                        errors.push(VerifierError::FunctionNotFound(*target_func));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::MakeClosure(res, target_func, captures) => {
                    if let Some(f) = functions.get(target_func) {
                        if captures.len() > f.params.len() {
                            errors.push(VerifierError::GenericError(format!(
                                "closure capture count exceeds target parameter count for {:?}",
                                target_func
                            )));
                            value_types.insert(*res, None);
                        } else {
                            for (capture, (_param_id, expected)) in
                                captures.iter().zip(f.params.iter())
                            {
                                match value_types.get(capture) {
                                    None => errors.push(VerifierError::UndeclaredValue(*capture)),
                                    Some(Some(actual)) if actual != expected => {
                                        errors.push(VerifierError::TypeMismatch {
                                            expected: expected.clone(),
                                            actual: actual.clone(),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            // Hidden capture parameters are excluded from the callable signature.
                            let params = f.params[captures.len()..]
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect();
                            value_types.insert(
                                *res,
                                Some(TirType::Function(params, Box::new(f.return_type.clone()))),
                            );
                        }
                    } else {
                        errors.push(VerifierError::FunctionNotFound(*target_func));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::MakeClosureRef(res, target_func, captures) => {
                    // Storage is borrowed, not copied: validate slot identities
                    // and their exact types against hidden leading parameters.
                    if captures.is_empty() {
                        errors.push(VerifierError::GenericError(
                            "make_closure_ref requires at least one borrowed slot".into(),
                        ));
                        value_types.insert(*res, None);
                    } else if let Some(f) = functions.get(target_func) {
                        if f.is_extern || f.blocks.is_empty() {
                            errors.push(VerifierError::GenericError(format!(
                                "make_closure_ref requires a defined target: {:?}",
                                target_func
                            )));
                            value_types.insert(*res, None);
                        } else if captures.len() > f.params.len() {
                            errors.push(VerifierError::GenericError(format!(
                                "borrowed closure capture count exceeds target parameters for {:?}",
                                target_func
                            )));
                            value_types.insert(*res, None);
                        } else {
                            let mut seen = std::collections::HashSet::new();
                            for (slot, (_, expected)) in captures.iter().zip(f.params.iter()) {
                                if !seen.insert(*slot) {
                                    errors.push(VerifierError::GenericError(format!(
                                        "duplicate borrowed capture slot ${}",
                                        slot.0
                                    )));
                                }
                                match slot_types.get(slot) {
                                    None => errors.push(VerifierError::UndeclaredSlot(*slot)),
                                    Some(actual) if actual != expected => {
                                        errors.push(VerifierError::TypeMismatch {
                                            expected: expected.clone(),
                                            actual: actual.clone(),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            let params = f.params[captures.len()..]
                                .iter()
                                .map(|(_, ty)| ty.clone())
                                .collect();
                            value_types.insert(
                                *res,
                                Some(TirType::Function(params, Box::new(f.return_type.clone()))),
                            );
                        }
                    } else {
                        errors.push(VerifierError::FunctionNotFound(*target_func));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::CallClosure(res, callee, args) => {
                    let callee_type = value_types.get(callee).cloned();
                    match callee_type {
                        None => {
                            errors.push(VerifierError::UndeclaredValue(*callee));
                            value_types.insert(*res, None);
                        }
                        Some(Some(TirType::Function(params, ret))) => {
                            if args.len() != params.len() {
                                errors.push(VerifierError::GenericError(format!(
                                    "closure call arity mismatch: expected {}, found {}",
                                    params.len(),
                                    args.len()
                                )));
                            }
                            for (arg, expected) in args.iter().zip(params.iter()) {
                                match value_types.get(arg) {
                                    None => errors.push(VerifierError::UndeclaredValue(*arg)),
                                    Some(Some(actual)) if actual != expected => {
                                        errors.push(VerifierError::TypeMismatch {
                                            expected: expected.clone(),
                                            actual: actual.clone(),
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            // Still diagnose undeclared extra arguments on wrong-arity calls.
                            for arg in args.iter().skip(params.len()) {
                                if !value_types.contains_key(arg) {
                                    errors.push(VerifierError::UndeclaredValue(*arg));
                                }
                            }
                            value_types.insert(*res, Some(*ret));
                        }
                        Some(Some(other)) => {
                            errors.push(VerifierError::GenericError(format!(
                                "cannot call non-function TIR value of type {:?}",
                                other
                            )));
                            value_types.insert(*res, None);
                        }
                        Some(None) => {
                            errors.push(VerifierError::GenericError(
                                "closure callee type is unresolved".to_string(),
                            ));
                            value_types.insert(*res, None);
                        }
                    }
                }
                TirInstr::Add(res, a, b)
                | TirInstr::Sub(res, a, b)
                | TirInstr::Mul(res, a, b)
                | TirInstr::Div(res, a, b) => {
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*a));
                    }
                    if b_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*b));
                    }
                    if matches!(a_ty, Some(None)) {
                        errors.push(VerifierError::UnresolvedValueType(*a));
                    }
                    if matches!(b_ty, Some(None)) {
                        errors.push(VerifierError::UnresolvedValueType(*b));
                    }

                    let mut res_ty = None;
                    if let (Some(Some(at)), Some(Some(bt))) = (a_ty, b_ty) {
                        if at != bt {
                            errors.push(VerifierError::TypeMismatch {
                                expected: at.clone(),
                                actual: bt.clone(),
                            });
                        } else if matches!(at, TirType::Int64 | TirType::UInt64 | TirType::Float64)
                        {
                            res_ty = Some(at.clone());
                        } else {
                            errors.push(VerifierError::InvalidArithmeticType(at.clone()));
                        }
                    }
                    value_types.insert(*res, res_ty);
                }
                TirInstr::Eq(res, a, b) | TirInstr::Lt(res, a, b) | TirInstr::Gt(res, a, b) => {
                    let requires_ordering =
                        matches!(instr, TirInstr::Lt(_, _, _) | TirInstr::Gt(_, _, _));
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*a));
                    }
                    if b_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*b));
                    }
                    if matches!(a_ty, Some(None)) {
                        errors.push(VerifierError::UnresolvedValueType(*a));
                    }
                    if matches!(b_ty, Some(None)) {
                        errors.push(VerifierError::UnresolvedValueType(*b));
                    }

                    if let (Some(Some(at)), Some(Some(bt))) = (a_ty, b_ty) {
                        if at != bt {
                            errors.push(VerifierError::TypeMismatch {
                                expected: at.clone(),
                                actual: bt.clone(),
                            });
                        } else if requires_ordering
                            && !matches!(at, TirType::Int64 | TirType::UInt64 | TirType::Float64)
                        {
                            errors.push(VerifierError::InvalidComparisonType(at.clone()));
                        }
                    }
                    value_types.insert(*res, Some(TirType::Bool));
                }
                TirInstr::And(res, a, b) | TirInstr::Or(res, a, b) => {
                    let a_ty = value_types.get(a);
                    let b_ty = value_types.get(b);
                    if a_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*a));
                    }
                    if b_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*b));
                    }

                    if let Some(Some(at)) = a_ty {
                        if *at != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch {
                                expected: TirType::Bool,
                                actual: at.clone(),
                            });
                        }
                    }
                    if let Some(Some(bt)) = b_ty {
                        if *bt != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch {
                                expected: TirType::Bool,
                                actual: bt.clone(),
                            });
                        }
                    }
                    value_types.insert(*res, Some(TirType::Bool));
                }
                TirInstr::Not(res, a) => {
                    let a_ty = value_types.get(a);
                    if a_ty.is_none() {
                        errors.push(VerifierError::UndeclaredValue(*a));
                    }
                    if let Some(Some(at)) = a_ty {
                        if *at != TirType::Bool {
                            errors.push(VerifierError::TypeMismatch {
                                expected: TirType::Bool,
                                actual: at.clone(),
                            });
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
                    let object_ty = value_types.get(object).cloned().flatten();
                    let result_ty = match object_ty {
                        Some(TirType::Array(inner)) | Some(TirType::Slice(inner)) => Some(*inner),
                        Some(TirType::String) => Some(TirType::UInt8),
                        Some(TirType::Tuple(elements)) => {
                            // A dynamic tuple index cannot select a single static type.
                            let _ = elements;
                            None
                        }
                        _ => None,
                    };
                    value_types.insert(*res, result_ty);
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
                    let object_ty = value_types.get(object).cloned().flatten();
                    let result_ty = match object_ty {
                        Some(TirType::Array(inner)) | Some(TirType::Slice(inner)) => {
                            Some(TirType::Slice(inner))
                        }
                        Some(TirType::String) => Some(TirType::Slice(Box::new(TirType::UInt8))),
                        _ => None,
                    };
                    value_types.insert(*res, result_ty);
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
                        errors.push(VerifierError::TypeMismatch {
                            expected: TirType::Bool,
                            actual: cond_ty.clone(),
                        });
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
                            errors.push(VerifierError::TypeMismatch {
                                expected: func.return_type.clone(),
                                actual: val_ty.clone(),
                            });
                        }
                    } else if !value_types.contains_key(val) {
                        errors.push(VerifierError::UndeclaredValue(*val));
                    }
                } else {
                    if func.return_type != TirType::Unit {
                        errors.push(VerifierError::TypeMismatch {
                            expected: func.return_type.clone(),
                            actual: TirType::Unit,
                        });
                    }
                }
            }
            Some(TirTerminator::Unreachable) => {}
            None => {
                errors.push(VerifierError::MissingReturn(block.id));
            }
        }
    }

    errors.extend(verify_borrowed_closure_frame_lifetimes(func));

    if errors.is_empty() {
        Ok(value_types)
    } else {
        Err(errors)
    }
}

// VM Parity 2B.1d: frame-bound borrowed closures.
// This is a conservative structural proof, not a general lexical borrow checker.
// A borrowed closure may move through local slots and be called in its owner
// frame, but it cannot escape through a return, an argument, or an aggregate.
fn verify_borrowed_closure_frame_lifetimes(func: &TirFunction) -> Vec<VerifierError> {
    let mut source_slots = HashSet::new();
    let mut borrowed_values = HashSet::new();
    let mut borrow_sites = Vec::new();

    for block in &func.blocks {
        for (index, instr) in block.instrs.iter().enumerate() {
            if let TirInstr::MakeClosureRef(result, _, captures) = instr {
                borrowed_values.insert(*result);
                for slot in captures {
                    source_slots.insert(*slot);
                    borrow_sites.push((block.id, index, *slot));
                }
            }
        }
    }

    if borrowed_values.is_empty() {
        return Vec::new();
    }

    let mut errors = Vec::new();
    let entry_block = func.blocks.first().map(|block| block.id);

    // For this milestone, owner storage must have one static initialization
    // in the entry block, and it must execute before any borrow in that block.
    // A single instruction in a loop/non-entry block is NOT a safe initializer.
    // This conservative restriction also excludes later mutation of a borrowed
    // slot, even when a more sophisticated analysis could prove it harmless.
    let mut ordered_sources: Vec<_> = source_slots.iter().copied().collect();
    ordered_sources.sort_by_key(|slot| slot.0);
    for source in ordered_sources {
        let mut writes = Vec::new();
        for block in &func.blocks {
            for (index, instr) in block.instrs.iter().enumerate() {
                if let TirInstr::Store(slot, _) = instr {
                    if *slot == source {
                        writes.push((block.id, index));
                    }
                }
            }
        }
        let entry_write = writes.len() == 1 && Some(writes[0].0) == entry_block;
        let initialized_before_borrow = entry_write
            && borrow_sites.iter().all(|(block, index, slot)| {
                *slot != source || *block != writes[0].0 || writes[0].1 < *index
            });
        if !initialized_before_borrow {
            errors.push(VerifierError::GenericError(format!(
                "borrowed source slot ${} requires exactly one entry-block initialization before the borrow",
                source.0
            )));
        }
    }

    // May-flow fixpoint: any local slot receiving a borrowed closure becomes
    // tainted; any subsequent load is tainted as well. Union over every Store
    // is necessary across branches and loops (never use the last Store only).
    let mut borrowed_slots = HashSet::new();
    loop {
        let mut changed = false;
        for block in &func.blocks {
            for instr in &block.instrs {
                match instr {
                    TirInstr::Store(slot, value) if borrowed_values.contains(value) => {
                        changed |= borrowed_slots.insert(*slot);
                    }
                    TirInstr::Load(value, slot) if borrowed_slots.contains(slot) => {
                        changed |= borrowed_values.insert(*value);
                    }
                    _ => {}
                }
            }
        }
        if !changed {
            break;
        }
    }

    // Scan in stable function/block/instruction order. All uses other than
    // local Store/Load and CallClosure-as-callee are forbidden until full
    // interprocedural lifetime support is available.
    for block in &func.blocks {
        for instr in &block.instrs {
            if let TirInstr::MakeClosureRef(_, _, captures) = instr {
                if captures.iter().any(|slot| borrowed_slots.contains(slot)) {
                    errors.push(VerifierError::GenericError(
                        "nested ref capture of a borrowed closure is unsupported".into(),
                    ));
                }
            }
            if instruction_escapes_borrowed_closure(instr, &borrowed_values) {
                errors.push(VerifierError::GenericError(
                    "borrowed closure escapes its owner frame or is used as a non-callee value"
                        .into(),
                ));
            }
        }
        if let Some(TirTerminator::Ret(Some(value))) = &block.terminator {
            if borrowed_values.contains(value) {
                errors.push(VerifierError::GenericError(
                    "borrowed closure escapes its owner frame through return".into(),
                ));
            }
        }
    }

    errors
}

fn instruction_escapes_borrowed_closure(instr: &TirInstr, borrowed: &HashSet<TirValueId>) -> bool {
    let contains = |value: &TirValueId| borrowed.contains(value);
    match instr {
        TirInstr::Const(_, _)
        | TirInstr::Slot(_, _)
        | TirInstr::Load(_, _)
        | TirInstr::Store(_, _)
        | TirInstr::MakeClosureRef(_, _, _) => false,
        TirInstr::CallClosure(_, _, args)
        | TirInstr::Call(_, _, args)
        | TirInstr::MakeClosure(_, _, args)
        | TirInstr::Make(_, _, _, args) => args.iter().any(contains),
        TirInstr::MakeStruct(_, _, fields) => fields.iter().any(|(_, value)| contains(value)),
        TirInstr::Get(_, object, _)
        | TirInstr::Tag(_, object)
        | TirInstr::Len(_, object)
        | TirInstr::Size(_, object)
        | TirInstr::Bytes(_, object)
        | TirInstr::Not(_, object) => contains(object),
        TirInstr::Unwrap(_, object, _, _) => contains(object),
        TirInstr::Set(_, object, _, value)
        | TirInstr::GetIndex(_, object, value)
        | TirInstr::ArrayPush(_, object, value)
        | TirInstr::Add(_, object, value)
        | TirInstr::Sub(_, object, value)
        | TirInstr::Mul(_, object, value)
        | TirInstr::Div(_, object, value)
        | TirInstr::Eq(_, object, value)
        | TirInstr::Lt(_, object, value)
        | TirInstr::Gt(_, object, value)
        | TirInstr::And(_, object, value)
        | TirInstr::Or(_, object, value) => contains(object) || contains(value),
        TirInstr::Slice(_, object, start, end) => {
            contains(object) || contains(start) || contains(end)
        }
    }
}
