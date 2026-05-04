use crate::check_stmt::check_block;
use crate::context::{EnumInfo, FunctionSignature, StructFieldInfo, StructInfo, TypeContext};
use crate::types::{types_compatible, Type};
use std::collections::HashMap;
use tempra_hir::{HirFunction, HirItem, HirType, ParameterKind};

/// Check a single item (function body, struct, enum, etc.)
pub fn check_item(ctx: &mut TypeContext, item: &mut HirItem) {
    match item {
        HirItem::Function(f) => check_function(ctx, f),
        // Structs and enums are checked during registration
        HirItem::Struct(_) | HirItem::Enum(_) | HirItem::Import(_) | HirItem::Extend(_) => {}
    }
}

/// Check a function body against its declared signature.
fn check_function(ctx: &mut TypeContext, f: &mut HirFunction) {
    let sig = ctx
        .functions
        .get(&f.name)
        .cloned()
        .expect("Function signature missing");
    ctx.current_generic_params = sig.generic_params.iter().map(|p| p.id).collect();
    ctx.current_generic_params_info = sig.generic_params.clone();
    
    let mut return_type = f
        .return_type
        .as_ref()
        .map(|t| ctx.resolve_hir_type(t))
        .unwrap_or(Type::Unit);

    if f.is_fallible && !matches!(return_type, Type::Result(_, _)) {
        return_type = Type::Result(Box::new(return_type), Box::new(Type::BuiltinError));
    }

    let is_result_return = matches!(return_type, Type::Result(_, _));
    ctx.current_return_type = Some(return_type);
    ctx.current_is_fallible = f.is_fallible || is_result_return;
    ctx.scopes.push_scope();

    // Bind parameters
    for p in &f.parameters {
        let ty = ctx.resolve_hir_type(&p.ty);
        let is_mutable = if p.kind == ParameterKind::Comptime {
            false
        } else {
            p.is_inout
        };
        ctx.scopes.insert_info(
            p.name.clone(),
            ty,
            is_mutable,
            false,
            p.kind == ParameterKind::Comptime || p.kind == ParameterKind::Type,
            true,
        );
    }

    let expected_ret = ctx.current_return_type.clone();
    let body_ty = check_block(ctx, &mut f.body, expected_ret.as_ref());

    // Check final expression return type
    if let Some(expected) = &ctx.current_return_type {
        if body_ty != Type::Error && !types_compatible(expected, &body_ty) {
            // Special case: implicit Ok(()) for fallible Unit functions
            let is_implicit_ok_unit = f.is_fallible
                && matches!(f.return_type, None | Some(HirType::Named(_, _))) // Simplified check for Unit success
                && body_ty == Type::Unit
                && *expected == Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError));

            if !is_implicit_ok_unit {
                ctx.error(
                    format!(
                        "function body return type mismatch: expected {}, found {}",
                        expected, body_ty
                    ),
                    f.body.span,
                );
            }
        }
    }

    ctx.scopes.pop_scope();
    ctx.current_return_type = None;
    ctx.current_is_fallible = false;
    ctx.current_generic_params.clear();
}

/// Register all top-level items so that forward references work.
pub fn register_items(ctx: &mut TypeContext, items: &[HirItem]) {
    for item in items {
        match item {
            HirItem::Function(f) => {
                let mut generic_params = Vec::new();
                for p in &f.generic_parameters {
                    let id = crate::types::GenericParamId(ctx.next_generic_param_id);
                    ctx.next_generic_param_id += 1;
                    generic_params.push(crate::context::GenericParam {
                        id,
                        name: p.name.clone(),
                    });
                }
                ctx.current_generic_params = generic_params.iter().map(|p| p.id).collect();
                ctx.current_generic_params_info = generic_params.clone();
                let params: Vec<crate::context::FunctionParam> = f
                    .parameters
                    .iter()
                    .map(|p| crate::context::FunctionParam {
                        name: p.name.clone(),
                        ty: ctx.resolve_hir_type(&p.ty),
                        kind: p.kind,
                    })
                    .collect();
                let mut return_type = f
                    .return_type
                    .as_ref()
                    .map(|t| ctx.resolve_hir_type(t))
                    .unwrap_or(Type::Unit);
                if f.is_fallible && !matches!(return_type, Type::Result(_, _)) {
                    return_type = Type::Result(Box::new(return_type), Box::new(Type::BuiltinError));
                }
                ctx.functions.insert(
                    f.name.clone(),
                    FunctionSignature {
                        visibility: f.visibility,
                        generic_params,
                        params,
                        return_type,
                        is_fallible: f.is_fallible,
                    },
                );
                ctx.current_generic_params.clear();
            }
            HirItem::Struct(s) => {
                let mut generic_params = Vec::new();
                for p in &s.generic_params {
                    let id = crate::types::GenericParamId(ctx.next_generic_param_id);
                    ctx.next_generic_param_id += 1;
                    generic_params.push(crate::context::GenericParam {
                        id,
                        name: p.name.clone(),
                    });
                }
                ctx.current_generic_params = generic_params.iter().map(|p| p.id).collect();
                ctx.current_generic_params_info = generic_params.clone();

                let mut fields = HashMap::new();
                let mut field_order = Vec::new();
                for field in &s.fields {
                    fields.insert(
                        field.name.clone(),
                        StructFieldInfo {
                            visibility: field.visibility,
                            ty: ctx.resolve_hir_type(&field.ty),
                            is_mutable: field.is_mutable,
                            has_default: field.default_value.is_some(),
                        },
                    );
                    field_order.push(field.name.clone());
                }
                ctx.structs.insert(
                    s.name.clone(),
                    StructInfo {
                        visibility: s.visibility,
                        generic_params,
                        fields,
                        field_order,
                    },
                );
                ctx.current_generic_params.clear();
            }
            HirItem::Enum(e) => {
                let mut generic_params = Vec::new();
                for p in &e.generic_params {
                    let id = crate::types::GenericParamId(ctx.next_generic_param_id);
                    ctx.next_generic_param_id += 1;
                    generic_params.push(crate::context::GenericParam {
                        id,
                        name: p.name.clone(),
                    });
                }
                ctx.current_generic_params = generic_params.iter().map(|p| p.id).collect();
                ctx.current_generic_params_info = generic_params.clone();

                // Pre-insert the enum to allow self-referential variants
                ctx.enums.insert(
                    e.name.clone(),
                    EnumInfo {
                        visibility: e.visibility,
                        generic_params: generic_params.clone(),
                        variants: HashMap::new(),
                    },
                );

                let mut resolved_variants = HashMap::new();
                for v in &e.variants {
                    let payload: Vec<Type> =
                        v.payload.iter().map(|t| ctx.resolve_hir_type(t)).collect();
                    resolved_variants.insert(v.name.clone(), payload);

                    // Track which enums have this variant for ambiguity detection
                    ctx.variant_to_enums
                        .entry(v.name.clone())
                        .or_default()
                        .push(e.name.clone());
                }

                // Update the enum with the resolved variants
                if let Some(info) = ctx.enums.get_mut(&e.name) {
                    info.variants = resolved_variants;
                }
                ctx.current_generic_params.clear();
            }
            HirItem::Import(_) | HirItem::Extend(_) => {}
        }
    }
}
