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

fn is_slice_scalar(ty: &TirType) -> bool {
    matches!(
        ty,
        TirType::Int64 | TirType::UInt64 | TirType::UInt8 | TirType::Float64 | TirType::Bool
    )
}

fn exclusive_slice_element(func: &TirFunction, parameter: usize) -> Option<TirType> {
    if !func.inout_params.get(parameter).copied().unwrap_or(false) {
        return None;
    }
    match &func.params.get(parameter)?.1 {
        TirType::Slice(inner) if is_slice_scalar(inner) => Some((**inner).clone()),
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

    if !func.inout_params.is_empty() && func.inout_params.len() != func.params.len() {
        return Err(vec![VerifierError::GenericError(
            "inout parameter metadata must match function arity".into(),
        )]);
    }
    if func.blocks.is_empty() {
        // Declarations cannot certify executable borrowed return contracts.
        if func.return_borrow.is_some() {
            return Err(vec![VerifierError::GenericError(
                "read slice return requires an executable body".into(),
            )]);
        }
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
                TirInstr::BindInout(_, _) => {
                    // The complete entry/slot/parameter contract is checked below.
                }
                TirInstr::Load(res, slot) | TirInstr::BorrowInout(res, slot) => {
                    if let Some(ty) = slot_types.get(slot) {
                        value_types.insert(*res, Some(ty.clone()));
                    } else {
                        errors.push(VerifierError::UndeclaredSlot(*slot));
                        value_types.insert(*res, None);
                    }
                }
                TirInstr::BorrowSliceInout(res, source, start, end) => {
                    let element = match source {
                        TirSliceSource::Owner(slot) => match slot_types.get(slot) {
                            Some(TirType::Array(inner)) if is_slice_scalar(inner) => {
                                Some((**inner).clone())
                            }
                            _ => None,
                        },
                        TirSliceSource::Parameter(index) => exclusive_slice_element(func, *index),
                    };
                    if element.is_none()
                        || [start, end]
                            .iter()
                            .any(|v| value_types.get(*v) != Some(&Some(TirType::Int64)))
                    {
                        errors.push(VerifierError::GenericError(
                            "invalid exclusive slice source or bounds".into(),
                        ));
                    }
                    value_types.insert(*res, element.map(|ty| TirType::Slice(Box::new(ty))));
                }
                TirInstr::BorrowReadArray(res, slot) => {
                    let element = match slot_types.get(slot) {
                        Some(TirType::Array(inner)) if is_slice_scalar(inner) => {
                            Some(inner.clone())
                        }
                        _ => None,
                    };
                    if element.is_none() {
                        errors.push(VerifierError::GenericError(
                            "read array borrow requires a scalar array place".into(),
                        ));
                    }
                    value_types.insert(*res, element.map(TirType::Slice));
                }
                TirInstr::ReadInoutSlice(res, parameter, start, end) => {
                    let element = exclusive_slice_element(func, *parameter);
                    if element.is_none()
                        || [start, end]
                            .iter()
                            .any(|v| value_types.get(*v) != Some(&Some(TirType::Int64)))
                    {
                        errors.push(VerifierError::GenericError(
                            "invalid read subview of exclusive slice".into(),
                        ));
                    }
                    value_types.insert(*res, element.map(|ty| TirType::Slice(Box::new(ty))));
                }
                TirInstr::InoutSliceGet(res, parameter, index) => {
                    let element = exclusive_slice_element(func, *parameter);
                    if element.is_none() || value_types.get(index) != Some(&Some(TirType::Int64)) {
                        errors.push(VerifierError::GenericError(
                            "invalid exclusive slice read".into(),
                        ));
                    }
                    value_types.insert(*res, element);
                }
                TirInstr::InoutSliceSet(parameter, index, value) => {
                    let element = exclusive_slice_element(func, *parameter);
                    if element.is_none()
                        || value_types.get(index) != Some(&Some(TirType::Int64))
                        || value_types.get(value) != Some(&element)
                    {
                        errors.push(VerifierError::GenericError(
                            "invalid exclusive slice write".into(),
                        ));
                    }
                }
                TirInstr::InoutSliceLen(res, parameter) => {
                    if exclusive_slice_element(func, *parameter).is_none() {
                        errors.push(VerifierError::GenericError(
                            "invalid exclusive slice length".into(),
                        ));
                    }
                    value_types.insert(*res, Some(TirType::Int64));
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
                            for (index, (arg, (_param_val, param_ty))) in
                                args.iter().zip(&f.params).enumerate()
                            {
                                let is_exclusive_value = func.blocks.iter().any(|block| {
                                    block.instrs.iter().any(|instr| {
                                    matches!(instr, TirInstr::BorrowInout(value, _) | TirInstr::BorrowSliceInout(value, _, _, _) if value == arg)
                                })
                                });
                                if is_exclusive_value
                                    != f.inout_params.get(index).copied().unwrap_or(false)
                                {
                                    errors.push(VerifierError::GenericError(
                                        "exclusive call argument must match inout parameter access"
                                            .into(),
                                    ));
                                }
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
                        if matches!(&f.return_type, TirType::Slice(_)) {
                            errors.push(VerifierError::GenericError(
                                "indirect slice return requires a provenance contract".into(),
                            ));
                        }
                        if f.inout_params.iter().any(|access| *access) {
                            errors.push(VerifierError::GenericError(
                                "ordinary closure target cannot have inout parameters".into(),
                            ));
                        }
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
                TirInstr::MakeClosureRef(res, target_func, captures)
                | TirInstr::MakeClosureInout(res, target_func, captures) => {
                    // Storage is borrowed, not copied: validate slot identities
                    // and their exact types against hidden leading parameters.
                    if captures.is_empty() {
                        errors.push(VerifierError::GenericError(
                            "make_closure_ref requires at least one borrowed slot".into(),
                        ));
                        value_types.insert(*res, None);
                    } else if let Some(f) = functions.get(target_func) {
                        if matches!(&f.return_type, TirType::Slice(_)) {
                            errors.push(VerifierError::GenericError(
                                "indirect slice return requires a provenance contract".into(),
                            ));
                        }
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
                            let exclusive = matches!(instr, TirInstr::MakeClosureInout(_, _, _));
                            if f.params.iter().enumerate().any(|(index, (_, ty))| {
                                let expected = exclusive && index < captures.len();
                                let actual = f.inout_params.get(index).copied().unwrap_or(false);
                                expected != actual || (actual && matches!(ty, TirType::Slice(_)))
                            }) {
                                errors.push(VerifierError::GenericError(
                                    "closure capture access must match hidden parameter modes; public inout is unsupported".into(),
                                ));
                            }
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
                            if matches!(ret.as_ref(), TirType::Slice(_)) {
                                errors.push(VerifierError::GenericError(
                                    "indirect slice return requires a provenance contract".into(),
                                ));
                            }
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

    errors.extend(verify_borrowed_closure_frame_lifetimes(func, functions));
    errors.extend(verify_exclusive_call_borrows(func, functions));
    errors.extend(verify_inout_bindings(func));
    if let Err(error) = verify_read_return_provenance(func, functions) {
        errors.push(error);
    }

    if errors.is_empty() {
        if let Err(error) = verify_tir_frame_read_loans(func, functions, &slot_types) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(value_types)
    } else {
        Err(errors)
    }
}

/// Must-provenance for the declared read-only return parameter. Unknown origins
/// never become proven at a join. This checks return provenance, not full loan
/// liveness; the latter is still enforced at source level in this tranche.
fn verify_read_return_provenance(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
) -> Result<(), VerifierError> {
    let error = || {
        VerifierError::GenericError(
            "read slice return must derive from its declared parameter on every path".into(),
        )
    };
    let Some(parameter) = func.return_borrow else {
        return if matches!(&func.return_type, TirType::Slice(_)) {
            Err(error())
        } else {
            Ok(())
        };
    };
    let Some((source, ty)) = func.params.get(parameter) else {
        return Err(error());
    };
    if !matches!(ty, TirType::Slice(_))
        || ty != &func.return_type
        || func.is_extern
        || (func.inout_params.get(parameter).copied().unwrap_or(false)
            && exclusive_slice_element(func, parameter).is_none())
    {
        return Err(error());
    }
    let entry = func.blocks.first().ok_or_else(error)?.id;

    // Storage whose address can be exposed to a writer is not a reliable
    // origin-preserving alias. This is deliberately conservative.
    let mut exposed = HashSet::new();
    for block in &func.blocks {
        for instr in &block.instrs {
            match instr {
                TirInstr::BorrowInout(_, slot) | TirInstr::BindInout(slot, _) => {
                    exposed.insert(*slot);
                }
                TirInstr::BorrowSliceInout(_, TirSliceSource::Owner(slot), _, _) => {
                    exposed.insert(*slot);
                }
                TirInstr::MakeClosureInout(_, _, slots) => exposed.extend(slots.iter().copied()),
                _ => {}
            }
        }
    }
    #[derive(Clone)]
    struct State {
        values: HashSet<TirValueId>,
        slots: HashSet<TirSlotId>,
    }
    let mut states = HashMap::from([(
        entry,
        State {
            values: HashSet::from([*source]),
            slots: HashSet::new(),
        },
    )]);
    let mut pending = std::collections::VecDeque::from([entry]);
    while let Some(id) = pending.pop_front() {
        let block = func.blocks.iter().find(|b| b.id == id).ok_or_else(error)?;
        let mut state = states[&id].clone();
        for instr in &block.instrs {
            let result = match instr {
                TirInstr::Slot(slot, _) | TirInstr::BindInout(slot, _) => {
                    state.slots.remove(slot);
                    None
                }
                TirInstr::Store(slot, value) => {
                    if state.values.contains(value) && !exposed.contains(slot) {
                        state.slots.insert(*slot);
                    } else {
                        state.slots.remove(slot);
                    }
                    None
                }
                TirInstr::Load(value, slot) => Some((*value, state.slots.contains(slot))),
                TirInstr::Slice(value, object, _, _) => {
                    Some((*value, state.values.contains(object)))
                }
                TirInstr::ReadInoutSlice(value, index, _, _) => Some((*value, *index == parameter)),
                TirInstr::BorrowSliceInout(value, TirSliceSource::Parameter(index), _, _) => {
                    Some((*value, *index == parameter))
                }
                TirInstr::Call(value, target, args) => {
                    let proven = functions
                        .get(target)
                        .and_then(|callee| {
                            if !matches!(&callee.return_type, TirType::Slice(_)) {
                                return None;
                            }
                            let parameter = callee.return_borrow?;
                            args.get(parameter)
                        })
                        .is_some_and(|arg| state.values.contains(arg));
                    Some((*value, proven))
                }
                // All other producers lose the proof, including aggregate
                // projections, bytes(), and indirect calls without contracts.
                TirInstr::Const(v, _)
                | TirInstr::Make(v, _, _, _)
                | TirInstr::MakeStruct(v, _, _)
                | TirInstr::Get(v, _, _)
                | TirInstr::Set(v, _, _, _)
                | TirInstr::Tag(v, _)
                | TirInstr::Unwrap(v, _, _, _)
                | TirInstr::BorrowInout(v, _)
                | TirInstr::BorrowReadArray(v, _)
                | TirInstr::BorrowSliceInout(v, _, _, _)
                | TirInstr::InoutSliceGet(v, _, _)
                | TirInstr::InoutSliceLen(v, _)
                | TirInstr::MakeClosure(v, _, _)
                | TirInstr::MakeClosureRef(v, _, _)
                | TirInstr::MakeClosureInout(v, _, _)
                | TirInstr::CallClosure(v, _, _)
                | TirInstr::Add(v, _, _)
                | TirInstr::Sub(v, _, _)
                | TirInstr::Mul(v, _, _)
                | TirInstr::Div(v, _, _)
                | TirInstr::Eq(v, _, _)
                | TirInstr::Lt(v, _, _)
                | TirInstr::Gt(v, _, _)
                | TirInstr::And(v, _, _)
                | TirInstr::Or(v, _, _)
                | TirInstr::Not(v, _)
                | TirInstr::GetIndex(v, _, _)
                | TirInstr::Len(v, _)
                | TirInstr::Size(v, _)
                | TirInstr::Bytes(v, _)
                | TirInstr::ArrayPush(v, _, _) => Some((*v, false)),
                TirInstr::InoutSliceSet(_, _, _) => None,
            };
            if let Some((value, proven)) = result {
                if proven {
                    state.values.insert(value);
                } else {
                    state.values.remove(&value);
                }
            }
        }
        let successors = match &block.terminator {
            Some(TirTerminator::Ret(Some(value))) if state.values.contains(value) => vec![],
            Some(TirTerminator::Ret(_)) => return Err(error()),
            Some(TirTerminator::Br(target)) => vec![*target],
            Some(TirTerminator::CondBr(_, yes, no)) => vec![*yes, *no],
            Some(TirTerminator::Unreachable) => vec![],
            None => return Err(error()),
        };
        for next in successors {
            if let Some(existing) = states.get_mut(&next) {
                let before = existing.values.len() + existing.slots.len();
                existing.values.retain(|value| state.values.contains(value));
                existing.slots.retain(|slot| state.slots.contains(slot));
                if before != existing.values.len() + existing.slots.len() {
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

/// Whole-place loans use their symbolic slot, including verified BindInout
/// aliases. Incoming slice parameters have their own parameter namespace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TirReadOrigin {
    read: HashSet<TirSliceSource>,
    exclusive: HashSet<TirSliceSource>,
}

#[derive(Clone, Copy)]
enum TirReadLocation {
    Value(TirValueId),
    Slot(TirSlotId),
}

fn read_value_def(instr: &TirInstr) -> Option<TirValueId> {
    match instr {
        TirInstr::Const(v, _)
        | TirInstr::Make(v, _, _, _)
        | TirInstr::MakeStruct(v, _, _)
        | TirInstr::Get(v, _, _)
        | TirInstr::Set(v, _, _, _)
        | TirInstr::Tag(v, _)
        | TirInstr::Unwrap(v, _, _, _)
        | TirInstr::Load(v, _)
        | TirInstr::BorrowInout(v, _)
        | TirInstr::BorrowSliceInout(v, _, _, _)
        | TirInstr::ReadInoutSlice(v, _, _, _)
        | TirInstr::BorrowReadArray(v, _)
        | TirInstr::InoutSliceGet(v, _, _)
        | TirInstr::InoutSliceLen(v, _)
        | TirInstr::Call(v, _, _)
        | TirInstr::MakeClosure(v, _, _)
        | TirInstr::MakeClosureRef(v, _, _)
        | TirInstr::MakeClosureInout(v, _, _)
        | TirInstr::CallClosure(v, _, _)
        | TirInstr::Add(v, _, _)
        | TirInstr::Sub(v, _, _)
        | TirInstr::Mul(v, _, _)
        | TirInstr::Div(v, _, _)
        | TirInstr::Eq(v, _, _)
        | TirInstr::Lt(v, _, _)
        | TirInstr::Gt(v, _, _)
        | TirInstr::And(v, _, _)
        | TirInstr::Or(v, _, _)
        | TirInstr::Not(v, _)
        | TirInstr::GetIndex(v, _, _)
        | TirInstr::Slice(v, _, _, _)
        | TirInstr::Len(v, _)
        | TirInstr::Size(v, _)
        | TirInstr::Bytes(v, _)
        | TirInstr::ArrayPush(v, _, _) => Some(*v),
        TirInstr::Slot(_, _)
        | TirInstr::BindInout(_, _)
        | TirInstr::Store(_, _)
        | TirInstr::InoutSliceSet(_, _, _) => None,
    }
}

fn read_value_used(instr: &TirInstr, value: TirValueId) -> bool {
    match instr {
        TirInstr::Const(_, _)
        | TirInstr::Slot(_, _)
        | TirInstr::BindInout(_, _)
        | TirInstr::Load(_, _)
        | TirInstr::BorrowInout(_, _)
        | TirInstr::BorrowReadArray(_, _)
        | TirInstr::MakeClosureRef(_, _, _)
        | TirInstr::MakeClosureInout(_, _, _)
        | TirInstr::InoutSliceLen(_, _) => false,
        TirInstr::Make(_, _, _, args)
        | TirInstr::MakeClosure(_, _, args)
        | TirInstr::Call(_, _, args) => args.contains(&value),
        TirInstr::CallClosure(_, target, args) => *target == value || args.contains(&value),
        TirInstr::MakeStruct(_, _, fields) => fields.iter().any(|(_, v)| *v == value),
        TirInstr::Get(_, v, _)
        | TirInstr::Tag(_, v)
        | TirInstr::Unwrap(_, v, _, _)
        | TirInstr::Store(_, v)
        | TirInstr::Not(_, v)
        | TirInstr::Len(_, v)
        | TirInstr::Size(_, v)
        | TirInstr::Bytes(_, v)
        | TirInstr::InoutSliceGet(_, _, v) => *v == value,
        TirInstr::Set(_, a, _, b)
        | TirInstr::Add(_, a, b)
        | TirInstr::Sub(_, a, b)
        | TirInstr::Mul(_, a, b)
        | TirInstr::Div(_, a, b)
        | TirInstr::Eq(_, a, b)
        | TirInstr::Lt(_, a, b)
        | TirInstr::Gt(_, a, b)
        | TirInstr::And(_, a, b)
        | TirInstr::Or(_, a, b)
        | TirInstr::GetIndex(_, a, b)
        | TirInstr::ArrayPush(_, a, b)
        | TirInstr::BorrowSliceInout(_, _, a, b)
        | TirInstr::ReadInoutSlice(_, _, a, b)
        | TirInstr::InoutSliceSet(_, a, b) => *a == value || *b == value,
        TirInstr::Slice(_, a, b, c) => *a == value || *b == value || *c == value,
    }
}

fn read_slot_used(instr: &TirInstr, slot: TirSlotId) -> bool {
    match instr {
        TirInstr::Load(_, id) | TirInstr::BorrowInout(_, id) | TirInstr::BorrowReadArray(_, id) => {
            *id == slot
        }
        TirInstr::BorrowSliceInout(_, TirSliceSource::Owner(id), _, _) => *id == slot,
        TirInstr::MakeClosureRef(_, _, captures) | TirInstr::MakeClosureInout(_, _, captures) => {
            captures.contains(&slot)
        }
        _ => false,
    }
}

fn tir_read_successors(term: Option<&TirTerminator>) -> Vec<TirBlockId> {
    match term {
        Some(TirTerminator::Br(target)) => vec![*target],
        Some(TirTerminator::CondBr(_, a, b)) => vec![*a, *b],
        _ => vec![],
    }
}

/// Last-use proof for an SSA value or slot, accounting for redefinition at a
/// loop backedge. Only normalized TIR is analyzed here.
fn tir_read_location_live(
    func: &TirFunction,
    block: TirBlockId,
    index: usize,
    location: TirReadLocation,
) -> bool {
    let mut pending = vec![(block, index)];
    let mut seen = HashSet::new();
    while let Some((id, start)) = pending.pop() {
        if !seen.insert((id, start)) {
            continue;
        }
        let Some(block) = func.blocks.iter().find(|b| b.id == id) else {
            continue;
        };
        let mut killed = false;
        for instr in block.instrs.iter().skip(start) {
            match location {
                TirReadLocation::Value(value) => {
                    if read_value_used(instr, value) {
                        return true;
                    }
                    if read_value_def(instr) == Some(value) {
                        killed = true;
                        break;
                    }
                }
                TirReadLocation::Slot(slot) => {
                    if read_slot_used(instr, slot) {
                        return true;
                    }
                    if matches!(instr, TirInstr::Store(id, _) | TirInstr::Slot(id, _) | TirInstr::BindInout(id, _) if *id == slot)
                    {
                        killed = true;
                        break;
                    }
                }
            }
        }
        if killed {
            continue;
        }
        if let TirReadLocation::Value(value) = location {
            if matches!(&block.terminator, Some(TirTerminator::Ret(Some(v))) | Some(TirTerminator::CondBr(v, _, _)) if *v == value)
            {
                return true;
            }
        }
        pending.extend(
            tir_read_successors(block.terminator.as_ref())
                .into_iter()
                .map(|next| (next, 0)),
        );
    }
    false
}

/// Verify frame read loans and incoming read parameters independently of
/// bytecode lowering. The may-analysis detects any reachable conflicting live origin;
/// the separate read-return must-analysis proves the declared return source.
fn verify_tir_frame_read_loans(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
    slot_types: &HashMap<TirSlotId, TirType>,
) -> Result<(), VerifierError> {
    if !func.params.iter().enumerate().any(|(i, (_, ty))| {
        matches!(ty, TirType::Slice(_)) && !func.inout_params.get(i).copied().unwrap_or(false)
    }) && !func
        .blocks
        .iter()
        .flat_map(|b| &b.instrs)
        .any(|instr| match instr {
            TirInstr::ReadInoutSlice(_, _, _, _) | TirInstr::BorrowReadArray(_, _) => true,
            TirInstr::Call(_, target, _) => functions.get(target).is_some_and(|target| {
                target
                    .return_borrow
                    .is_some_and(|i| exclusive_slice_element(target, i).is_some())
            }),
            _ => false,
        })
    {
        return Ok(());
    }
    let unsupported = || {
        VerifierError::GenericError(
            "frame read loan escapes through an unsupported TIR operation or ABI".into(),
        )
    };
    let conflict = || {
        VerifierError::GenericError(
            "conflicting write or exclusive borrow while a frame read loan is live".into(),
        )
    };
    let entry = func.blocks.first().ok_or_else(unsupported)?.id;
    #[derive(Clone, Default)]
    struct State {
        values: HashMap<TirValueId, TirReadOrigin>,
        slots: HashMap<TirSlotId, TirReadOrigin>,
        initialized: HashSet<TirSlotId>,
        captured_exclusive: HashSet<TirSlotId>,
    }
    fn merge<K: std::hash::Hash + Eq + Copy>(
        old: &mut HashMap<K, TirReadOrigin>,
        new: &HashMap<K, TirReadOrigin>,
    ) -> bool {
        let mut changed = false;
        for (key, value) in new {
            let entry = old.entry(*key).or_default();
            let before = entry.read.len() + entry.exclusive.len();
            entry.read.extend(value.read.iter().copied());
            entry.exclusive.extend(value.exclusive.iter().copied());
            changed |= before != entry.read.len() + entry.exclusive.len();
        }
        changed
    }
    let mut initial = State::default();
    for (index, (value, ty)) in func.params.iter().enumerate() {
        if matches!(ty, TirType::Slice(_))
            && !func.inout_params.get(index).copied().unwrap_or(false)
        {
            initial.values.insert(
                *value,
                TirReadOrigin {
                    read: HashSet::from([TirSliceSource::Parameter(index)]),
                    exclusive: HashSet::new(),
                },
            );
        }
    }
    let mut states = HashMap::from([(entry, initial)]);
    let mut pending = std::collections::VecDeque::from([entry]);
    while let Some(id) = pending.pop_front() {
        let block = func
            .blocks
            .iter()
            .find(|b| b.id == id)
            .ok_or_else(unsupported)?;
        let mut state = states[&id].clone();
        for (index, instr) in block.instrs.iter().enumerate() {
            let owner_is_read = |owner: TirSliceSource, state: &State| {
                state.values.iter().any(|(value, origin)| {
                    origin.read.contains(&owner)
                        && (read_value_used(instr, *value)
                            || tir_read_location_live(
                                func,
                                id,
                                index + 1,
                                TirReadLocation::Value(*value),
                            ))
                }) || state.slots.iter().any(|(slot, origin)| {
                    origin.read.contains(&owner)
                        && (read_slot_used(instr, *slot)
                            || tir_read_location_live(
                                func,
                                id,
                                index + 1,
                                TirReadLocation::Slot(*slot),
                            ))
                })
            };
            match instr {
                TirInstr::Store(slot, _)
                | TirInstr::Slot(slot, _)
                | TirInstr::BindInout(slot, _)
                    if owner_is_read(TirSliceSource::Owner(*slot), &state) =>
                {
                    return Err(conflict())
                }
                TirInstr::InoutSliceSet(parameter, _, _)
                    if owner_is_read(TirSliceSource::Parameter(*parameter), &state) =>
                {
                    return Err(conflict())
                }
                _ => {}
            }
            let mut result = TirReadOrigin::default();
            match instr {
                TirInstr::Slot(slot, _) => {
                    state.slots.remove(slot);
                    state.initialized.remove(slot);
                }
                TirInstr::BindInout(slot, _) => {
                    // The separate binding verifier has proved a unique entry
                    // binding to an incoming exclusive whole-place parameter.
                    // Use this same symbolic slot for reads, stores and reborrows.
                    state.slots.remove(slot);
                    state.initialized.insert(*slot);
                }
                TirInstr::Store(slot, value) => {
                    state.initialized.insert(*slot);
                    let origin = state.values.get(value).cloned().unwrap_or_default();
                    if !origin.read.is_empty()
                        && !matches!(slot_types.get(slot), Some(TirType::Slice(_)))
                    {
                        return Err(unsupported());
                    }
                    if origin == TirReadOrigin::default() {
                        state.slots.remove(slot);
                    } else {
                        state.slots.insert(*slot, origin);
                    }
                }
                TirInstr::Load(_, slot) => {
                    result = state.slots.get(slot).cloned().unwrap_or_default()
                }
                TirInstr::BorrowInout(_, slot) => {
                    if state.slots.get(slot).is_some_and(|o| !o.read.is_empty()) {
                        return Err(unsupported());
                    }
                    result.exclusive.insert(TirSliceSource::Owner(*slot));
                }
                TirInstr::BorrowSliceInout(_, source, _, _) => {
                    result.exclusive.insert(*source);
                }
                TirInstr::BorrowReadArray(_, slot) => {
                    if !state.initialized.contains(slot) {
                        return Err(unsupported());
                    }
                    let owner = TirSliceSource::Owner(*slot);
                    if state.captured_exclusive.contains(slot)
                        || state.values.iter().any(|(value, origin)| {
                            origin.exclusive.contains(&owner)
                                && tir_read_location_live(
                                    func,
                                    id,
                                    index + 1,
                                    TirReadLocation::Value(*value),
                                )
                        })
                    {
                        return Err(conflict());
                    }
                    result.read.insert(owner);
                }
                TirInstr::ReadInoutSlice(_, parameter, _, _) => {
                    result.read.insert(TirSliceSource::Parameter(*parameter));
                }
                TirInstr::Slice(_, object, start, end) => {
                    if [start, end]
                        .iter()
                        .any(|v| state.values.get(*v).is_some_and(|o| !o.read.is_empty()))
                    {
                        return Err(unsupported());
                    }
                    if let Some(origin) = state.values.get(object) {
                        result.read = origin.read.clone();
                    }
                }
                TirInstr::Call(_, target, args) => {
                    let target = functions.get(target).ok_or_else(unsupported)?;
                    for (index, value) in args.iter().enumerate() {
                        let Some(origin) = state.values.get(value) else {
                            continue;
                        };
                        if !origin.read.is_empty()
                            && (target.is_extern
                                || !matches!(
                                    target.params.get(index),
                                    Some((_, TirType::Slice(_)))
                                )
                                || target.inout_params.get(index) != Some(&false))
                        {
                            return Err(unsupported());
                        }
                        for owner in &origin.exclusive {
                            if owner_is_read(*owner, &state) {
                                return Err(conflict());
                            }
                        }
                    }
                    if let Some(source) = target
                        .return_borrow
                        .and_then(|index| args.get(index))
                        .and_then(|v| state.values.get(v))
                    {
                        result.read.extend(source.read.iter().copied());
                        result.read.extend(source.exclusive.iter().copied());
                    }
                }
                TirInstr::MakeClosureRef(_, _, slots) | TirInstr::MakeClosureInout(_, _, slots) => {
                    for slot in slots {
                        if state.slots.get(slot).is_some_and(|o| !o.read.is_empty()) {
                            return Err(unsupported());
                        }
                        if matches!(instr, TirInstr::MakeClosureInout(_, _, _)) {
                            if owner_is_read(TirSliceSource::Owner(*slot), &state) {
                                return Err(conflict());
                            }
                            state.captured_exclusive.insert(*slot);
                        }
                    }
                }
                TirInstr::GetIndex(_, _, index) => {
                    if state.values.get(index).is_some_and(|o| !o.read.is_empty()) {
                        return Err(unsupported());
                    }
                }
                TirInstr::Len(_, _) => {}
                _ => {
                    if state
                        .values
                        .iter()
                        .any(|(v, origin)| !origin.read.is_empty() && read_value_used(instr, *v))
                    {
                        return Err(unsupported());
                    }
                }
            }
            if let Some(value) = read_value_def(instr) {
                if result == TirReadOrigin::default() {
                    state.values.remove(&value);
                } else {
                    state.values.insert(value, result);
                }
            }
        }
        if let Some(TirTerminator::Ret(Some(value))) = &block.terminator {
            if state
                .values
                .get(value)
                .is_some_and(|origin| !origin.read.is_empty())
                && !matches!(&func.return_type, TirType::Slice(_))
            {
                return Err(unsupported());
            }
        }
        for next in tir_read_successors(block.terminator.as_ref()) {
            if let Some(existing) = states.get_mut(&next) {
                let changed_values = merge(&mut existing.values, &state.values);
                let changed_slots = merge(&mut existing.slots, &state.slots);
                let old_init = existing.initialized.len();
                existing
                    .initialized
                    .retain(|slot| state.initialized.contains(slot));
                let old_captures = existing.captured_exclusive.len();
                existing
                    .captured_exclusive
                    .extend(state.captured_exclusive.iter().copied());
                if changed_values
                    || changed_slots
                    || old_init != existing.initialized.len()
                    || old_captures != existing.captured_exclusive.len()
                {
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

// VM Parity 2B.1d: frame-bound borrowed closures.
// This is a conservative structural proof, not a general lexical borrow checker.
// A borrowed closure may move through local slots and cross a direct call
// only when the callee is verified not to let that callback escape.
// A Store in the entry block may execute repeatedly if the CFG returns to
// entry. Counted-once syntax is not counted-once execution (v0.4 §14.5).
fn borrowed_entry_can_repeat(func: &TirFunction, entry: TirBlockId) -> bool {
    let successors = |id: TirBlockId| -> Vec<TirBlockId> {
        let terminator = func
            .blocks
            .iter()
            .find(|block| block.id == id)
            .and_then(|block| block.terminator.as_ref());
        match terminator {
            Some(TirTerminator::Br(target)) => vec![*target],
            Some(TirTerminator::CondBr(_, yes, no)) => vec![*yes, *no],
            _ => vec![],
        }
    };

    let mut pending = successors(entry);
    let mut seen = HashSet::new();
    while let Some(block) = pending.pop() {
        if block == entry {
            return true;
        }
        if seen.insert(block) {
            pending.extend(successors(block));
        }
    }
    false
}

/// An incoming exclusive place is bound once, immediately after its slot
/// declaration in the entry block. No ordinary Store can create that alias.
fn verify_inout_bindings(func: &TirFunction) -> Vec<VerifierError> {
    let Some(entry) = func.blocks.first() else {
        return Vec::new();
    };
    let mut bound_slots = HashSet::new();
    let mut bound_parameters = HashSet::new();
    let mut errors = Vec::new();
    for block in &func.blocks {
        for (index, instr) in block.instrs.iter().enumerate() {
            let TirInstr::BindInout(slot, parameter) = instr else {
                continue;
            };
            let parameter_type = func.params.get(*parameter).map(|(_, ty)| ty);
            let adjacent = index.checked_sub(1).and_then(|i| block.instrs.get(i));
            let matches_slot = matches!(adjacent, Some(TirInstr::Slot(id, ty))
                if id == slot && Some(ty) == parameter_type);
            let declarations = func
                .blocks
                .iter()
                .flat_map(|b| &b.instrs)
                .filter(|i| matches!(i, TirInstr::Slot(id, _) if id == slot))
                .count();
            if block.id != entry.id
                || borrowed_entry_can_repeat(func, entry.id)
                || !matches_slot
                || declarations != 1
                || !func.inout_params.get(*parameter).copied().unwrap_or(false)
                || matches!(
                    parameter_type,
                    None | Some(TirType::Slice(_) | TirType::Function(_, _))
                )
                || !bound_slots.insert(*slot)
                || !bound_parameters.insert(*parameter)
            {
                errors.push(VerifierError::GenericError(
                    "invalid inout place binding: requires one matching entry slot per exclusive parameter".into(),
                ));
            }
        }
    }
    errors
}

/// Exclusive SSA operands have only one legal consumer: a direct call's
/// corresponding `inout` parameter. No assignment, return or alias is legal.
fn verify_exclusive_call_borrows(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
) -> Vec<VerifierError> {
    let borrow_sources: HashMap<TirValueId, TirSliceSource> = func
        .blocks
        .iter()
        .flat_map(|block| &block.instrs)
        .filter_map(|instr| match instr {
            TirInstr::BorrowInout(value, slot) => Some((*value, TirSliceSource::Owner(*slot))),
            TirInstr::BorrowSliceInout(value, source, _, _) => Some((*value, *source)),
            _ => None,
        })
        .collect();
    let exclusive: HashSet<TirValueId> = borrow_sources.keys().copied().collect();
    let mut uses: HashMap<TirValueId, usize> = exclusive.iter().map(|v| (*v, 0)).collect();
    let exclusive_parameters: HashSet<TirValueId> = func
        .params
        .iter()
        .enumerate()
        .filter(|(i, _)| func.inout_params.get(*i).copied().unwrap_or(false))
        .map(|(_, (value, _))| *value)
        .collect();
    let slice_parameters: HashSet<TirValueId> = func
        .params
        .iter()
        .enumerate()
        .filter(|(i, (_, ty))| {
            func.inout_params.get(*i).copied().unwrap_or(false) && matches!(ty, TirType::Slice(_))
        })
        .map(|(_, (value, _))| *value)
        .collect();
    let slice_borrows: HashSet<TirValueId> = func
        .blocks
        .iter()
        .flat_map(|block| &block.instrs)
        .filter_map(|instruction| match instruction {
            TirInstr::BorrowSliceInout(value, _, _, _) => Some(*value),
            _ => None,
        })
        .collect();
    let mut invalid = false;
    if !slice_parameters.is_empty()
        && func.params.iter().enumerate().any(|(i, (_, ty))| {
            if matches!(ty, TirType::Slice(_)) {
                exclusive_slice_element(func, i).is_none()
            } else {
                !is_slice_scalar(ty)
            }
        })
    {
        invalid = true;
    }
    if !slice_parameters.is_empty()
        && !is_slice_scalar(&func.return_type)
        && func.return_type != TirType::Unit
        && !func.return_borrow.is_some_and(|index| {
            exclusive_slice_element(func, index)
                .is_some_and(|ty| func.return_type == TirType::Slice(Box::new(ty)))
        })
    {
        invalid = true;
    }
    for block in &func.blocks {
        for instr in &block.instrs {
            let mut operands: Vec<TirValueId> = Vec::new();
            match instr {
                TirInstr::Const(_, _)
                | TirInstr::Slot(_, _)
                | TirInstr::BindInout(_, _)
                | TirInstr::Load(_, _)
                | TirInstr::BorrowInout(_, _)
                | TirInstr::BorrowReadArray(_, _)
                | TirInstr::MakeClosureRef(_, _, _)
                | TirInstr::MakeClosureInout(_, _, _)
                | TirInstr::InoutSliceLen(_, _) => {}
                TirInstr::BorrowSliceInout(_, _, start, end)
                | TirInstr::ReadInoutSlice(_, _, start, end) => operands.extend([*start, *end]),
                TirInstr::InoutSliceGet(_, _, index) => operands.push(*index),
                TirInstr::InoutSliceSet(_, index, value) => operands.extend([*index, *value]),
                TirInstr::Make(_, _, _, args) | TirInstr::MakeClosure(_, _, args) => {
                    operands.extend(args.iter().copied())
                }
                TirInstr::MakeStruct(_, _, fields) => {
                    operands.extend(fields.iter().map(|(_, value)| *value))
                }
                TirInstr::Get(_, obj, _)
                | TirInstr::Tag(_, obj)
                | TirInstr::Unwrap(_, obj, _, _)
                | TirInstr::Not(_, obj)
                | TirInstr::Len(_, obj)
                | TirInstr::Size(_, obj)
                | TirInstr::Bytes(_, obj) => operands.push(*obj),
                TirInstr::Store(_, value) => operands.push(*value),
                TirInstr::Call(_, target, args) => {
                    let callee = functions.get(target);
                    let mut borrowed_slots = HashSet::new();
                    for (index, value) in args.iter().enumerate() {
                        if exclusive.contains(value) {
                            // Two independent SSA handles to one owner may not
                            // coexist in the same direct call.
                            if !borrowed_slots.insert(borrow_sources[value]) {
                                invalid = true;
                            }
                            *uses.get_mut(value).unwrap() += 1;
                            if !callee.is_some_and(|f| {
                                !f.is_extern
                                    && f.inout_params.get(index).copied().unwrap_or(false)
                                    && (slice_borrows.contains(value)
                                        == matches!(
                                            f.params.get(index),
                                            Some((_, TirType::Slice(_)))
                                        ))
                            }) {
                                invalid = true;
                            }
                        }
                    }
                    operands.extend(args.iter().copied().filter(|v| !exclusive.contains(v)));
                }
                TirInstr::CallClosure(_, callable, args) => {
                    operands.push(*callable);
                    operands.extend(args.iter().copied());
                }
                TirInstr::Set(_, obj, _, value)
                | TirInstr::ArrayPush(_, obj, value)
                | TirInstr::GetIndex(_, obj, value)
                | TirInstr::Add(_, obj, value)
                | TirInstr::Sub(_, obj, value)
                | TirInstr::Mul(_, obj, value)
                | TirInstr::Div(_, obj, value)
                | TirInstr::Eq(_, obj, value)
                | TirInstr::Lt(_, obj, value)
                | TirInstr::Gt(_, obj, value)
                | TirInstr::And(_, obj, value)
                | TirInstr::Or(_, obj, value) => operands.extend([*obj, *value]),
                TirInstr::Slice(_, obj, start, end) => operands.extend([*obj, *start, *end]),
            }
            if operands
                .iter()
                .any(|value| exclusive.contains(value) || exclusive_parameters.contains(value))
            {
                invalid = true;
            }
        }
        match &block.terminator {
            Some(TirTerminator::Ret(Some(v))) | Some(TirTerminator::CondBr(v, _, _))
                if exclusive.contains(v) || exclusive_parameters.contains(v) =>
            {
                invalid = true
            }
            _ => {}
        }
    }
    if uses.values().any(|count| *count != 1) {
        invalid = true;
    }
    if invalid {
        vec![VerifierError::GenericError(
            "inout borrow must be consumed exactly once by a verified direct call".into(),
        )]
    } else {
        Vec::new()
    }
}

fn verify_borrowed_closure_frame_lifetimes(
    func: &TirFunction,
    functions: &HashMap<TirFunctionId, &TirFunction>,
) -> Vec<VerifierError> {
    let mut source_slots = HashSet::new();
    let mut borrowed_values = HashSet::new();
    let mut borrow_sites = Vec::new();

    for block in &func.blocks {
        for (index, instr) in block.instrs.iter().enumerate() {
            if let TirInstr::MakeClosureRef(result, _, captures)
            | TirInstr::MakeClosureInout(result, _, captures) = instr
            {
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
        let entry_write = writes.len() == 1
            && Some(writes[0].0) == entry_block
            && entry_block.is_some_and(|id| !borrowed_entry_can_repeat(func, id));
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

    // A slot bound to incoming inout storage is not a local callback sink.
    // Storing a borrowed closure into it would escape into the caller.
    if func
        .blocks
        .iter()
        .flat_map(|block| &block.instrs)
        .any(|instr| matches!(instr, TirInstr::BindInout(slot, _) if borrowed_slots.contains(slot)))
    {
        errors.push(VerifierError::GenericError(
            "borrowed closure cannot escape through an inout place".into(),
        ));
    }

    // Scan in stable function/block/instruction order. A direct callback
    // argument is permitted only after a separate interprocedural proof.
    for block in &func.blocks {
        for instr in &block.instrs {
            if let TirInstr::MakeClosureRef(_, _, captures)
            | TirInstr::MakeClosureInout(_, _, captures) = instr
            {
                if captures.iter().any(|slot| borrowed_slots.contains(slot)) {
                    errors.push(VerifierError::GenericError(
                        "nested ref capture of a borrowed closure is unsupported".into(),
                    ));
                }
            }
            if instruction_escapes_borrowed_closure(instr, &borrowed_values, functions) {
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

/// Proves that the indicated function parameter is used only during the call.
/// The analysis conservatively joins taint from all branches/iterations and
/// follows direct forwarding calls. Cycles in the call graph require an
/// explicit future contract; they are rejected rather than assumed safe.
fn proves_nonescaping_tir_callback_parameter(
    functions: &HashMap<TirFunctionId, &TirFunction>,
    function_id: TirFunctionId,
    parameter_index: usize,
    visiting: &mut HashSet<(TirFunctionId, usize)>,
) -> bool {
    let Some(function) = functions.get(&function_id).copied() else {
        return false;
    };
    if function.is_extern || function.blocks.is_empty() {
        return false;
    }
    let Some((parameter_value, TirType::Function(_, return_type))) =
        function.params.get(parameter_index)
    else {
        return false;
    };
    if !visiting.insert((function_id, parameter_index)) {
        return false;
    }

    let proven = (|| {
        // May-taint is intentionally path-insensitive: every possible use
        // must be safe. Storing into a local slot is allowed; capturing that
        // slot into a new environment or returning its value is not.
        let mut values = HashSet::from([*parameter_value]);
        let mut slots = HashSet::new();
        loop {
            let mut changed = false;
            for block in &function.blocks {
                for instruction in &block.instrs {
                    match instruction {
                        TirInstr::Store(slot, value) if values.contains(value) => {
                            changed |= slots.insert(*slot);
                        }
                        TirInstr::Load(value, slot) if slots.contains(slot) => {
                            changed |= values.insert(*value);
                        }
                        _ => {}
                    }
                }
            }
            if !changed {
                break;
            }
        }

        if function
            .blocks
            .iter()
            .flat_map(|block| &block.instrs)
            .any(|instr| matches!(instr, TirInstr::BindInout(slot, _) if slots.contains(slot)))
        {
            return false;
        }

        for block in &function.blocks {
            for instruction in &block.instrs {
                match instruction {
                    TirInstr::Call(_, target, args) => {
                        let tainted: Vec<_> = args
                            .iter()
                            .enumerate()
                            .filter_map(|(index, value)| values.contains(value).then_some(index))
                            .collect();
                        if tainted.is_empty() {
                            continue;
                        }
                        let Some(callee) = functions.get(target) else {
                            return false;
                        };
                        // A forwarded callback cannot be returned by the
                        // callee, including through a different local alias.
                        if matches!(&callee.return_type, TirType::Function(_, _)) {
                            return false;
                        }
                        for index in tainted {
                            if !proves_nonescaping_tir_callback_parameter(
                                functions, *target, index, visiting,
                            ) {
                                return false;
                            }
                        }
                    }
                    TirInstr::CallClosure(_, callee, args) => {
                        if args.iter().any(|value| values.contains(value)) {
                            return false;
                        }
                        // Invoking the callback itself is safe only when it
                        // cannot return another callback that may escape.
                        if values.contains(callee)
                            && matches!(return_type.as_ref(), TirType::Function(_, _))
                        {
                            return false;
                        }
                    }
                    TirInstr::MakeClosureRef(_, _, captures)
                    | TirInstr::MakeClosureInout(_, _, captures) => {
                        if captures.iter().any(|slot| slots.contains(slot)) {
                            return false;
                        }
                    }
                    _ => {
                        if instruction_escapes_borrowed_closure(instruction, &values, functions) {
                            return false;
                        }
                    }
                }
            }
            match &block.terminator {
                Some(TirTerminator::Ret(Some(value)))
                | Some(TirTerminator::CondBr(value, _, _))
                    if values.contains(value) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        true
    })();

    visiting.remove(&(function_id, parameter_index));
    proven
}

fn instruction_escapes_borrowed_closure(
    instr: &TirInstr,
    borrowed: &HashSet<TirValueId>,
    functions: &HashMap<TirFunctionId, &TirFunction>,
) -> bool {
    let contains = |value: &TirValueId| borrowed.contains(value);
    match instr {
        TirInstr::Const(_, _)
        | TirInstr::Slot(_, _)
        | TirInstr::BindInout(_, _)
        | TirInstr::Load(_, _)
        | TirInstr::BorrowInout(_, _)
        | TirInstr::BorrowReadArray(_, _)
        | TirInstr::Store(_, _)
        | TirInstr::MakeClosureRef(_, _, _)
        | TirInstr::MakeClosureInout(_, _, _)
        | TirInstr::InoutSliceLen(_, _) => false,
        TirInstr::BorrowSliceInout(_, _, start, end)
        | TirInstr::ReadInoutSlice(_, _, start, end) => contains(start) || contains(end),
        TirInstr::InoutSliceGet(_, _, index) => contains(index),
        TirInstr::InoutSliceSet(_, index, value) => contains(index) || contains(value),
        TirInstr::Call(_, target, args) => {
            let borrowed_args: Vec<_> = args
                .iter()
                .enumerate()
                .filter_map(|(index, value)| contains(value).then_some(index))
                .collect();
            if borrowed_args.is_empty() {
                return false;
            }
            let Some(callee) = functions.get(target) else {
                return true;
            };
            if matches!(&callee.return_type, TirType::Function(_, _)) {
                return true;
            }
            borrowed_args.into_iter().any(|index| {
                !proves_nonescaping_tir_callback_parameter(
                    functions,
                    *target,
                    index,
                    &mut HashSet::new(),
                )
            })
        }
        TirInstr::CallClosure(_, _, args)
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
