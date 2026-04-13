use fero_hir::*;
use crate::types::Type;
use crate::context::{TypeContext, FunctionSignature, StructInfo, EnumInfo};
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
    let return_type = f
        .return_type
        .as_ref()
        .map(|t| ctx.resolve_hir_type(t))
        .unwrap_or(Type::Unit);

    ctx.current_return_type = Some(return_type);
    ctx.scopes.push_scope();

    // Bind parameters
    for p in &f.parameters {
        let ty = ctx.resolve_hir_type(&p.ty);
        ctx.scopes.insert(p.name.clone(), ty);
    }

    let _body_ty = check_block(ctx, &f.body);
    ctx.scopes.pop_scope();
    ctx.current_return_type = None;
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
                let return_type = f
                    .return_type
                    .as_ref()
                    .map(|t| ctx.resolve_hir_type(t))
                    .unwrap_or(Type::Unit);
                ctx.functions.insert(
                    f.name.clone(),
                    FunctionSignature { params, return_type },
                );
            }
            HirItem::Struct(s) => {
                let mut fields = HashMap::new();
                for field in &s.fields {
                    fields.insert(field.name.clone(), ctx.resolve_hir_type(&field.ty));
                }
                ctx.structs.insert(s.name.clone(), StructInfo { fields });
            }
            HirItem::Enum(e) => {
                let mut variants = HashMap::new();
                for v in &e.variants {
                    let payload: Vec<Type> =
                        v.payload.iter().map(|t| ctx.resolve_hir_type(t)).collect();
                    variants.insert(v.name.clone(), payload);
                }
                ctx.enums.insert(e.name.clone(), EnumInfo { variants });
            }
            HirItem::Import(_) | HirItem::Extend(_) => {}
        }
    }
}
