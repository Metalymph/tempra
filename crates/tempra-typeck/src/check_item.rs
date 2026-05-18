use crate::check_stmt::check_block;
use crate::context::{
    EnumInfo, FunctionSignature, InterfaceInfo, StructFieldInfo, StructInfo, TypeContext,
};
use crate::types::{types_compatible, Type};
use std::collections::HashMap;
use tempra_hir::{HirFunction, HirItem, HirType, ParameterKind};

/// Check a single item (function body, struct, enum, etc.)
pub fn check_item(ctx: &mut TypeContext, item: &mut HirItem) {
    match item {
        HirItem::Function(f) => check_function(ctx, f),
        // Structs and enums are checked during registration
        HirItem::Struct(_)
        | HirItem::Enum(_)
        | HirItem::Import(_)
        | HirItem::Extend(_)
        | HirItem::Interface(_) => {}
    }
}

fn verify_no_self(ctx: &mut TypeContext, ty: &HirType) {
    match ty {
        tempra_hir::HirType::SelfType(span) => {
            ctx.error(
                "'Self' is only allowed within interface requirements",
                *span,
            );
        }
        tempra_hir::HirType::Array(_, inner) => verify_no_self(ctx, inner),
        tempra_hir::HirType::Tuple(_, elems) => {
            for e in elems {
                verify_no_self(ctx, e);
            }
        }
        tempra_hir::HirType::Apply(_, callee, args) => {
            verify_no_self(ctx, callee);
            for a in args {
                verify_no_self(ctx, a);
            }
        }
        _ => {}
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

    if let Some(rt) = &f.return_type {
        verify_no_self(ctx, rt);
    }
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
        verify_no_self(ctx, &p.ty);
        let mut ty = ctx.resolve_hir_type(&p.ty);
        if p.kind == ParameterKind::Type {
            ty = Type::Meta(Box::new(Type::GenericParam {
                id: ctx
                    .current_generic_params_info
                    .iter()
                    .find(|gp| gp.name == p.name)
                    .map(|gp| gp.id)
                    .unwrap_or(crate::types::GenericParamId(0)),
                name: p.name.clone(),
                constraint: ctx
                    .current_generic_params_info
                    .iter()
                    .find(|gp| gp.name == p.name)
                    .and_then(|gp| gp.constraint.clone()),
            }));
        }
        let is_mutable = if p.kind == ParameterKind::Comptime {
            false
        } else {
            p.is_inout
        };
        let param_name_span = ctx.find_name_span(&p.name, p.span);
        ctx.facts.binding_types.insert(param_name_span, ty.clone());

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

pub fn register_items(ctx: &mut TypeContext, items: &[HirItem]) {
    // Pre-populate declaration spans for TypecheckFacts mapping
    for item in items {
        match item {
            HirItem::Struct(s) => {
                let struct_name_span = ctx.find_name_span(&s.name, s.span);
                ctx.function_spans.insert(s.name.clone(), struct_name_span);
                for f in &s.fields {
                    let name_span = ctx.find_name_span(&f.name, f.span);
                    ctx.struct_field_spans.insert((s.name.clone(), f.name.clone()), name_span);
                }
            }
            HirItem::Enum(e) => {
                let enum_name_span = ctx.find_name_span(&e.name, e.span);
                ctx.function_spans.insert(e.name.clone(), enum_name_span);
                for v in &e.variants {
                    let name_span = ctx.find_name_span(&v.name, v.span);
                    ctx.enum_variant_spans.insert((e.name.clone(), v.name.clone()), name_span);
                }
            }
            HirItem::Extend(ext) => {
                let target_ty = ctx.resolve_hir_type(&ext.target);
                let target_name = target_ty.get_name();
                for m in &ext.methods {
                    let name_span = ctx.find_name_span(&m.name, m.span);
                    ctx.method_spans.insert((target_name.clone(), m.name.clone()), name_span);
                }
            }
            HirItem::Function(f) => {
                let name_span = ctx.find_name_span(&f.name, f.span);
                ctx.function_spans.insert(f.name.clone(), name_span);
            }
            _ => {}
        }
    }

    // First pass: register names of all type items to support forward references in Extend
    for item in items {
        match item {
            HirItem::Struct(s) => {
                ctx.structs.insert(
                    s.name.clone(),
                    StructInfo {
                        visibility: s.visibility,
                        generic_params: Vec::new(),
                        fields: HashMap::new(),
                        field_order: Vec::new(),
                    },
                );
            }
            HirItem::Enum(e) => {
                ctx.enums.insert(
                    e.name.clone(),
                    EnumInfo {
                        visibility: e.visibility,
                        generic_params: Vec::new(),
                        variants: HashMap::new(),
                    },
                );
            }
            HirItem::Interface(i) => {
                ctx.interfaces.insert(
                    i.name.clone(),
                    InterfaceInfo {
                        name: i.name.clone(),
                        visibility: i.visibility,
                        methods: HashMap::new(),
                        associated_types: Vec::new(),
                    },
                );
            }
            _ => {}
        }
    }

    // Second pass: full registration
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
                        constraint: p.constraint.clone(),
                    });
                }
                ctx.current_generic_params = generic_params.iter().map(|p| p.id).collect();
                ctx.current_generic_params_info = generic_params.clone();
                let mut params = Vec::new();
                for p in &f.parameters {
                    let ty = if p.kind == tempra_ast::ParameterKind::Type {
                        // Resolve the generic param name to get the correct GenericParamId
                        // And wrap it in Meta because it's a type value parameter.
                        Type::Meta(Box::new(ctx.resolve_type_name(&p.name)))
                    } else {
                        let ty = ctx.resolve_hir_type(&p.ty);
                        ctx.verify_not_interface(&ty, p.ty.span());
                        ty
                    };
                    params.push(crate::context::FunctionParam {
                        name: p.name.clone(),
                        ty,
                        is_inout: p.is_inout,
                        kind: p.kind,
                    });
                }
                let mut return_type = f
                    .return_type
                    .as_ref()
                    .map(|t| {
                        let ty = ctx.resolve_hir_type(t);
                        ctx.verify_not_interface(&ty, t.span());
                        ty
                    })
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
                    if let Some(_) = &p.constraint {
                        ctx.error("interface constraints on struct generic parameters are not yet supported".to_string(), p.span);
                    }
                    let id = crate::types::GenericParamId(ctx.next_generic_param_id);
                    ctx.next_generic_param_id += 1;
                    generic_params.push(crate::context::GenericParam {
                        id,
                        name: p.name.clone(),
                        constraint: None,
                    });
                }
                ctx.current_generic_params = generic_params.iter().map(|p| p.id).collect();
                ctx.current_generic_params_info = generic_params.clone();

                let mut fields = HashMap::new();
                let mut field_order = Vec::new();
                for field in &s.fields {
                    let field_ty = ctx.resolve_hir_type(&field.ty);
                    ctx.verify_not_interface(&field_ty, field.span);
                    fields.insert(
                        field.name.clone(),
                        StructFieldInfo {
                            visibility: field.visibility,
                            ty: field_ty,
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
                    if let Some(_) = &p.constraint {
                        ctx.error("interface constraints on enum generic parameters are not yet supported".to_string(), p.span);
                    }
                    let id = crate::types::GenericParamId(ctx.next_generic_param_id);
                    ctx.next_generic_param_id += 1;
                    generic_params.push(crate::context::GenericParam {
                        id,
                        name: p.name.clone(),
                        constraint: None,
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
                    let mut payload = Vec::new();
                    for t in &v.payload {
                        let ty = ctx.resolve_hir_type(t);
                        ctx.verify_not_interface(&ty, v.span);
                        payload.push(ty);
                    }
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
            HirItem::Interface(i) => {
                ctx.current_interface = Some(i.name.clone());

                let associated_types: Vec<String> = i
                    .associated_types
                    .iter()
                    .map(|at| at.name.clone())
                    .collect();

                // Update interface with associated types first so resolve_hir_type can find them
                if let Some(info) = ctx.interfaces.get_mut(&i.name) {
                    info.associated_types = associated_types.clone();
                }

                let mut methods = HashMap::new();
                for m in &i.methods {
                    let mut params = Vec::new();
                    let mut param_inout = Vec::new();
                    for p in &m.parameters {
                        params.push(ctx.resolve_hir_type(&p.ty));
                        param_inout.push(p.is_inout);
                    }
                    let return_type = ctx.resolve_hir_type(&m.return_type);
                    methods.insert(
                        m.name.clone(),
                        crate::context::InterfaceMethodRequirementInfo {
                            name: m.name.clone(),
                            params,
                            param_inout,
                            return_type,
                        },
                    );
                }

                ctx.interfaces.insert(
                    i.name.clone(),
                    crate::context::InterfaceInfo {
                        name: i.name.clone(),
                        visibility: i.visibility,
                        methods,
                        associated_types,
                    },
                );
                ctx.current_interface = None;
            }
            HirItem::Extend(e) => {
                let target_ty = ctx.resolve_hir_type(&e.target);
                let mut method_names = Vec::new();
                for m in &e.methods {
                    let name = format!("{}.{}", target_ty.get_name(), m.name);

                    let mut params = Vec::new();
                    for p in &m.parameters {
                        params.push(crate::context::FunctionParam {
                            name: p.name.clone(),
                            ty: ctx.resolve_hir_type(&p.ty),
                            is_inout: p.is_inout,
                            kind: p.kind,
                        });
                    }
                    let return_type = m
                        .return_type
                        .as_ref()
                        .map(|t| ctx.resolve_hir_type(t))
                        .unwrap_or(Type::Unit);

                    ctx.functions.insert(
                        name.clone(),
                        FunctionSignature {
                            visibility: m.visibility,
                            generic_params: Vec::new(),
                            params,
                            return_type,
                            is_fallible: m.is_fallible,
                        },
                    );
                    method_names.push(name);
                }
                ctx.extends
                    .entry(target_ty)
                    .or_default()
                    .extend(method_names);
            }
            HirItem::Import(_) => {}
        }
    }
}
