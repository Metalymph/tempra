use fero_hir::*;
use crate::types::Type;
use crate::context::{TypeContext, FunctionSignature, StructInfo, EnumInfo, StructFieldInfo};
use crate::check_stmt::check_block;
use std::collections::HashMap;

/// Check a single item (function body, struct, enum, etc.)
pub fn check_item(ctx: &mut TypeContext, item: &HirItem) {
    match item {
        HirItem::Function(f) => check_function(ctx, f),
        // Structs and enums are checked during registration
        HirItem::Struct(_) | HirItem::Enum(_) | HirItem::Import(_) | HirItem::Extend(_) => {}
    }
}

/// Check a function body against its declared signature.
fn check_function(ctx: &mut TypeContext, f: &HirFunction) {
    let mut return_type = f
        .return_type
        .as_ref()
        .map(|t| ctx.resolve_hir_type(t))
        .unwrap_or(Type::Unit);

    if f.is_fallible {
        return_type = Type::Result(Box::new(return_type), Box::new(Type::BuiltinError));
    }

    ctx.current_return_type = Some(return_type);
    ctx.current_is_fallible = f.is_fallible;
    ctx.scopes.push_scope();

    // Bind parameters
    for p in &f.parameters {
        let ty = ctx.resolve_hir_type(&p.ty);
        ctx.scopes.insert(p.name.clone(), ty);
    }

    let body_ty = check_block(ctx, &f.body);
    
    // Check final expression return type
    if let Some(expected) = &ctx.current_return_type {
        if body_ty != Type::Error && body_ty != *expected {
            // Special case: implicit Ok(()) for fallible Unit functions
            let is_implicit_ok_unit = f.is_fallible 
                && matches!(f.return_type, None | Some(HirType::Named(_, _))) // Simplified check for Unit success
                && body_ty == Type::Unit
                && *expected == Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError));

            if !is_implicit_ok_unit {
                ctx.error(
                    format!("Function body return type mismatch: expected {}, found {}", expected, body_ty),
                    f.body.span,
                );
            }
        }
    }

    ctx.scopes.pop_scope();
    ctx.current_return_type = None;
    ctx.current_is_fallible = false;
}

/// Register all top-level items so that forward references work.
pub fn register_items(ctx: &mut TypeContext, items: &[HirItem]) {
    for item in items {
        match item {
            HirItem::Function(f) => {
                let params: Vec<(String, Type)> = f
                    .parameters
                    .iter()
                    .map(|p| (p.name.clone(), ctx.resolve_hir_type(&p.ty)))
                    .collect();
                let mut return_type = f
                    .return_type
                    .as_ref()
                    .map(|t| ctx.resolve_hir_type(t))
                    .unwrap_or(Type::Unit);
                if f.is_fallible {
                    return_type = Type::Result(Box::new(return_type), Box::new(Type::BuiltinError));
                }
                ctx.functions.insert(
                    f.name.clone(),
                    FunctionSignature { 
                        visibility: f.visibility,
                        params, 
                        return_type, 
                        is_fallible: f.is_fallible 
                    },
                );
            }
            HirItem::Struct(s) => {
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
                        fields,
                        field_order,
                    },
                );
            }
            HirItem::Enum(e) => {
                let mut variants = HashMap::new();
                for v in &e.variants {
                    let payload: Vec<Type> =
                        v.payload.iter().map(|t| ctx.resolve_hir_type(t)).collect();
                    variants.insert(v.name.clone(), payload);
                }
                ctx.enums.insert(
                    e.name.clone(),
                    EnumInfo {
                        visibility: e.visibility,
                        variants,
                    },
                );
            }
            HirItem::Import(_) | HirItem::Extend(_) => {}
        }
    }
}
