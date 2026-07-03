use crate::error::TirLowerError;
use std::collections::HashMap;
use tempra_hir::Type;
use tempra_hir::*;
use tempra_span::Span;
use tempra_tir::builder::{FunctionBuilder, ModuleBuilder};
use tempra_tir::ir::*;
use tempra_typeck::context::TypeContext;

pub fn map_type(ty: &Type) -> Result<TirType, TirLowerError> {
    match ty {
        Type::Unit => Ok(TirType::Unit),
        Type::Bool => Ok(TirType::Bool),
        Type::Int | Type::Int8 | Type::Int16 | Type::Int32 | Type::Int64 => Ok(TirType::Int64),
        Type::UInt16 | Type::UInt32 | Type::UInt64 => Ok(TirType::UInt64),
        Type::UInt8 => Ok(TirType::UInt8),
        Type::Float32 | Type::Float64 => Ok(TirType::Float64),
        Type::String => Ok(TirType::String),
        Type::Never => Ok(TirType::Never),
        Type::Result(ok, err) => Ok(TirType::Result(
            Box::new(map_type(ok)?),
            Box::new(map_type(err)?),
        )),
        Type::Option(inner) => Ok(TirType::Option(Box::new(map_type(inner)?))),
        Type::Enum(name, args) => {
            if name == "Option" && args.len() == 1 {
                Ok(TirType::Option(Box::new(map_type(&args[0])?)))
            } else if name == "Result" && args.len() == 2 {
                Ok(TirType::Result(
                    Box::new(map_type(&args[0])?),
                    Box::new(map_type(&args[1])?),
                ))
            } else {
                Err(TirLowerError::UnsupportedType(format!("Enum({})", name)))
            }
        }
        Type::Unknown => Err(TirLowerError::UnsupportedType("Unknown".to_string())),
        Type::Error => Err(TirLowerError::UnsupportedType("Error".to_string())),

        Type::Array(inner) => Ok(TirType::Array(Box::new(map_type(inner)?))),
        Type::Slice(inner) => Ok(TirType::Slice(Box::new(map_type(inner)?))),
        Type::Tuple(_) => Err(TirLowerError::UnsupportedType("Tuple".to_string())),
        Type::Struct(name, _) => Ok(TirType::Struct(name.clone())),
        Type::Command => Err(TirLowerError::UnsupportedType("Command".to_string())),
        Type::CommandResult => Err(TirLowerError::UnsupportedType("CommandResult".to_string())),
        Type::Regex => Err(TirLowerError::UnsupportedType("Regex".to_string())),
        Type::SelfType => Err(TirLowerError::UnsupportedType("Self".to_string())),
        Type::Associated(_, _) => Err(TirLowerError::UnsupportedType("Associated".to_string())),
        Type::BuiltinError => Ok(TirType::String),
        Type::Namespace(_) => Err(TirLowerError::UnsupportedType("Namespace".to_string())),
        Type::Function(_, _) => Err(TirLowerError::UnsupportedType("Function".to_string())),
        Type::GenericParam { .. } => {
            Err(TirLowerError::UnsupportedType("GenericParam".to_string()))
        }
        Type::Meta(_) => Err(TirLowerError::UnsupportedType("Meta".to_string())),
        Type::Projection { .. } => Err(TirLowerError::UnsupportedType("Projection".to_string())),
        Type::Interface(_) => Err(TirLowerError::UnsupportedType("Interface".to_string())),
        Type::ArrayIter(_) => Ok(TirType::Struct("$ArrayIter".to_string())),
        Type::SliceIter(_) => Ok(TirType::Struct("$SliceIter".to_string())),
    }
}

pub fn unmap_type(tir_ty: &TirType) -> Result<Type, TirLowerError> {
    match tir_ty {
        TirType::Unit => Ok(Type::Unit),
        TirType::Bool => Ok(Type::Bool),
        TirType::Int64 => Ok(Type::Int64),
        TirType::UInt64 => Ok(Type::UInt64),
        TirType::UInt8 => Ok(Type::UInt8),
        TirType::Float64 => Ok(Type::Float64),
        TirType::String => Ok(Type::String),
        TirType::Never => Ok(Type::Never),
        TirType::Option(inner) => Ok(Type::Option(Box::new(unmap_type(inner)?))),
        TirType::Result(ok, err) => Ok(Type::Result(
            Box::new(unmap_type(ok)?),
            Box::new(unmap_type(err)?),
        )),
        TirType::Tag => Ok(Type::String),
        TirType::Array(inner) => Ok(Type::Array(Box::new(unmap_type(inner)?))),
        TirType::Slice(inner) => Ok(Type::Slice(Box::new(unmap_type(inner)?))),
        TirType::Struct(name) => Ok(Type::Struct(name.clone(), vec![])),
        _ => Err(TirLowerError::UnsupportedType(format!("{:?}", tir_ty))),
    }
}

fn sanitize_specialization_part(part: &str) -> String {
    part.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect()
}

pub fn specialized_function_name(base: &str, type_args: &[Type]) -> String {
    if type_args.is_empty() {
        return base.to_string();
    }

    let suffix = type_args
        .iter()
        .map(|ty| sanitize_specialization_part(&ty.to_string()))
        .collect::<Vec<_>>()
        .join("$");
    format!("{}${}", base, suffix)
}

fn infer_generic_type_args(
    pattern: &Type,
    actual: &Type,
    inferred: &mut HashMap<tempra_hir::types::GenericParamId, Type>,
) {
    match (pattern, actual) {
        (Type::GenericParam { id, .. }, ty) => {
            inferred.entry(*id).or_insert_with(|| ty.clone());
        }
        (Type::Array(p), Type::Array(a))
        | (Type::Slice(p), Type::Slice(a))
        | (Type::Option(p), Type::Option(a))
        | (Type::ArrayIter(p), Type::ArrayIter(a))
        | (Type::SliceIter(p), Type::SliceIter(a)) => infer_generic_type_args(p, a, inferred),
        (Type::Result(p_ok, p_err), Type::Result(a_ok, a_err)) => {
            infer_generic_type_args(p_ok, a_ok, inferred);
            infer_generic_type_args(p_err, a_err, inferred);
        }
        (Type::Tuple(p), Type::Tuple(a)) if p.len() == a.len() => {
            for (p_ty, a_ty) in p.iter().zip(a.iter()) {
                infer_generic_type_args(p_ty, a_ty, inferred);
            }
        }
        (Type::Struct(p_name, p_args), Type::Struct(a_name, a_args))
        | (Type::Enum(p_name, p_args), Type::Enum(a_name, a_args))
            if p_name == a_name && p_args.len() == a_args.len() =>
        {
            for (p_ty, a_ty) in p_args.iter().zip(a_args.iter()) {
                infer_generic_type_args(p_ty, a_ty, inferred);
            }
        }
        _ => {}
    }
}

/// Resolve an HirType to a semantic Type. Minimal — only handles what Phase 14.4 needs.
fn resolve_hir_type(
    hir_ty: &HirType,
    _type_ctx: &TypeContext,
    self_ty: Option<&Type>,
    generic_subs: &HashMap<String, Type>,
) -> Result<Type, TirLowerError> {
    match hir_ty {
        HirType::Named(_, name) => {
            if let Some(ty) = generic_subs.get(name) {
                return Ok(ty.clone());
            }
            let ty = tempra_typeck::types::resolve_named_type(name);
            if ty == Type::Unknown {
                if let Some(info) = _type_ctx.structs.get(name) {
                    Ok(Type::Struct(
                        name.clone(),
                        info.generic_params
                            .iter()
                            .map(|p| Type::GenericParam {
                                id: p.id,
                                name: p.name.clone(),
                                constraint: p.constraint.clone(),
                            })
                            .collect(),
                    ))
                } else if let Some(info) = _type_ctx.enums.get(name) {
                    Ok(Type::Enum(
                        name.clone(),
                        info.generic_params
                            .iter()
                            .map(|p| Type::GenericParam {
                                id: p.id,
                                name: p.name.clone(),
                                constraint: p.constraint.clone(),
                            })
                            .collect(),
                    ))
                } else {
                    Err(TirLowerError::UnsupportedType(format!(
                        "Unknown type name: {}",
                        name
                    )))
                }
            } else {
                Ok(ty)
            }
        }
        HirType::Result(_, ok_hir, err_hir) => {
            let ok_ty = resolve_hir_type(ok_hir, _type_ctx, self_ty, generic_subs)?;
            let err_ty = resolve_hir_type(err_hir, _type_ctx, self_ty, generic_subs)?;
            Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
        }
        HirType::Optional(_, inner_hir) => {
            let inner_ty = resolve_hir_type(inner_hir, _type_ctx, self_ty, generic_subs)?;
            Ok(Type::Option(Box::new(inner_ty)))
        }
        HirType::Array(_, inner_hir) => {
            let inner_ty = resolve_hir_type(inner_hir, _type_ctx, self_ty, generic_subs)?;
            Ok(Type::Array(Box::new(inner_ty)))
        }
        HirType::Apply(_, callee, args) => {
            if let HirType::Named(_, name) = &**callee {
                if name == "Slice" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty, generic_subs)?;
                    return Ok(Type::Slice(Box::new(inner_ty)));
                }
                if name == "Array" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty, generic_subs)?;
                    return Ok(Type::Array(Box::new(inner_ty)));
                }
                if name == "Option" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty, generic_subs)?;
                    return Ok(Type::Option(Box::new(inner_ty)));
                }
                if name == "Result" && args.len() == 2 {
                    let ok_ty = resolve_hir_type(&args[0], _type_ctx, self_ty, generic_subs)?;
                    let err_ty = resolve_hir_type(&args[1], _type_ctx, self_ty, generic_subs)?;
                    return Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)));
                }
            }
            Err(TirLowerError::UnsupportedType(format!(
                "Cannot resolve HirType::Apply: {:?}",
                hir_ty
            )))
        }
        HirType::SelfType(_) => {
            if let Some(sty) = self_ty {
                Ok(sty.clone())
            } else {
                Err(TirLowerError::UnsupportedType(
                    "Self type used outside of method".to_string(),
                ))
            }
        }
        _ => Err(TirLowerError::UnsupportedType(format!(
            "Cannot resolve HirType: {:?}",
            hir_ty
        ))),
    }
}

pub struct LowerContext<'a> {
    pub builder: FunctionBuilder,
    pub type_ctx: &'a TypeContext,
    pub module_builder: &'a mut ModuleBuilder,
    pub self_ty: Option<Type>,
    pub generic_subs: HashMap<String, Type>,
    /// (SlotId, Type, is_mutable)
    pub locals: HashMap<String, (TirSlotId, TirType, bool)>,
    pub local_hir_types: HashMap<String, Type>,
}

impl<'a> LowerContext<'a> {
    pub fn new(
        builder: FunctionBuilder,
        type_ctx: &'a TypeContext,
        module_builder: &'a mut ModuleBuilder,
        self_ty: Option<Type>,
        generic_subs: HashMap<String, Type>,
    ) -> Self {
        Self {
            builder,
            type_ctx,
            module_builder,
            self_ty,
            generic_subs,
            locals: HashMap::new(),
            local_hir_types: HashMap::new(),
        }
    }

    fn collect_locals_from_statement(&mut self, stmt: &HirStatement) -> Result<(), TirLowerError> {
        match stmt {
            HirStatement::Let(l) => {
                let ty = if let Some(ht) = &l.ty {
                    resolve_hir_type(ht, self.type_ctx, self.self_ty.as_ref(), &self.generic_subs)?
                } else {
                    self.infer_expr_type(&l.value)?
                };
                let tir_ty = map_type(&ty)?;
                self.locals
                    .insert(l.name.clone(), (TirSlotId(0), tir_ty, false));
                self.local_hir_types.insert(l.name.clone(), ty);
            }
            HirStatement::Var(v) => {
                let ty = if let Some(ht) = &v.ty {
                    resolve_hir_type(ht, self.type_ctx, self.self_ty.as_ref(), &self.generic_subs)?
                } else {
                    self.infer_expr_type(&v.value)?
                };
                let tir_ty = map_type(&ty)?;
                self.locals
                    .insert(v.name.clone(), (TirSlotId(0), tir_ty, true));
                self.local_hir_types.insert(v.name.clone(), ty);
            }
            _ => {}
        }
        Ok(())
    }

    fn infer_block_type(&mut self, block: &HirBlock) -> Result<Type, TirLowerError> {
        let old_locals = self.locals.clone();
        for stmt in &block.statements {
            self.collect_locals_from_statement(stmt)?;
        }

        let res = if let Some(HirStatement::Expr(expr)) = block.statements.last() {
            self.infer_expr_type(expr)
        } else {
            Ok(Type::Unit)
        };

        self.locals = old_locals;
        res
    }

    fn infer_expr_type(&mut self, expr: &HirExpression) -> Result<Type, TirLowerError> {
        match expr {
            HirExpression::Integer(_, _) => Ok(Type::Int64),
            HirExpression::Bool(_, _) => Ok(Type::Bool),
            HirExpression::String(_, _) => Ok(Type::String),
            HirExpression::Name(span, name) => {
                if name == "None" {
                    return Ok(Type::Option(Box::new(Type::Unknown)));
                }
                if let Some(info) = self.type_ctx.scopes.lookup(name) {
                    Ok(info.ty.clone())
                } else if let Some(hir_ty) = self.local_hir_types.get(name) {
                    Ok(hir_ty.clone())
                } else if let Some((_, tir_ty, _)) = self.locals.get(name) {
                    unmap_type(tir_ty)
                } else if self.type_ctx.is_module(name) {
                    Ok(Type::Namespace(name.clone()))
                } else {
                    Err(TirLowerError::UnknownLocal(name.clone(), *span))
                }
            }
            HirExpression::Binary(b) => match b.op {
                HirBinaryOp::Add | HirBinaryOp::Sub | HirBinaryOp::Mul | HirBinaryOp::Div => {
                    self.infer_expr_type(&b.left)
                }
                HirBinaryOp::Eq
                | HirBinaryOp::NotEq
                | HirBinaryOp::Less
                | HirBinaryOp::LessEq
                | HirBinaryOp::Greater
                | HirBinaryOp::GreaterEq
                | HirBinaryOp::And
                | HirBinaryOp::Or => Ok(Type::Bool),
                _ => Err(TirLowerError::UnsupportedExpr(
                    "binary op type".into(),
                    b.span,
                )),
            },
            HirExpression::Unary(u) => {
                if u.op == HirUnaryOp::Not {
                    Ok(Type::Bool)
                } else {
                    self.infer_expr_type(&u.expr)
                }
            }
            HirExpression::Array(_, elements) => {
                if let Some(first) = elements.first() {
                    let inner = self.infer_expr_type(first)?;
                    Ok(Type::Array(Box::new(inner)))
                } else {
                    Ok(Type::Array(Box::new(Type::Unknown)))
                }
            }
            HirExpression::Index(i) => {
                let obj_ty = self.infer_expr_type(&i.object)?;
                match obj_ty {
                    Type::Array(inner) | Type::Slice(inner) => Ok(*inner),
                    _ => Err(TirLowerError::UnsupportedExpr(
                        "Indexing non-array/slice".into(),
                        i.span,
                    )),
                }
            }
            HirExpression::Slice(s) => {
                let obj_ty = self.infer_expr_type(&s.object)?;
                match obj_ty {
                    Type::Array(inner) | Type::Slice(inner) => Ok(Type::Slice(inner)),
                    _ => Err(TirLowerError::UnsupportedExpr(
                        "Slicing non-array/slice".into(),
                        s.span,
                    )),
                }
            }
            HirExpression::Member(m) => {
                let obj_ty = self.infer_expr_type(&m.object)?;
                if let Type::Namespace(ns) = &obj_ty {
                    let qualified = format!("{}.{}", ns, m.field);
                    if self.type_ctx.is_module(&qualified) {
                        return Ok(Type::Namespace(qualified));
                    }
                }
                if m.field == "len" {
                    if matches!(obj_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
                        return Ok(Type::Int64);
                    }
                } else if m.field == "bytes" {
                    if matches!(obj_ty, Type::String) {
                        return Ok(Type::Slice(Box::new(Type::UInt8)));
                    }
                }

                // If it's a struct field access
                if let Type::Struct(name, _) = &obj_ty {
                    if let Some(info) = self.type_ctx.structs.get(name) {
                        if let Some(field_info) = info.fields.get(&m.field) {
                            return Ok(field_info.ty.clone());
                        }
                    }
                }
                Err(TirLowerError::UnsupportedExpr(
                    format!("Member access: {}", m.field),
                    m.span,
                ))
            }
            HirExpression::StructLiteral(s) => {
                let ty = resolve_hir_type(
                    &s.ty,
                    self.type_ctx,
                    self.self_ty.as_ref(),
                    &self.generic_subs,
                )?;
                Ok(ty)
            }
            HirExpression::Call(c) => {
                if let HirExpression::Member(m) = &*c.callee {
                    if m.field == "len" || m.field == "size" {
                        return Ok(Type::Int64);
                    }
                    if m.field == "count" {
                        return Ok(Type::Int);
                    }
                    if m.field == "bytes" {
                        return Ok(Type::Slice(Box::new(Type::UInt8)));
                    }
                    if m.field == "is_empty" {
                        return Ok(Type::Bool);
                    }

                    let obj_ty = self.infer_expr_type(&m.object)?;
                    if m.field == "first" || m.field == "last" {
                        if let Type::Array(inner) | Type::Slice(inner) = &obj_ty {
                            return Ok(Type::Option(inner.clone()));
                        }
                    }
                    if m.field == "iter" {
                        if let Type::Array(inner) = &obj_ty {
                            return Ok(Type::ArrayIter(inner.clone()));
                        } else if let Type::Slice(inner) = &obj_ty {
                            return Ok(Type::SliceIter(inner.clone()));
                        } else if let Type::String = &obj_ty {
                            return Ok(Type::SliceIter(Box::new(Type::UInt8)));
                        }
                    }
                    if m.field == "next" {
                        if let Type::ArrayIter(inner) | Type::SliceIter(inner) = &obj_ty {
                            return Ok(Type::Option(inner.clone()));
                        }
                    }
                    if let Some(elem_ty) = self.collection_element_type(&obj_ty) {
                        match m.field.as_str() {
                            "map" => {
                                let callback_name = self.resolve_collection_callback(
                                    &m.field,
                                    &c.args,
                                    c.span,
                                    0,
                                    std::slice::from_ref(&elem_ty),
                                )?;
                                let sig = self.type_ctx.functions.get(&callback_name).ok_or_else(
                                    || {
                                        TirLowerError::UnknownFunction(
                                            callback_name.clone(),
                                            c.span,
                                        )
                                    },
                                )?;
                                return Ok(Type::Array(Box::new(sig.return_type.clone())));
                            }
                            "filter" => return Ok(Type::Array(Box::new(elem_ty))),
                            "find" => return Ok(Type::Option(Box::new(elem_ty))),
                            "any" | "all" => return Ok(Type::Bool),
                            "for_each" => return Ok(Type::Unit),
                            "reduce" => return Ok(Type::Option(Box::new(elem_ty))),
                            "fold" => {
                                if let Some(HirCallArgument::Positional(init)) = c.args.first() {
                                    return self.infer_expr_type(init);
                                }
                            }
                            _ => {}
                        }
                    }
                    if let Type::Namespace(ns) = &obj_ty {
                        let qualified = format!("{}.{}", ns, m.field);
                        if let Some(sig) = self.type_ctx.functions.get(&qualified) {
                            return Ok(sig.return_type.clone());
                        }
                    }
                    if let Some(sig) = self
                        .type_ctx
                        .lookup_direct_method_signature(&obj_ty, &m.field)
                    {
                        return Ok(sig.return_type.clone());
                    }
                    Err(TirLowerError::UnsupportedExpr(
                        format!("Cannot infer call type for method: {}", m.field),
                        c.span,
                    ))
                } else if let HirExpression::Name(_, name) = &*c.callee {
                    match name.as_str() {
                        "Some" => {
                            if let Some(arg) = c.args.first() {
                                let arg_expr = match arg {
                                    HirCallArgument::Positional(e) => e,
                                    HirCallArgument::Named(n) => &n.value,
                                };
                                let inner = self.infer_expr_type(arg_expr)?;
                                return Ok(Type::Option(Box::new(inner)));
                            }
                            Ok(Type::Unknown)
                        }
                        "Ok" | "Err" => Ok(Type::Unknown),
                        _ => {
                            if let Some(sig) = self.type_ctx.functions.get(name.as_str()) {
                                Ok(sig.return_type.clone())
                            } else {
                                Err(TirLowerError::UnsupportedExpr(
                                    format!("Cannot infer call type for {}", name),
                                    c.span,
                                ))
                            }
                        }
                    }
                } else {
                    Err(TirLowerError::UnsupportedExpr(
                        format!("Cannot infer call type for complex callee: {:?}", c.callee),
                        c.span,
                    ))
                }
            }
            HirExpression::Try(e) => {
                let inner_ty = self.infer_expr_type(&e.expr)?;
                match inner_ty {
                    Type::Result(ok, _) => Ok(*ok),
                    _ => Err(TirLowerError::UnsupportedExpr(
                        "'?' operator expects a Result type".into(),
                        e.span,
                    )),
                }
            }
            HirExpression::IsPattern(_) => Ok(Type::Bool),
            HirExpression::Block(b) => self.infer_block_type(b),
            HirExpression::For(f) => {
                let iterable_ty = self.infer_expr_type(&f.iterable)?;
                let element_ty = match iterable_ty {
                    Type::Array(inner) | Type::Slice(inner) => *inner,
                    _ => Type::Unknown,
                };
                let tir_element_ty = map_type(&element_ty)?;
                let old_locals = self.locals.clone();
                self.locals
                    .insert(f.binding.clone(), (TirSlotId(0), tir_element_ty, false));
                let body_ty = self.infer_block_type(&f.body)?;
                self.locals = old_locals;
                Ok(Type::Array(Box::new(body_ty)))
            }

            _ => Err(TirLowerError::UnsupportedExpr(
                format!("{:?}", expr),
                expr.span(),
            )),
        }
    }

    pub fn lower_statement(&mut self, stmt: &HirStatement) -> Result<(), TirLowerError> {
        match stmt {
            HirStatement::Expr(expr) => {
                self.lower_expression(expr)?;
                Ok(())
            }
            HirStatement::Let(l) => {
                // Use explicit type if available, otherwise infer
                let val_ty = if let Some(hir_ty) = &l.ty {
                    resolve_hir_type(
                        hir_ty,
                        self.type_ctx,
                        self.self_ty.as_ref(),
                        &self.generic_subs,
                    )?
                } else {
                    self.infer_expr_type(&l.value)?
                };
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&l.name))?;
                self.locals
                    .insert(l.name.clone(), (slot, tir_ty.clone(), false));
                self.local_hir_types.insert(l.name.clone(), val_ty);

                let val = match &l.value {
                    HirExpression::Name(_, name) if name == "None" => {
                        self.builder
                            .build_make(tir_ty.clone(), Some("None".to_string()), vec![])?
                    }
                    HirExpression::Array(_, elements) if elements.is_empty() => {
                        self.builder.build_make(tir_ty.clone(), None, vec![])?
                    }
                    _ => self.lower_expression(&l.value)?,
                };
                self.builder.build_store(slot, val)?;
                Ok(())
            }
            HirStatement::Var(v) => {
                let val_ty = if let Some(hir_ty) = &v.ty {
                    resolve_hir_type(
                        hir_ty,
                        self.type_ctx,
                        self.self_ty.as_ref(),
                        &self.generic_subs,
                    )?
                } else {
                    self.infer_expr_type(&v.value)?
                };
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&v.name))?;
                self.locals
                    .insert(v.name.clone(), (slot, tir_ty.clone(), true));
                self.local_hir_types.insert(v.name.clone(), val_ty);

                let val = match &v.value {
                    HirExpression::Name(_, name) if name == "None" => {
                        self.builder
                            .build_make(tir_ty.clone(), Some("None".to_string()), vec![])?
                    }
                    HirExpression::Array(_, elements) if elements.is_empty() => {
                        self.builder.build_make(tir_ty.clone(), None, vec![])?
                    }
                    _ => self.lower_expression(&v.value)?,
                };
                self.builder.build_store(slot, val)?;
                Ok(())
            }
            HirStatement::Return(Some(expr)) => {
                let val = self.lower_expression(expr)?;
                self.builder.build_ret(Some(val))?;
                Ok(())
            }
            HirStatement::Return(None) => {
                self.builder.build_ret(None)?;
                Ok(())
            }
            HirStatement::For(f) => self.lower_for(f),
            _ => Err(TirLowerError::UnsupportedStmt(
                format!("{:?}", stmt),
                stmt.span(),
            )),
        }
    }

    fn lower_for(&mut self, f: &HirFor) -> Result<(), TirLowerError> {
        let iterable_ty = self.infer_expr_type(&f.iterable)?;
        if !matches!(iterable_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
            return Err(TirLowerError::UnsupportedExpr(
                "for-in currently only supports Array/Slice/String in experimental VM".into(),
                f.iterable.span(),
            ));
        }

        let element_ty = match &iterable_ty {
            Type::Array(inner) | Type::Slice(inner) => inner.as_ref().clone(),
            Type::String => Type::UInt8,
            _ => {
                return Err(TirLowerError::UnsupportedExpr(
                    format!(
                        "for-in currently only supports Array/Slice/String in experimental VM, found {}",
                        iterable_ty
                    ),
                    f.iterable.span(),
                ))
            }
        };

        // 1. Lower iterable and create iterator struct { data, idx: 0 }
        let iter_val = self.lower_expression(&f.iterable)?;
        // For String, desugar to bytes() first
        let data_val = if matches!(iterable_ty, Type::String) {
            self.builder.build_bytes(iter_val)?
        } else {
            iter_val
        };
        let zero = self.builder.build_const(TirConst::Int64(0))?;
        let struct_name = if matches!(iterable_ty, Type::Array(_)) {
            "$ArrayIter"
        } else {
            "$SliceIter"
        };
        let iter_struct = self.builder.build_make_struct(
            struct_name.to_string(),
            vec![("data".to_string(), data_val), ("idx".to_string(), zero)],
        )?;

        let tir_iter_ty = TirType::Struct(struct_name.to_string());
        let iter_slot = self
            .builder
            .build_slot(tir_iter_ty, Some(&format!("$it_{}", f.span.start)))?;
        self.builder.build_store(iter_slot, iter_struct)?;

        // 2. Create blocks
        let cond_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        self.builder.build_br(cond_block)?;

        // 3. Condition block — inline next() logic
        self.builder.set_current_block(cond_block);
        let loaded_it = self.builder.build_load(iter_slot)?;
        let data_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("data".to_string()))?;
        let idx_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("idx".to_string()))?;
        let len_val = self.builder.build_len(data_val)?;
        let is_less = self.builder.build_lt(idx_val, len_val)?;
        self.builder.build_condbr(is_less, body_block, exit_block)?;

        // 4. Body block
        self.builder.set_current_block(body_block);

        // Extract element at current idx
        let element_val = self.builder.build_get_index(data_val, idx_val)?;

        // Scope for loop variable
        let old_locals = self.locals.clone();

        let tir_element_ty = map_type(&element_ty)?;
        let x_slot = self
            .builder
            .build_slot(tir_element_ty.clone(), Some(&f.binding))?;
        self.builder.build_store(x_slot, element_val)?;
        self.locals
            .insert(f.binding.clone(), (x_slot, tir_element_ty, false));

        // Lower body statements
        for stmt in &f.body.statements {
            self.lower_statement(stmt)?;
        }

        // Restore locals scope
        self.locals = old_locals;

        // Increment idx and store back updated iterator
        if !self.builder.is_terminated() {
            let one = self.builder.build_const(TirConst::Int64(1))?;
            let next_idx = self.builder.build_add(idx_val, one)?;
            let updated_it = self.builder.build_set(
                loaded_it,
                tempra_tir::ir::TirKey::Field("idx".to_string()),
                next_idx,
            )?;
            self.builder.build_store(iter_slot, updated_it)?;
            self.builder.build_br(cond_block)?;
        }

        // 5. Exit block
        self.builder.set_current_block(exit_block);

        Ok(())
    }

    fn lower_block_expr(&mut self, block: &HirBlock) -> Result<TirValueId, TirLowerError> {
        let old_locals = self.locals.clone();
        let mut last_val = None;
        for (i, stmt) in block.statements.iter().enumerate() {
            if i == block.statements.len() - 1 {
                if let HirStatement::Expr(expr) = stmt {
                    last_val = Some(self.lower_expression(expr)?);
                } else {
                    self.lower_statement(stmt)?;
                }
            } else {
                self.lower_statement(stmt)?;
            }
        }
        self.locals = old_locals;
        if let Some(v) = last_val {
            Ok(v)
        } else {
            self.builder
                .build_const(TirConst::Unit)
                .map_err(|e| e.into())
        }
    }

    fn lower_for_expr(&mut self, f: &HirForExpr) -> Result<TirValueId, TirLowerError> {
        let iterable_ty = self.infer_expr_type(&f.iterable)?;
        if !matches!(iterable_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
            return Err(TirLowerError::UnsupportedExpr(
                "for-expr currently only supports Array/Slice/String in experimental VM".into(),
                f.iterable.span(),
            ));
        }

        let element_ty = match &iterable_ty {
            Type::Array(inner) | Type::Slice(inner) => inner.as_ref().clone(),
            Type::String => Type::UInt8,
            _ => {
                return Err(TirLowerError::UnsupportedExpr(
                    format!(
                        "for-expr currently only supports Array/Slice/String in experimental VM, found {}",
                        iterable_ty
                    ),
                    f.iterable.span(),
                ))
            }
        };

        let tir_element_ty = map_type(&element_ty)?;

        // 1. Temporarily insert binding for inference of result array type
        let old_locals_for_inf = self.locals.clone();
        // Use a dummy TirValueId(0) as it's not used during inference
        self.locals.insert(
            f.binding.clone(),
            (TirSlotId(0), tir_element_ty.clone(), false),
        );
        let body_ty = self.infer_block_type(&f.body)?;
        self.locals = old_locals_for_inf;

        let tir_body_ty = map_type(&body_ty)?;
        let result_array_ty = TirType::Array(Box::new(tir_body_ty));

        // 2. Initialize result accumulator slot with empty array
        let empty_arr = self
            .builder
            .build_make(result_array_ty.clone(), None, vec![])?;
        let res_slot = self
            .builder
            .build_slot(result_array_ty, Some(&format!("$for_res_{}", f.span.start)))?;
        self.builder.build_store(res_slot, empty_arr)?;

        // 3. Lower iterable and create iterator struct { data, idx: 0 }
        let iter_val = self.lower_expression(&f.iterable)?;
        // For String, desugar to bytes() first
        let data_val = if matches!(iterable_ty, Type::String) {
            self.builder.build_bytes(iter_val)?
        } else {
            iter_val
        };
        let zero = self.builder.build_const(TirConst::Int64(0))?;
        let struct_name = if matches!(iterable_ty, Type::Array(_)) {
            "$ArrayIter"
        } else {
            "$SliceIter"
        };
        let iter_struct = self.builder.build_make_struct(
            struct_name.to_string(),
            vec![("data".to_string(), data_val), ("idx".to_string(), zero)],
        )?;

        let tir_iter_ty = TirType::Struct(struct_name.to_string());
        let iter_slot = self
            .builder
            .build_slot(tir_iter_ty, Some(&format!("$it_{}", f.span.start)))?;
        self.builder.build_store(iter_slot, iter_struct)?;

        // 4. Create blocks
        let cond_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        self.builder.build_br(cond_block)?;

        // 5. Condition block — inline next() logic
        self.builder.set_current_block(cond_block);
        let loaded_it = self.builder.build_load(iter_slot)?;
        let data_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("data".to_string()))?;
        let idx_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("idx".to_string()))?;
        let len_val = self.builder.build_len(data_val)?;
        let is_less = self.builder.build_lt(idx_val, len_val)?;
        self.builder.build_condbr(is_less, body_block, exit_block)?;

        // 6. Body block
        self.builder.set_current_block(body_block);

        // Extract element at current idx
        let element_val = self.builder.build_get_index(data_val, idx_val)?;

        // Scope for loop variable
        let old_locals = self.locals.clone();

        let x_slot = self
            .builder
            .build_slot(tir_element_ty.clone(), Some(&f.binding))?;
        self.builder.build_store(x_slot, element_val)?;
        self.locals
            .insert(f.binding.clone(), (x_slot, tir_element_ty, false));

        // Lower body statements
        let mut last_val = None;
        for (i, stmt) in f.body.statements.iter().enumerate() {
            if i == f.body.statements.len() - 1 {
                if let HirStatement::Expr(expr) = stmt {
                    last_val = Some(self.lower_expression(expr)?);
                } else {
                    self.lower_statement(stmt)?;
                }
            } else {
                self.lower_statement(stmt)?;
            }
        }

        let body_val = if let Some(v) = last_val {
            v
        } else {
            self.builder
                .build_const(TirConst::Unit)
                .map_err(TirLowerError::from)?
        };

        // Push result to accumulator
        let cur_res = self.builder.build_load(res_slot)?;
        let next_res = self.builder.build_array_push(cur_res, body_val)?;
        self.builder.build_store(res_slot, next_res)?;

        // Restore locals scope
        self.locals = old_locals;

        // Increment idx and store back updated iterator
        if !self.builder.is_terminated() {
            let one = self.builder.build_const(TirConst::Int64(1))?;
            let next_idx = self.builder.build_add(idx_val, one)?;
            let updated_it = self.builder.build_set(
                loaded_it,
                tempra_tir::ir::TirKey::Field("idx".to_string()),
                next_idx,
            )?;
            self.builder.build_store(iter_slot, updated_it)?;
            self.builder.build_br(cond_block)?;
        }

        // 7. Exit block
        self.builder.set_current_block(exit_block);

        self.builder
            .build_load(res_slot)
            .map_err(TirLowerError::from)
    }

    fn collection_element_type(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::Array(inner) | Type::Slice(inner) => Some(*inner.clone()),
            _ => None,
        }
    }

    fn lookup_direct_callback_signature(
        &self,
        name: &str,
    ) -> Option<(String, tempra_typeck::context::FunctionSignature)> {
        if !self.type_ctx.current_module.is_empty() {
            let qualified = format!("{}.{}", self.type_ctx.current_module, name);
            if let Some(sig) = self.type_ctx.functions.get(&qualified) {
                return Some((qualified, sig.clone()));
            }
        }

        self.type_ctx
            .functions
            .get(name)
            .map(|sig| (name.to_string(), sig.clone()))
    }

    fn resolve_collection_callback(
        &self,
        method: &str,
        args: &[HirCallArgument],
        span: Span,
        callback_arg_index: usize,
        expected_params: &[Type],
    ) -> Result<String, TirLowerError> {
        if args.len() != callback_arg_index + 1 {
            return Err(TirLowerError::UnsupportedExpr(
                format!("'{}' expects {} arguments", method, callback_arg_index + 1),
                span,
            ));
        }

        let expr = match &args[callback_arg_index] {
            HirCallArgument::Positional(expr) => expr,
            HirCallArgument::Named(named) => {
                return Err(TirLowerError::UnsupportedExpr(
                    format!(
                        "'{}' expects a positional direct function callback, found '{}'",
                        method, named.name
                    ),
                    named.value.span(),
                ))
            }
        };

        let HirExpression::Name(name_span, name) = expr else {
            return Err(TirLowerError::UnsupportedExpr(
                format!("'{}' expects a direct top-level function reference", method),
                expr.span(),
            ));
        };

        let Some((qualified_name, sig)) = self.lookup_direct_callback_signature(name) else {
            return Err(TirLowerError::UnsupportedExpr(
                format!(
                    "'{}' expects a direct top-level function reference, found '{}'",
                    method, name
                ),
                *name_span,
            ));
        };

        let runtime_params: Vec<_> = sig
            .params
            .iter()
            .filter(|param| param.kind == tempra_ast::ParameterKind::Runtime)
            .collect();

        if runtime_params.len() != expected_params.len() {
            return Err(TirLowerError::UnsupportedExpr(
                format!(
                    "'{}' callback '{}' must be a function with exactly {} runtime parameters",
                    method,
                    name,
                    expected_params.len()
                ),
                *name_span,
            ));
        }

        if !sig.generic_params.is_empty() {
            let mut inferred = HashMap::new();
            for (param, expected) in runtime_params.iter().zip(expected_params.iter()) {
                infer_generic_type_args(&param.ty, expected, &mut inferred);
            }

            let mut type_args = Vec::new();
            for generic in &sig.generic_params {
                let Some(ty) = inferred.get(&generic.id) else {
                    return Err(TirLowerError::UnsupportedExpr(
                        format!(
                            "'{}' callback '{}' generic parameter '{}' could not be inferred",
                            method, name, generic.name
                        ),
                        *name_span,
                    ));
                };
                type_args.push(ty.clone());
            }

            return Ok(specialized_function_name(&qualified_name, &type_args));
        }

        Ok(qualified_name)
    }

    fn build_collection_iterator(
        &mut self,
        object: &HirExpression,
        obj_ty: &Type,
        span: Span,
    ) -> Result<TirSlotId, TirLowerError> {
        let obj = self.lower_expression(object)?;
        let zero = self.builder.build_const(TirConst::Int64(0))?;
        let struct_name = match obj_ty {
            Type::Array(_) => "$ArrayIter",
            Type::Slice(_) => "$SliceIter",
            _ => {
                return Err(TirLowerError::UnsupportedExpr(
                    format!("collection helper requires Array/Slice, found {}", obj_ty),
                    span,
                ))
            }
        };
        let iter_struct = self.builder.build_make_struct(
            struct_name.to_string(),
            vec![("data".to_string(), obj), ("idx".to_string(), zero)],
        )?;
        let iter_slot = self.builder.build_slot(
            TirType::Struct(struct_name.to_string()),
            Some(&format!("$functional_it_{}", span.start)),
        )?;
        self.builder.build_store(iter_slot, iter_struct)?;
        Ok(iter_slot)
    }

    fn build_collection_loop_cond(
        &mut self,
        iter_slot: TirSlotId,
        body_block: TirBlockId,
        exit_block: TirBlockId,
    ) -> Result<(TirValueId, TirValueId, TirValueId), TirLowerError> {
        let loaded_it = self.builder.build_load(iter_slot)?;
        let data_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("data".to_string()))?;
        let idx_val = self
            .builder
            .build_get(loaded_it, tempra_tir::ir::TirKey::Field("idx".to_string()))?;
        let len_val = self.builder.build_len(data_val)?;
        let is_less = self.builder.build_lt(idx_val, len_val)?;
        self.builder.build_condbr(is_less, body_block, exit_block)?;
        Ok((loaded_it, data_val, idx_val))
    }

    fn build_collection_loop_continue(
        &mut self,
        loaded_it: TirValueId,
        idx_val: TirValueId,
        iter_slot: TirSlotId,
        cond_block: TirBlockId,
    ) -> Result<(), TirLowerError> {
        let one = self.builder.build_const(TirConst::Int64(1))?;
        let next_idx = self.builder.build_add(idx_val, one)?;
        let updated_it = self.builder.build_set(
            loaded_it,
            tempra_tir::ir::TirKey::Field("idx".to_string()),
            next_idx,
        )?;
        self.builder.build_store(iter_slot, updated_it)?;
        self.builder.build_br(cond_block)?;
        Ok(())
    }

    fn lower_collection_helper_call(
        &mut self,
        method: &str,
        object: &HirExpression,
        args: &[HirCallArgument],
        span: Span,
        obj_ty: &Type,
    ) -> Result<Option<TirValueId>, TirLowerError> {
        let Some(elem_ty) = self.collection_element_type(obj_ty) else {
            return Ok(None);
        };

        match method {
            "count" => {
                if !args.is_empty() {
                    return Err(TirLowerError::UnsupportedExpr(
                        "'count' expects 0 arguments".into(),
                        span,
                    ));
                }

                let iter_slot = self.build_collection_iterator(object, obj_ty, span)?;
                let zero = self.builder.build_const(TirConst::Int64(0))?;
                let count_slot = self
                    .builder
                    .build_slot(TirType::Int64, Some(&format!("$count_{}", span.start)))?;
                self.builder.build_store(count_slot, zero)?;

                let cond_block = self.builder.new_block();
                let body_block = self.builder.new_block();
                let exit_block = self.builder.new_block();
                self.builder.build_br(cond_block)?;

                self.builder.set_current_block(cond_block);
                let (loaded_it, _data_val, idx_val) =
                    self.build_collection_loop_cond(iter_slot, body_block, exit_block)?;

                self.builder.set_current_block(body_block);
                let current = self.builder.build_load(count_slot)?;
                let one = self.builder.build_const(TirConst::Int64(1))?;
                let next = self.builder.build_add(current, one)?;
                self.builder.build_store(count_slot, next)?;
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;

                self.builder.set_current_block(exit_block);
                return self
                    .builder
                    .build_load(count_slot)
                    .map(Some)
                    .map_err(TirLowerError::from);
            }
            "fold" => {
                if args.len() != 2 {
                    return Err(TirLowerError::UnsupportedExpr(
                        "'fold' expects 2 arguments".into(),
                        span,
                    ));
                }

                let init_expr = match &args[0] {
                    HirCallArgument::Positional(expr) => expr,
                    HirCallArgument::Named(named) => {
                        return Err(TirLowerError::UnsupportedExpr(
                            format!(
                                "'fold' expects positional arguments, found '{}'",
                                named.name
                            ),
                            named.value.span(),
                        ))
                    }
                };
                let init_ty = self.infer_expr_type(init_expr)?;
                let callback_name = self.resolve_collection_callback(
                    method,
                    args,
                    span,
                    1,
                    &[init_ty.clone(), elem_ty.clone()],
                )?;
                let callback_id = self.module_builder.declare_function(&callback_name);
                let init_val = self.lower_expression(init_expr)?;
                let acc_ty = map_type(&init_ty)?;
                let acc_slot = self
                    .builder
                    .build_slot(acc_ty.clone(), Some(&format!("$fold_acc_{}", span.start)))?;
                self.builder.build_store(acc_slot, init_val)?;

                let iter_slot = self.build_collection_iterator(object, obj_ty, span)?;
                let cond_block = self.builder.new_block();
                let body_block = self.builder.new_block();
                let exit_block = self.builder.new_block();
                self.builder.build_br(cond_block)?;

                self.builder.set_current_block(cond_block);
                let (loaded_it, data_val, idx_val) =
                    self.build_collection_loop_cond(iter_slot, body_block, exit_block)?;

                self.builder.set_current_block(body_block);
                let item_val = self.builder.build_get_index(data_val, idx_val)?;
                let acc_val = self.builder.build_load(acc_slot)?;
                let next_acc = self
                    .builder
                    .build_call(callback_id, vec![acc_val, item_val])?;
                self.builder.build_store(acc_slot, next_acc)?;
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;

                self.builder.set_current_block(exit_block);
                return self
                    .builder
                    .build_load(acc_slot)
                    .map(Some)
                    .map_err(TirLowerError::from);
            }
            "reduce" => {
                let callback_name = self.resolve_collection_callback(
                    method,
                    args,
                    span,
                    0,
                    &[elem_ty.clone(), elem_ty.clone()],
                )?;
                let callback_id = self.module_builder.declare_function(&callback_name);
                let elem_tir_ty = map_type(&elem_ty)?;
                let opt_ty = TirType::Option(Box::new(elem_tir_ty.clone()));
                let none =
                    self.builder
                        .build_make(opt_ty.clone(), Some("None".to_string()), vec![])?;
                let res_slot = self
                    .builder
                    .build_slot(opt_ty.clone(), Some(&format!("$reduce_res_{}", span.start)))?;
                self.builder.build_store(res_slot, none)?;
                let acc_slot = self.builder.build_slot(
                    elem_tir_ty.clone(),
                    Some(&format!("$reduce_acc_{}", span.start)),
                )?;

                let iter_slot = self.build_collection_iterator(object, obj_ty, span)?;
                let first_cond_block = self.builder.new_block();
                let first_body_block = self.builder.new_block();
                let cond_block = self.builder.new_block();
                let body_block = self.builder.new_block();
                let done_block = self.builder.new_block();
                let exit_block = self.builder.new_block();
                self.builder.build_br(first_cond_block)?;

                self.builder.set_current_block(first_cond_block);
                let (loaded_it, data_val, idx_val) =
                    self.build_collection_loop_cond(iter_slot, first_body_block, exit_block)?;

                self.builder.set_current_block(first_body_block);
                let first_val = self.builder.build_get_index(data_val, idx_val)?;
                self.builder.build_store(acc_slot, first_val)?;
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;

                self.builder.set_current_block(cond_block);
                let (loaded_it, data_val, idx_val) =
                    self.build_collection_loop_cond(iter_slot, body_block, done_block)?;

                self.builder.set_current_block(body_block);
                let item_val = self.builder.build_get_index(data_val, idx_val)?;
                let acc_val = self.builder.build_load(acc_slot)?;
                let next_acc = self
                    .builder
                    .build_call(callback_id, vec![acc_val, item_val])?;
                self.builder.build_store(acc_slot, next_acc)?;
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;

                self.builder.set_current_block(done_block);
                let final_acc = self.builder.build_load(acc_slot)?;
                let some =
                    self.builder
                        .build_make(opt_ty, Some("Some".to_string()), vec![final_acc])?;
                self.builder.build_store(res_slot, some)?;
                self.builder.build_br(exit_block)?;

                self.builder.set_current_block(exit_block);
                return self
                    .builder
                    .build_load(res_slot)
                    .map(Some)
                    .map_err(TirLowerError::from);
            }
            "map" | "filter" | "find" | "any" | "all" | "for_each" => {}
            _ => return Ok(None),
        }

        let callback_name = self.resolve_collection_callback(
            method,
            args,
            span,
            0,
            std::slice::from_ref(&elem_ty),
        )?;
        let callback_sig = self
            .type_ctx
            .functions
            .get(&callback_name)
            .ok_or_else(|| TirLowerError::UnknownFunction(callback_name.clone(), span))?
            .clone();
        let callback_id = self.module_builder.declare_function(&callback_name);
        let elem_tir_ty = map_type(&elem_ty)?;

        let iter_slot = self.build_collection_iterator(object, obj_ty, span)?;
        let result_slot = match method {
            "map" => {
                let out_ty = map_type(&callback_sig.return_type)?;
                let arr_ty = TirType::Array(Box::new(out_ty));
                let empty = self.builder.build_make(arr_ty.clone(), None, vec![])?;
                let slot = self
                    .builder
                    .build_slot(arr_ty, Some(&format!("$map_res_{}", span.start)))?;
                self.builder.build_store(slot, empty)?;
                Some(slot)
            }
            "filter" => {
                let arr_ty = TirType::Array(Box::new(elem_tir_ty.clone()));
                let empty = self.builder.build_make(arr_ty.clone(), None, vec![])?;
                let slot = self
                    .builder
                    .build_slot(arr_ty, Some(&format!("$filter_res_{}", span.start)))?;
                self.builder.build_store(slot, empty)?;
                Some(slot)
            }
            "find" => {
                let opt_ty = TirType::Option(Box::new(elem_tir_ty.clone()));
                let none =
                    self.builder
                        .build_make(opt_ty.clone(), Some("None".to_string()), vec![])?;
                let slot = self
                    .builder
                    .build_slot(opt_ty, Some(&format!("$find_res_{}", span.start)))?;
                self.builder.build_store(slot, none)?;
                Some(slot)
            }
            "any" => {
                let init = self.builder.build_const(TirConst::Bool(false))?;
                let slot = self
                    .builder
                    .build_slot(TirType::Bool, Some(&format!("$any_res_{}", span.start)))?;
                self.builder.build_store(slot, init)?;
                Some(slot)
            }
            "all" => {
                let init = self.builder.build_const(TirConst::Bool(true))?;
                let slot = self
                    .builder
                    .build_slot(TirType::Bool, Some(&format!("$all_res_{}", span.start)))?;
                self.builder.build_store(slot, init)?;
                Some(slot)
            }
            "for_each" => None,
            _ => unreachable!(),
        };

        let cond_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();
        self.builder.build_br(cond_block)?;

        self.builder.set_current_block(cond_block);
        let (loaded_it, data_val, idx_val) =
            self.build_collection_loop_cond(iter_slot, body_block, exit_block)?;

        self.builder.set_current_block(body_block);
        let item_val = self.builder.build_get_index(data_val, idx_val)?;
        let callback_val = self.builder.build_call(callback_id, vec![item_val])?;

        match method {
            "map" => {
                let slot = result_slot.unwrap();
                let cur = self.builder.build_load(slot)?;
                let next = self.builder.build_array_push(cur, callback_val)?;
                self.builder.build_store(slot, next)?;
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;
            }
            "filter" => {
                let push_block = self.builder.new_block();
                let skip_block = self.builder.new_block();
                let cont_block = self.builder.new_block();
                self.builder
                    .build_condbr(callback_val, push_block, skip_block)?;

                self.builder.set_current_block(push_block);
                let slot = result_slot.unwrap();
                let cur = self.builder.build_load(slot)?;
                let next = self.builder.build_array_push(cur, item_val)?;
                self.builder.build_store(slot, next)?;
                self.builder.build_br(cont_block)?;

                self.builder.set_current_block(skip_block);
                self.builder.build_br(cont_block)?;

                self.builder.set_current_block(cont_block);
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;
            }
            "find" => {
                let found_block = self.builder.new_block();
                let next_block = self.builder.new_block();
                self.builder
                    .build_condbr(callback_val, found_block, next_block)?;

                self.builder.set_current_block(found_block);
                let slot = result_slot.unwrap();
                let opt_ty = TirType::Option(Box::new(elem_tir_ty));
                let some =
                    self.builder
                        .build_make(opt_ty, Some("Some".to_string()), vec![item_val])?;
                self.builder.build_store(slot, some)?;
                self.builder.build_br(exit_block)?;

                self.builder.set_current_block(next_block);
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;
            }
            "any" => {
                let true_block = self.builder.new_block();
                let false_block = self.builder.new_block();
                self.builder
                    .build_condbr(callback_val, true_block, false_block)?;

                self.builder.set_current_block(true_block);
                let slot = result_slot.unwrap();
                let true_val = self.builder.build_const(TirConst::Bool(true))?;
                self.builder.build_store(slot, true_val)?;
                self.builder.build_br(exit_block)?;

                self.builder.set_current_block(false_block);
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;
            }
            "all" => {
                let true_block = self.builder.new_block();
                let false_block = self.builder.new_block();
                self.builder
                    .build_condbr(callback_val, true_block, false_block)?;

                self.builder.set_current_block(true_block);
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;

                self.builder.set_current_block(false_block);
                let slot = result_slot.unwrap();
                let false_val = self.builder.build_const(TirConst::Bool(false))?;
                self.builder.build_store(slot, false_val)?;
                self.builder.build_br(exit_block)?;
            }
            "for_each" => {
                self.build_collection_loop_continue(loaded_it, idx_val, iter_slot, cond_block)?;
            }
            _ => unreachable!(),
        }

        self.builder.set_current_block(exit_block);
        if let Some(slot) = result_slot {
            self.builder
                .build_load(slot)
                .map(Some)
                .map_err(TirLowerError::from)
        } else {
            self.builder
                .build_const(TirConst::Unit)
                .map(Some)
                .map_err(TirLowerError::from)
        }
    }

    pub fn lower_expression(&mut self, expr: &HirExpression) -> Result<TirValueId, TirLowerError> {
        match expr {
            HirExpression::Integer(span, s) => {
                let val = s.parse::<i64>().map_err(|_| {
                    TirLowerError::UnsupportedExpr("Invalid Int64 literal".into(), *span)
                })?;
                self.builder
                    .build_const(TirConst::Int64(val))
                    .map_err(|e| e.into())
            }
            HirExpression::Bool(_, b) => self
                .builder
                .build_const(TirConst::Bool(*b))
                .map_err(|e| e.into()),
            HirExpression::String(_, s) => {
                let unquoted = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    &s[1..s.len() - 1]
                } else {
                    s.as_str()
                };
                self.builder
                    .build_const(TirConst::String(unquoted.to_string()))
                    .map_err(|e| e.into())
            }
            HirExpression::Name(span, name) => {
                if name == "None" {
                    let ty = self.infer_expr_type(expr)?;
                    let tir_ty = map_type(&ty)?;
                    return self
                        .builder
                        .build_make(tir_ty, Some("None".to_string()), vec![])
                        .map_err(|e| e.into());
                }
                if let Some((slot, _ty, _)) = self.locals.get(name) {
                    self.builder.build_load(*slot).map_err(|e| e.into())
                } else {
                    let keys: Vec<_> = self.locals.keys().cloned().collect();
                    Err(TirLowerError::UnsupportedExpr(
                        format!("Unknown local: {}. Available: {:?}", name, keys),
                        *span,
                    ))
                }
            }
            HirExpression::Binary(b) => {
                if b.op == HirBinaryOp::Assign {
                    if let HirExpression::Name(span, name) = &*b.left {
                        let (slot, _ty, mutable) = if let Some(info) = self.locals.get(name) {
                            info.clone()
                        } else {
                            return Err(TirLowerError::UnknownLocal(name.clone(), *span));
                        };

                        if !mutable {
                            return Err(TirLowerError::UnsupportedStmt(
                                "cannot assign to let binding".into(),
                                *span,
                            ));
                        }

                        let val = self.lower_expression(&b.right)?;
                        self.builder.build_store(slot, val)?;
                        return self
                            .builder
                            .build_const(TirConst::Unit)
                            .map_err(|e| e.into());
                    } else if let HirExpression::Member(m) = &*b.left {
                        if let HirExpression::Name(span, name) = &*m.object {
                            let (slot, _ty, mutable) = if let Some(info) = self.locals.get(name) {
                                info.clone()
                            } else {
                                return Err(TirLowerError::UnknownLocal(name.clone(), *span));
                            };
                            if !mutable {
                                return Err(TirLowerError::UnsupportedStmt(
                                    "cannot assign to let binding".into(),
                                    *span,
                                ));
                            }
                            let val = self.lower_expression(&b.right)?;
                            let obj = self.lower_expression(&m.object)?;
                            let new_obj = self.builder.build_set(
                                obj,
                                tempra_tir::ir::TirKey::Field(m.field.clone()),
                                val,
                            )?;
                            self.builder.build_store(slot, new_obj)?;
                            return self
                                .builder
                                .build_const(TirConst::Unit)
                                .map_err(|e| e.into());
                        } else {
                            return Err(TirLowerError::UnsupportedStmt(
                                "nested struct assignment not supported in v0".into(),
                                b.span,
                            ));
                        }
                    } else {
                        return Err(TirLowerError::UnsupportedStmt(
                            "non-local/non-member assignment not supported".into(),
                            b.span,
                        ));
                    }
                }

                let left = self.lower_expression(&b.left)?;
                let right = self.lower_expression(&b.right)?;

                match b.op {
                    HirBinaryOp::Add => self.builder.build_add(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Sub => self.builder.build_sub(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Mul => self.builder.build_mul(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Div => self.builder.build_div(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Eq => self.builder.build_eq(left, right).map_err(|e| e.into()),
                    HirBinaryOp::NotEq => {
                        let eq = self.builder.build_eq(left, right)?;
                        self.builder.build_not(eq).map_err(|e| e.into())
                    }
                    HirBinaryOp::Less => self.builder.build_lt(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Greater => {
                        self.builder.build_gt(left, right).map_err(|e| e.into())
                    }
                    HirBinaryOp::LessEq => {
                        let gt = self.builder.build_gt(left, right)?;
                        self.builder.build_not(gt).map_err(|e| e.into())
                    }
                    HirBinaryOp::GreaterEq => {
                        let lt = self.builder.build_lt(left, right)?;
                        self.builder.build_not(lt).map_err(|e| e.into())
                    }
                    HirBinaryOp::And => self.builder.build_and(left, right).map_err(|e| e.into()),
                    HirBinaryOp::Or => self.builder.build_or(left, right).map_err(|e| e.into()),
                    _ => Err(TirLowerError::UnsupportedExpr(
                        format!("Binary {:?}", b.op),
                        b.span,
                    )),
                }
            }
            HirExpression::Unary(u) => {
                let inner = self.lower_expression(&u.expr)?;
                match u.op {
                    HirUnaryOp::Not => self.builder.build_not(inner).map_err(|e| e.into()),
                    HirUnaryOp::Negate => {
                        let zero = self.builder.build_const(TirConst::Int64(0))?;
                        self.builder.build_sub(zero, inner).map_err(|e| e.into())
                    }
                }
            }
            HirExpression::If(i) => {
                if let HirExpression::IsPattern(is_pat) = &*i.condition {
                    return self.lower_narrowing_if(i, is_pat);
                }
                let cond = self.lower_expression(&i.condition)?;
                let mut t_block = self.builder.new_block();
                let mut f_block = self.builder.new_block();

                self.builder.build_condbr(cond, t_block, f_block)?;

                // Then block
                self.builder.set_current_block(t_block);
                for stmt in &i.then_block.statements {
                    self.lower_statement(stmt)?;
                }
                t_block = self.builder.get_current_block().unwrap();
                let t_terminated = self.builder.is_terminated();

                // Else block
                self.builder.set_current_block(f_block);
                if let Some(else_expr) = &i.else_branch {
                    if let HirExpression::Block(b) = &**else_expr {
                        for stmt in &b.statements {
                            self.lower_statement(stmt)?;
                        }
                    } else if let HirExpression::If(_nested_if) = &**else_expr {
                        self.lower_expression(else_expr)?;
                    } else {
                        return Err(TirLowerError::UnsupportedExpr(
                            "Else is not a block or if".into(),
                            else_expr.span(),
                        ));
                    }
                }
                f_block = self.builder.get_current_block().unwrap();
                let f_terminated = self.builder.is_terminated();

                if t_terminated && f_terminated {
                    // Both branches terminate. We still create a join block so that
                    // subsequent instructions (which are unreachable) can be appended
                    // without causing a "block already terminated" error.
                    let end_block = self.builder.new_block();
                    self.builder.set_current_block(end_block);
                    Ok(TirValueId(0))
                } else {
                    let end_block = self.builder.new_block();

                    if !t_terminated {
                        self.builder.set_current_block(t_block);
                        self.builder.build_br(end_block)?;
                    }
                    if !f_terminated {
                        self.builder.set_current_block(f_block);
                        self.builder.build_br(end_block)?;
                    }

                    self.builder.set_current_block(end_block);
                    self.builder
                        .build_const(TirConst::Unit)
                        .map_err(|e| e.into())
                }
            }
            HirExpression::Call(c) => {
                // Handle .len() method call
                if let HirExpression::Member(m) = &*c.callee {
                    let obj_ty = self.infer_expr_type(&m.object)?;
                    if c.args.is_empty() {
                        if m.field == "len" {
                            if matches!(obj_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
                                let obj = self.lower_expression(&m.object)?;
                                return self.builder.build_len(obj).map_err(|e| e.into());
                            }
                        } else if m.field == "size" {
                            if matches!(obj_ty, Type::String) {
                                let obj = self.lower_expression(&m.object)?;
                                return self.builder.build_size(obj).map_err(|e| e.into());
                            }
                        } else if m.field == "bytes" {
                            if matches!(obj_ty, Type::String) {
                                let obj = self.lower_expression(&m.object)?;
                                return self.builder.build_bytes(obj).map_err(|e| e.into());
                            }
                        } else if m.field == "is_empty" {
                            if matches!(obj_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
                                let obj = self.lower_expression(&m.object)?;
                                let len_val = self.builder.build_len(obj)?;
                                let zero_val = self.builder.build_const(TirConst::Int64(0))?;
                                return self
                                    .builder
                                    .build_eq(len_val, zero_val)
                                    .map_err(|e| e.into());
                            }
                        } else if m.field == "first" {
                            if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                                let inner_ty = match &obj_ty {
                                    Type::Array(t) | Type::Slice(t) => map_type(t.as_ref())?,
                                    _ => unreachable!(),
                                };
                                let opt_ty = TirType::Option(Box::new(inner_ty));

                                let obj = self.lower_expression(&m.object)?;
                                let len_val = self.builder.build_len(obj)?;
                                let zero_val = self.builder.build_const(TirConst::Int64(0))?;
                                let is_empty = self.builder.build_eq(len_val, zero_val)?;

                                let tmp_slot = self
                                    .builder
                                    .build_slot(opt_ty.clone(), Some("$first_tmp"))?;
                                let init_none = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("None".to_string()),
                                    vec![],
                                )?;
                                self.builder.build_store(tmp_slot, init_none)?;

                                let empty_block = self.builder.new_block();
                                let non_empty_block = self.builder.new_block();
                                let end_block = self.builder.new_block();

                                self.builder.build_condbr(
                                    is_empty,
                                    empty_block,
                                    non_empty_block,
                                )?;

                                // empty_block
                                self.builder.set_current_block(empty_block);
                                let none_val = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("None".to_string()),
                                    vec![],
                                )?;
                                self.builder.build_store(tmp_slot, none_val)?;
                                self.builder.build_br(end_block)?;

                                // non_empty_block
                                self.builder.set_current_block(non_empty_block);
                                let first_val = self.builder.build_get_index(obj, zero_val)?;
                                let some_val = self.builder.build_make(
                                    opt_ty,
                                    Some("Some".to_string()),
                                    vec![first_val],
                                )?;
                                self.builder.build_store(tmp_slot, some_val)?;
                                self.builder.build_br(end_block)?;

                                // end_block
                                self.builder.set_current_block(end_block);
                                return self.builder.build_load(tmp_slot).map_err(|e| e.into());
                            }
                        } else if m.field == "last" {
                            if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                                let inner_ty = match &obj_ty {
                                    Type::Array(t) | Type::Slice(t) => map_type(t.as_ref())?,
                                    _ => unreachable!(),
                                };
                                let opt_ty = TirType::Option(Box::new(inner_ty));

                                let obj = self.lower_expression(&m.object)?;
                                let len_val = self.builder.build_len(obj)?;
                                let zero_val = self.builder.build_const(TirConst::Int64(0))?;
                                let is_empty = self.builder.build_eq(len_val, zero_val)?;

                                let tmp_slot =
                                    self.builder.build_slot(opt_ty.clone(), Some("$last_tmp"))?;
                                let init_none = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("None".to_string()),
                                    vec![],
                                )?;
                                self.builder.build_store(tmp_slot, init_none)?;

                                let empty_block = self.builder.new_block();
                                let non_empty_block = self.builder.new_block();
                                let end_block = self.builder.new_block();

                                self.builder.build_condbr(
                                    is_empty,
                                    empty_block,
                                    non_empty_block,
                                )?;

                                // empty_block
                                self.builder.set_current_block(empty_block);
                                let none_val = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("None".to_string()),
                                    vec![],
                                )?;
                                self.builder.build_store(tmp_slot, none_val)?;
                                self.builder.build_br(end_block)?;

                                // non_empty_block
                                self.builder.set_current_block(non_empty_block);
                                let one_val = self.builder.build_const(TirConst::Int64(1))?;
                                let last_idx = self.builder.build_sub(len_val, one_val)?;
                                let last_val = self.builder.build_get_index(obj, last_idx)?;
                                let some_val = self.builder.build_make(
                                    opt_ty,
                                    Some("Some".to_string()),
                                    vec![last_val],
                                )?;
                                self.builder.build_store(tmp_slot, some_val)?;
                                self.builder.build_br(end_block)?;

                                // end_block
                                self.builder.set_current_block(end_block);
                                return self.builder.build_load(tmp_slot).map_err(|e| e.into());
                            }
                        } else if m.field == "iter" {
                            if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                                let obj = self.lower_expression(&m.object)?;
                                let zero_val = self.builder.build_const(TirConst::Int64(0))?;
                                let struct_name = if matches!(obj_ty, Type::Array(_)) {
                                    "$ArrayIter"
                                } else {
                                    "$SliceIter"
                                };
                                return self
                                    .builder
                                    .build_make_struct(
                                        struct_name.to_string(),
                                        vec![
                                            ("data".to_string(), obj),
                                            ("idx".to_string(), zero_val),
                                        ],
                                    )
                                    .map_err(|e| e.into());
                            } else if matches!(obj_ty, Type::String) {
                                // String.iter() => string.bytes().iter()
                                // Lower the string, call bytes() on it, then wrap in $SliceIter
                                let obj = self.lower_expression(&m.object)?;
                                let bytes_val = self.builder.build_bytes(obj)?;
                                let zero_val = self.builder.build_const(TirConst::Int64(0))?;
                                return self
                                    .builder
                                    .build_make_struct(
                                        "$SliceIter".to_string(),
                                        vec![
                                            ("data".to_string(), bytes_val),
                                            ("idx".to_string(), zero_val),
                                        ],
                                    )
                                    .map_err(|e| e.into());
                            }
                        } else if m.field == "next" {
                            if let Type::ArrayIter(inner_ty) | Type::SliceIter(inner_ty) = &obj_ty {
                                let opt_ty =
                                    TirType::Option(Box::new(map_type(inner_ty.as_ref())?));

                                // Load the iterator from the receiver
                                let obj = self.lower_expression(&m.object)?;

                                // Extract fields
                                let data_val = self.builder.build_get(
                                    obj,
                                    tempra_tir::ir::TirKey::Field("data".to_string()),
                                )?;
                                let idx_val = self.builder.build_get(
                                    obj,
                                    tempra_tir::ir::TirKey::Field("idx".to_string()),
                                )?;
                                let len_val = self.builder.build_len(data_val)?;

                                let is_less = self.builder.build_lt(idx_val, len_val)?;

                                let tmp_slot =
                                    self.builder.build_slot(opt_ty.clone(), Some("$next_tmp"))?;

                                let empty_block = self.builder.new_block();
                                let non_empty_block = self.builder.new_block();
                                let end_block = self.builder.new_block();

                                self.builder
                                    .build_condbr(is_less, non_empty_block, empty_block)?;

                                // empty_block
                                self.builder.set_current_block(empty_block);
                                let none_val = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("None".to_string()),
                                    vec![],
                                )?;
                                self.builder.build_store(tmp_slot, none_val)?;
                                self.builder.build_br(end_block)?;

                                // non_empty_block
                                self.builder.set_current_block(non_empty_block);
                                let item_val = self.builder.build_get_index(data_val, idx_val)?;
                                let some_val = self.builder.build_make(
                                    opt_ty.clone(),
                                    Some("Some".to_string()),
                                    vec![item_val],
                                )?;
                                self.builder.build_store(tmp_slot, some_val)?;

                                // Increment and update
                                let one_val = self.builder.build_const(TirConst::Int64(1))?;
                                let next_idx = self.builder.build_add(idx_val, one_val)?;
                                let new_obj = self.builder.build_set(
                                    obj,
                                    tempra_tir::ir::TirKey::Field("idx".to_string()),
                                    next_idx,
                                )?;

                                // Store back to local if it's a variable
                                if let HirExpression::Name(_, ref name) = *m.object {
                                    if let Some((slot, _, _)) = self.locals.get(name) {
                                        self.builder.build_store(*slot, new_obj)?;
                                    }
                                }

                                self.builder.build_br(end_block)?;

                                // end_block
                                self.builder.set_current_block(end_block);
                                return self.builder.build_load(tmp_slot).map_err(|e| e.into());
                            }
                        }
                    }

                    if let Some(value) = self.lower_collection_helper_call(
                        &m.field, &m.object, &c.args, c.span, &obj_ty,
                    )? {
                        return Ok(value);
                    }

                    if let Type::Namespace(ns) = &obj_ty {
                        let full_name = format!("{}.{}", ns, m.field);
                        if let Some(sig) = self.type_ctx.functions.get(&full_name) {
                            if sig.params.len() != c.args.len() {
                                return Err(TirLowerError::UnsupportedExpr(
                                    format!(
                                        "wrong arity for call to {}: expected {}, found {}",
                                        full_name,
                                        sig.params.len(),
                                        c.args.len()
                                    ),
                                    c.span,
                                ));
                            }

                            let mut arg_values = Vec::new();
                            for arg in &c.args {
                                let arg_expr = match arg {
                                    HirCallArgument::Positional(e) => e,
                                    HirCallArgument::Named(n) => &n.value,
                                };
                                arg_values.push(self.lower_expression(arg_expr)?);
                            }

                            let func_id = self.module_builder.declare_function(&full_name);
                            return self
                                .builder
                                .build_call(func_id, arg_values)
                                .map_err(|e| e.into());
                        }
                    }

                    // Static method call resolution
                    if let Some(sig) = self
                        .type_ctx
                        .lookup_direct_method_signature(&obj_ty, &m.field)
                    {
                        let full_name = format!("{}.{}", obj_ty.get_name(), m.field);
                        let mut arg_values = Vec::new();

                        // 1. First argument is the receiver
                        arg_values.push(self.lower_expression(&m.object)?);

                        // 2. Following arguments
                        for arg in &c.args {
                            let arg_expr = match arg {
                                HirCallArgument::Positional(e) => e,
                                HirCallArgument::Named(n) => &n.value,
                            };
                            arg_values.push(self.lower_expression(arg_expr)?);
                        }

                        if sig.params.len() != arg_values.len() {
                            return Err(TirLowerError::UnsupportedExpr(
                                format!(
                                    "wrong arity for method {}: expected {}, found {}",
                                    full_name,
                                    sig.params.len(),
                                    arg_values.len()
                                ),
                                c.span,
                            ));
                        }

                        let func_id = self.module_builder.declare_function(&full_name);
                        return self
                            .builder
                            .build_call(func_id, arg_values)
                            .map_err(|e| e.into());
                    }
                }

                if let HirExpression::Name(span, name) = &*c.callee {
                    if matches!(name.as_str(), "Some" | "Ok" | "Err") {
                        if c.args.len() != 1 {
                            return Err(TirLowerError::UnsupportedExpr(
                                format!("wrong arity for {}", name),
                                *span,
                            ));
                        }
                        let arg_expr = match &c.args[0] {
                            HirCallArgument::Positional(e) => e,
                            HirCallArgument::Named(n) => &n.value,
                        };
                        let arg_val = self.lower_expression(arg_expr)?;
                        let arg_hir_ty = self.infer_expr_type(arg_expr)?;
                        let arg_tir_ty = map_type(&arg_hir_ty)?;
                        let ty = if name == "Some" {
                            TirType::Option(Box::new(arg_tir_ty))
                        } else {
                            TirType::Result(Box::new(arg_tir_ty), Box::new(TirType::Unit))
                        };
                        return self
                            .builder
                            .build_make(ty, Some(name.clone()), vec![arg_val])
                            .map_err(|e| e.into());
                    }

                    if let Some(sig) = self.type_ctx.functions.get(name) {
                        if sig.params.len() != c.args.len() {
                            return Err(TirLowerError::UnsupportedExpr(
                                format!(
                                    "wrong arity for call to {}: expected {}, found {}",
                                    name,
                                    sig.params.len(),
                                    c.args.len()
                                ),
                                c.span,
                            ));
                        }

                        let mut arg_values = Vec::new();
                        for arg in &c.args {
                            let arg_expr = match arg {
                                HirCallArgument::Positional(e) => e,
                                HirCallArgument::Named(n) => &n.value,
                            };
                            arg_values.push(self.lower_expression(arg_expr)?);
                        }

                        let func_id = self.module_builder.declare_function(name);
                        return self
                            .builder
                            .build_call(func_id, arg_values)
                            .map_err(|e| e.into());
                    } else {
                        return Err(TirLowerError::UnknownFunction(name.clone(), *span));
                    }
                }
                Err(TirLowerError::UnsupportedExpr(
                    "Complex call resolution not supported in Phase 12.2".into(),
                    c.span,
                ))
            }
            HirExpression::Try(e) => {
                // Desugar expr? into: tag/eq/condbr/unwrap/make/ret
                // Step 1: Lower the inner expression
                let r_val = self.lower_expression(&e.expr)?;

                // Step 2: Store in a slot so both branches can access it
                let tmp_slot = self.builder.build_slot(
                    TirType::Result(Box::new(TirType::Unit), Box::new(TirType::Unit)),
                    Some("__try_tmp"),
                )?;
                self.builder.build_store(tmp_slot, r_val)?;

                // Step 3: Tag the result and compare to "Err"
                let r_for_tag = self.builder.build_load(tmp_slot)?;
                let tag_val = self.builder.build_tag(r_for_tag)?;
                let err_str = self
                    .builder
                    .build_const(TirConst::String("Err".to_string()))?;
                let is_err = self.builder.build_eq(tag_val, err_str)?;

                // Step 4: Create err_block and ok_block
                let err_block = self.builder.new_block();
                let ok_block = self.builder.new_block();
                self.builder.build_condbr(is_err, err_block, ok_block)?;

                // Step 5: err_block — unwrap Err payload, wrap in Result::Err, return early
                self.builder.set_current_block(err_block);
                let r_err = self.builder.build_load(tmp_slot)?;
                let err_payload = self.builder.build_unwrap(r_err, "Err".to_string(), 0)?;
                // Use the enclosing function's return type for the Make
                let fn_ret_ty = self.builder.func.return_type.clone();
                let err_out = self.builder.build_make(
                    fn_ret_ty,
                    Some("Err".to_string()),
                    vec![err_payload],
                )?;
                self.builder.build_ret(Some(err_out))?;

                // Step 6: ok_block — unwrap Ok payload, continue
                self.builder.set_current_block(ok_block);
                let r_ok = self.builder.build_load(tmp_slot)?;
                let ok_val = self.builder.build_unwrap(r_ok, "Ok".to_string(), 0)?;

                // ok_val is the result of the ? expression; ok_block is now current
                Ok(ok_val)
            }
            HirExpression::StructLiteral(s) => {
                let mut field_vals = Vec::new();
                for (name, expr) in &s.fields {
                    let val = self.lower_expression(expr)?;
                    field_vals.push((name.clone(), val));
                }
                let struct_name = match &s.ty {
                    tempra_hir::hir::HirType::Named(_, n) => n.clone(),
                    _ => {
                        return Err(TirLowerError::UnsupportedExpr(
                            "StructLiteral with non-struct type".into(),
                            s.span,
                        ))
                    }
                };
                self.builder
                    .build_make_struct(struct_name, field_vals)
                    .map_err(|e| e.into())
            }
            HirExpression::IsPattern(i) => self.lower_is_pattern(i),
            HirExpression::Array(_, elements) => {
                let mut vals = Vec::new();
                for e in elements {
                    vals.push(self.lower_expression(e)?);
                }
                let hir_ty = self.infer_expr_type(expr)?;
                let tir_ty = map_type(&hir_ty)?;
                self.builder
                    .build_make(tir_ty, None, vals)
                    .map_err(|e| e.into())
            }
            HirExpression::Index(i) => {
                let obj_ty = self.infer_expr_type(&i.object)?;
                if matches!(obj_ty, Type::String) {
                    return Err(TirLowerError::UnsupportedExpr(
                        "String indexing is not supported in the experimental VM".into(),
                        i.span,
                    ));
                }
                let obj = self.lower_expression(&i.object)?;
                let idx = self.lower_expression(&i.index)?;
                self.builder.build_get_index(obj, idx).map_err(|e| e.into())
            }
            HirExpression::Slice(s) => {
                let obj_ty = self.infer_expr_type(&s.object)?;
                if matches!(obj_ty, Type::String) {
                    return Err(TirLowerError::UnsupportedExpr(
                        "String slicing is not supported in the experimental VM".into(),
                        s.span,
                    ));
                }
                let obj = self.lower_expression(&s.object)?;
                let start = self.lower_expression(&s.start)?;
                let end = self.lower_expression(&s.end)?;
                self.builder
                    .build_slice(obj, start, end)
                    .map_err(|e| e.into())
            }
            HirExpression::Member(m) => {
                let obj_ty = self.infer_expr_type(&m.object)?;
                if m.field == "len" {
                    if matches!(obj_ty, Type::Array(_) | Type::Slice(_) | Type::String) {
                        let obj = self.lower_expression(&m.object)?;
                        return self.builder.build_len(obj).map_err(|e| e.into());
                    }
                } else if m.field == "size" {
                    if matches!(obj_ty, Type::String) {
                        let obj = self.lower_expression(&m.object)?;
                        return self.builder.build_size(obj).map_err(|e| e.into());
                    }
                }

                // If it's a struct field access
                if let tempra_hir::types::Type::Struct(_, _) = obj_ty {
                    let obj = self.lower_expression(&m.object)?;
                    return self
                        .builder
                        .build_get(obj, tempra_tir::ir::TirKey::Field(m.field.clone()))
                        .map_err(|e| e.into());
                }

                Err(TirLowerError::UnsupportedExpr(
                    format!("Member access: {}", m.field),
                    m.span,
                ))
            }
            HirExpression::Block(b) => self.lower_block_expr(b),
            HirExpression::For(f) => self.lower_for_expr(f),
            HirExpression::Match(m) => self.lower_match(m),
            _ => Err(TirLowerError::UnsupportedExpr(
                format!(
                    "Expression of type {:?} is not yet supported in experimental VM lowering",
                    expr
                ),
                expr.span(),
            )),
        }
    }

    fn lower_is_pattern(&mut self, i: &HirIsPatternExpr) -> Result<TirValueId, TirLowerError> {
        let (variant_name, binding_name) = self.extract_simple_pattern(&i.pattern)?;
        if binding_name.is_some() {
            return Err(TirLowerError::UnsupportedPattern(
                "bindings in `is` are only supported in if conditions".into(),
                i.span,
            ));
        }

        let subject_val = self.lower_expression(&i.value)?;
        let tag_val = self.builder.build_tag(subject_val)?;
        let wanted_tag = self.builder.build_const(TirConst::String(variant_name))?;
        self.builder
            .build_eq(tag_val, wanted_tag)
            .map_err(|e| e.into())
    }

    fn lower_narrowing_if(
        &mut self,
        i: &HirIf,
        is_pat: &HirIsPatternExpr,
    ) -> Result<TirValueId, TirLowerError> {
        let (variant_name, binding_name) = self.extract_simple_pattern(&is_pat.pattern)?;

        // Subject must be a name for narrowing in this smoke phase
        if !matches!(&*is_pat.value, HirExpression::Name(_, _)) {
            return Err(TirLowerError::UnsupportedExpr(
                "is narrowing subject must be a variable in this phase".into(),
                is_pat.value.span(),
            ));
        };

        let subject_val = self.lower_expression(&is_pat.value)?;
        let tag_val = self.builder.build_tag(subject_val)?;
        let wanted_tag = self
            .builder
            .build_const(TirConst::String(variant_name.clone()))?;
        let is_match = self.builder.build_eq(tag_val, wanted_tag)?;

        let mut t_block = self.builder.new_block();
        let mut f_block = self.builder.new_block();
        self.builder.build_condbr(is_match, t_block, f_block)?;

        // Save locals for scoping
        let old_locals = self.locals.clone();

        // Then block
        self.builder.set_current_block(t_block);
        if let Some(binding) = binding_name {
            let payload = self
                .builder
                .build_unwrap(subject_val, variant_name.clone(), 0)?;

            // Infer type of payload
            let subject_ty = self.infer_expr_type(&is_pat.value)?;
            let payload_ty = match subject_ty {
                Type::Option(inner) => *inner,
                Type::Result(ok, err) => {
                    if variant_name == "Ok" {
                        *ok
                    } else {
                        *err
                    }
                }
                _ => Type::Unknown,
            };
            let tir_payload_ty = map_type(&payload_ty)?;

            let slot = self
                .builder
                .build_slot(tir_payload_ty.clone(), Some(&binding))?;
            self.builder.build_store(slot, payload)?;
            self.locals.insert(binding, (slot, tir_payload_ty, false));
        }

        for stmt in &i.then_block.statements {
            self.lower_statement(stmt)?;
        }
        t_block = self.builder.get_current_block().unwrap();
        let t_terminated = self.builder.is_terminated();

        // Restore locals after then block (bindings should not leak)
        self.locals = old_locals;

        // Else block
        self.builder.set_current_block(f_block);
        if let Some(else_expr) = &i.else_branch {
            if let HirExpression::Block(b) = &**else_expr {
                for stmt in &b.statements {
                    self.lower_statement(stmt)?;
                }
            } else {
                return Err(TirLowerError::UnsupportedExpr(
                    "Else is not a block".into(),
                    else_expr.span(),
                ));
            }
        }
        f_block = self.builder.get_current_block().unwrap();
        let f_terminated = self.builder.is_terminated();

        if t_terminated && f_terminated {
            // Both branches terminate. We still create a join block so that
            // subsequent instructions (which are unreachable) can be appended
            // without causing a "block already terminated" error.
            let end_block = self.builder.new_block();
            self.builder.set_current_block(end_block);
            Ok(TirValueId(0))
        } else {
            let end_block = self.builder.new_block();

            if !t_terminated {
                self.builder.set_current_block(t_block);
                self.builder.build_br(end_block)?;
            }
            if !f_terminated {
                self.builder.set_current_block(f_block);
                self.builder.build_br(end_block)?;
            }

            self.builder.set_current_block(end_block);
            self.builder
                .build_const(TirConst::Unit)
                .map_err(|e| e.into())
        }
    }

    fn extract_simple_pattern(
        &self,
        pattern: &HirPattern,
    ) -> Result<(String, Option<String>), TirLowerError> {
        match pattern {
            HirPattern::EnumCase(_, name, args) => {
                if args.is_empty() {
                    Ok((name.clone(), None))
                } else if args.len() == 1 {
                    if let HirPattern::Identifier(_, binding) = &args[0] {
                        Ok((name.clone(), Some(binding.clone())))
                    } else {
                        Err(TirLowerError::UnsupportedPattern(
                            "only identifier bindings are supported in this phase".into(),
                            pattern.span(),
                        ))
                    }
                } else {
                    Err(TirLowerError::UnsupportedPattern(
                        "multi-payload patterns not supported in this phase".into(),
                        pattern.span(),
                    ))
                }
            }
            HirPattern::EnumConstructor(e) => {
                if e.payloads.is_empty() {
                    Ok((e.variant_name.clone(), None))
                } else if e.payloads.len() == 1 {
                    if let HirPattern::Identifier(_, binding) = &e.payloads[0] {
                        Ok((e.variant_name.clone(), Some(binding.clone())))
                    } else {
                        Err(TirLowerError::UnsupportedPattern(
                            "only identifier bindings are supported in this phase".into(),
                            pattern.span(),
                        ))
                    }
                } else {
                    Err(TirLowerError::UnsupportedPattern(
                        "multi-payload patterns not supported in this phase".into(),
                        pattern.span(),
                    ))
                }
            }
            _ => Err(TirLowerError::UnsupportedPattern(
                format!("Pattern type {:?} not supported", pattern),
                pattern.span(),
            )),
        }
    }

    pub fn lower_expression_base(
        &mut self,
        expr: &HirExpression,
    ) -> Result<TirValueId, TirLowerError> {
        // This is just a marker for where the original code ended
        self.lower_expression(expr)
    }

    fn lower_match(&mut self, m: &HirMatch) -> Result<TirValueId, TirLowerError> {
        let scrutinee_val = self.lower_expression(&m.value)?;
        let scrutinee_ty = self.infer_expr_type(&m.value)?;

        // 1. Infer and verify result type from arms
        let mut match_ty = Type::Unknown;
        for arm in &m.arms {
            if arm.guard.is_some() {
                return Err(TirLowerError::UnsupportedExpr(
                    "match guards are not yet supported in experimental VM".into(),
                    arm.value.span(),
                ));
            }

            // Temporary bindings for inference
            let old_locals_inf = self.locals.clone();
            self.bind_pattern_for_inference(&arm.pattern, &scrutinee_ty)?;

            let arm_body_ty = self.infer_expr_type(&arm.value)?;
            self.locals = old_locals_inf;

            if match_ty == Type::Unknown {
                match_ty = arm_body_ty;
            } else if arm_body_ty != match_ty
                && arm_body_ty != Type::Never
                && match_ty != Type::Never
            {
                return Err(TirLowerError::TypeMismatch {
                    expected: match_ty.to_string(),
                    found: arm_body_ty.to_string(),
                    span: arm.value.span(),
                });
            } else if match_ty == Type::Never && arm_body_ty != Type::Never {
                match_ty = arm_body_ty;
            }
        }

        if match_ty == Type::Unknown {
            return Err(TirLowerError::UnsupportedExpr(
                "Match must have at least one arm".into(),
                m.span,
            ));
        }

        let tir_match_ty = map_type(&match_ty)?;
        let res_slot = self
            .builder
            .build_slot(tir_match_ty.clone(), Some("__match_result"))?;
        let exit_block = self.builder.new_block();

        // 2. Track exhaustiveness
        let mut seen_variants = std::collections::HashSet::new();
        let mut has_wildcard = false;

        // 3. Sequential checks
        for arm in &m.arms {
            if has_wildcard {
                break;
            }

            let then_block = self.builder.new_block();
            let next_arm_block = self.builder.new_block();

            match &arm.pattern {
                HirPattern::Wildcard(_) => {
                    has_wildcard = true;
                    self.builder.build_br(then_block)?;
                }
                HirPattern::Identifier(_span, _name) => {
                    has_wildcard = true;
                    self.builder.build_br(then_block)?;
                }
                HirPattern::Literal(_, lit_expr) => {
                    let lit_val = self.lower_expression(lit_expr)?;
                    let cond = self.builder.build_eq(scrutinee_val, lit_val)?;
                    self.builder
                        .build_condbr(cond, then_block, next_arm_block)?;

                    if let HirExpression::Bool(_, b) = &**lit_expr {
                        seen_variants.insert(b.to_string());
                    }
                }
                HirPattern::EnumConstructor(e) => {
                    let tag = self.builder.build_tag(scrutinee_val)?;
                    let wanted = self
                        .builder
                        .build_const(TirConst::String(e.variant_name.clone()))?;
                    let cond = self.builder.build_eq(tag, wanted)?;
                    self.builder
                        .build_condbr(cond, then_block, next_arm_block)?;
                    seen_variants.insert(e.variant_name.clone());
                }
                HirPattern::EnumCase(_, name, _) => {
                    let tag = self.builder.build_tag(scrutinee_val)?;
                    let wanted = self.builder.build_const(TirConst::String(name.clone()))?;
                    let cond = self.builder.build_eq(tag, wanted)?;
                    self.builder
                        .build_condbr(cond, then_block, next_arm_block)?;
                    seen_variants.insert(name.clone());
                }
                _ => {
                    return Err(TirLowerError::UnsupportedPattern(
                        format!(
                            "Pattern {:?} is not yet supported in experimental VM",
                            arm.pattern
                        ),
                        arm.pattern.span(),
                    ))
                }
            }

            // Body
            self.builder.set_current_block(then_block);
            let old_locals = self.locals.clone();

            // Bindings
            match &arm.pattern {
                HirPattern::Identifier(_span, name) => {
                    let tir_scrutinee_ty = map_type(&scrutinee_ty)?;
                    let slot = self
                        .builder
                        .build_slot(tir_scrutinee_ty.clone(), Some(name))?;
                    self.builder.build_store(slot, scrutinee_val)?;
                    self.locals
                        .insert(name.clone(), (slot, tir_scrutinee_ty, false));
                }
                HirPattern::EnumConstructor(e) => {
                    for (i, payload_pat) in e.payloads.iter().enumerate() {
                        match payload_pat {
                            HirPattern::Identifier(_span, name) => {
                                let payload_val = self.builder.build_unwrap(scrutinee_val, e.variant_name.clone(), i as u16)?;
                                let payload_ty = self.infer_payload_type(&scrutinee_ty, &e.variant_name, i)?;
                                let tir_payload_ty = map_type(&payload_ty)?;
                                let slot = self.builder.build_slot(tir_payload_ty.clone(), Some(name))?;
                                self.builder.build_store(slot, payload_val)?;
                                self.locals.insert(name.clone(), (slot, tir_payload_ty, false));
                            }
                            HirPattern::Wildcard(_) => {}
                            _ => return Err(TirLowerError::UnsupportedPattern(format!("Only simple bindings are supported in enum patterns for now, found {:?}", payload_pat), payload_pat.span())),
                        }
                    }
                }
                HirPattern::EnumCase(_, variant_name, payloads) => {
                    for (i, payload_pat) in payloads.iter().enumerate() {
                        match payload_pat {
                            HirPattern::Identifier(_span, name) => {
                                let payload_val = self.builder.build_unwrap(scrutinee_val, variant_name.clone(), i as u16)?;
                                let payload_ty = self.infer_payload_type(&scrutinee_ty, variant_name, i)?;
                                let tir_payload_ty = map_type(&payload_ty)?;
                                let slot = self.builder.build_slot(tir_payload_ty.clone(), Some(name))?;
                                self.builder.build_store(slot, payload_val)?;
                                self.locals.insert(name.clone(), (slot, tir_payload_ty, false));
                            }
                            HirPattern::Wildcard(_) => {}
                            _ => return Err(TirLowerError::UnsupportedPattern(format!("Only simple bindings are supported in enum patterns for now, found {:?}", payload_pat), payload_pat.span())),
                        }
                    }
                }
                _ => {}
            }

            let arm_val = self.lower_expression(&arm.value)?;
            if !self.builder.is_terminated() {
                if match_ty != Type::Never {
                    self.builder.build_store(res_slot, arm_val)?;
                }
                self.builder.build_br(exit_block)?;
            }
            self.locals = old_locals;

            self.builder.set_current_block(next_arm_block);
        }

        // 4. Exhaustiveness check
        self.check_match_exhaustiveness(&scrutinee_ty, &seen_variants, has_wildcard, m.span)?;

        if !self.builder.is_terminated() {
            self.builder.build_unreachable()?;
        }

        self.builder.set_current_block(exit_block);
        self.builder.build_load(res_slot).map_err(|e| e.into())
    }

    fn infer_payload_type(
        &self,
        enum_ty: &Type,
        variant_name: &str,
        index: usize,
    ) -> Result<Type, TirLowerError> {
        match enum_ty {
            Type::Option(inner) => {
                if variant_name == "Some" && index == 0 {
                    return Ok(inner.as_ref().clone());
                }
            }
            Type::Result(ok, err) => {
                if variant_name == "Ok" && index == 0 {
                    return Ok(ok.as_ref().clone());
                }
                if variant_name == "Err" && index == 0 {
                    return Ok(err.as_ref().clone());
                }
            }
            Type::Enum(name, args) => {
                if name == "Option" && args.len() == 1 {
                    if variant_name == "Some" && index == 0 {
                        return Ok(args[0].clone());
                    }
                    if variant_name == "None" {
                        return Ok(Type::Unit);
                    }
                }
                if name == "Result" && args.len() == 2 {
                    if variant_name == "Ok" && index == 0 {
                        return Ok(args[0].clone());
                    }
                    if variant_name == "Err" && index == 0 {
                        return Ok(args[1].clone());
                    }
                }

                if let Some(info) = self.type_ctx.enums.get(name) {
                    if let Some(payloads) = info.variants.get(variant_name) {
                        if index < payloads.len() {
                            return Ok(payloads[index].clone());
                        }
                    }
                }
            }
            _ => {}
        }
        Err(TirLowerError::UnsupportedType(format!(
            "cannot infer payload type for {}::{} at index {}",
            enum_ty, variant_name, index
        )))
    }

    fn check_match_exhaustiveness(
        &self,
        ty: &Type,
        seen: &std::collections::HashSet<String>,
        has_wildcard: bool,
        span: tempra_span::Span,
    ) -> Result<(), TirLowerError> {
        if has_wildcard {
            return Ok(());
        }

        match ty {
            Type::Bool => {
                if seen.contains("true") && seen.contains("false") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr(
                    "Match on Bool must be exhaustive (true AND false, or wildcard)".into(),
                    span,
                ))
            }
            Type::Option(_) => {
                if seen.contains("Some") && seen.contains("None") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr(
                    "Match on Option must be exhaustive (Some AND None, or wildcard)".into(),
                    span,
                ))
            }
            Type::Result(_, _) => {
                if seen.contains("Ok") && seen.contains("Err") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr(
                    "Match on Result must be exhaustive (Ok AND Err, or wildcard)".into(),
                    span,
                ))
            }
            _ => Err(TirLowerError::UnsupportedExpr(
                format!(
                    "Non-exhaustive match for type {}; wildcard '_' is required",
                    ty
                ),
                span,
            )),
        }
    }

    fn bind_pattern_for_inference(
        &mut self,
        pattern: &HirPattern,
        scrutinee_ty: &Type,
    ) -> Result<(), TirLowerError> {
        match pattern {
            HirPattern::Identifier(_, name) => {
                let tir_ty = map_type(scrutinee_ty)?;
                self.locals
                    .insert(name.clone(), (TirSlotId(0), tir_ty, false));
            }
            HirPattern::EnumConstructor(e) => {
                for (i, payload_pat) in e.payloads.iter().enumerate() {
                    match payload_pat {
                        HirPattern::Identifier(_, name) => {
                            let payload_ty =
                                self.infer_payload_type(scrutinee_ty, &e.variant_name, i)?;
                            let tir_payload_ty = map_type(&payload_ty)?;
                            self.locals
                                .insert(name.clone(), (TirSlotId(0), tir_payload_ty, false));
                        }
                        HirPattern::Wildcard(_) => {}
                        _ => {
                            return Err(TirLowerError::UnsupportedPattern(
                                "Nested patterns are not yet supported in experimental VM".into(),
                                payload_pat.span(),
                            ))
                        }
                    }
                }
            }
            HirPattern::EnumCase(_, variant_name, payloads) => {
                for (i, payload_pat) in payloads.iter().enumerate() {
                    match payload_pat {
                        HirPattern::Identifier(_, name) => {
                            let payload_ty =
                                self.infer_payload_type(scrutinee_ty, variant_name, i)?;
                            let tir_payload_ty = map_type(&payload_ty)?;
                            self.locals
                                .insert(name.clone(), (TirSlotId(0), tir_payload_ty, false));
                        }
                        HirPattern::Wildcard(_) => {}
                        _ => {
                            return Err(TirLowerError::UnsupportedPattern(
                                "Nested patterns are not yet supported in experimental VM".into(),
                                payload_pat.span(),
                            ))
                        }
                    }
                }
            }
            _ => return Ok(()),
        }
        Ok(())
    }
}

pub fn lower_function(
    func: &HirFunction,
    type_ctx: &TypeContext,
    module_builder: &mut ModuleBuilder,
    self_ty: Option<Type>,
    module_prefix: Option<&str>,
) -> Result<TirFunction, TirLowerError> {
    lower_function_inner(
        func,
        type_ctx,
        module_builder,
        self_ty,
        module_prefix,
        None,
        HashMap::new(),
    )
}

pub fn lower_function_specialization(
    func: &HirFunction,
    type_ctx: &TypeContext,
    module_builder: &mut ModuleBuilder,
    self_ty: Option<Type>,
    module_prefix: Option<&str>,
    type_args: &[Type],
) -> Result<TirFunction, TirLowerError> {
    let mut generic_subs = HashMap::new();
    for (generic, ty) in func.generic_parameters.iter().zip(type_args.iter()) {
        generic_subs.insert(generic.name.clone(), ty.clone());
    }

    let base_name = if let Some(p) = module_prefix {
        if p.is_empty() {
            func.name.clone()
        } else {
            format!("{}.{}", p, func.name)
        }
    } else {
        func.name.clone()
    };
    let specialized_name = specialized_function_name(&base_name, type_args);

    lower_function_inner(
        func,
        type_ctx,
        module_builder,
        self_ty,
        module_prefix,
        Some(specialized_name),
        generic_subs,
    )
}

fn lower_function_inner(
    func: &HirFunction,
    type_ctx: &TypeContext,
    module_builder: &mut ModuleBuilder,
    self_ty: Option<Type>,
    module_prefix: Option<&str>,
    override_name: Option<String>,
    generic_subs: HashMap<String, Type>,
) -> Result<TirFunction, TirLowerError> {
    let ret_ty = if let Some(rt) = &func.return_type {
        let ty = resolve_hir_type(rt, type_ctx, self_ty.as_ref(), &generic_subs)?;
        map_type(&ty)?
    } else {
        TirType::Unit
    };

    let qual_name = if let Some(name) = override_name {
        name
    } else if let Some(p) = module_prefix {
        if p.is_empty() {
            func.name.clone()
        } else {
            format!("{}.{}", p, func.name)
        }
    } else {
        func.name.clone()
    };

    let mut builder = module_builder.new_function(&qual_name, ret_ty);
    builder.func.is_extern = func.is_extern;

    // Extern functions: emit params + empty entry block with Ret
    if func.is_extern {
        let entry_block = builder.new_block();
        builder.set_current_block(entry_block);
        for p in &func.parameters {
            if p.kind == tempra_ast::ParameterKind::Type {
                continue;
            }
            let ty = resolve_hir_type(&p.ty, type_ctx, self_ty.as_ref(), &generic_subs)?;
            let tir_ty = map_type(&ty)?;
            builder.add_param(tir_ty);
        }
        if builder.func.return_type == TirType::Unit {
            builder.build_ret(None)?;
        } else {
            builder.build_unreachable()?;
        }
        return Ok(builder.build());
    }

    let mut ctx = LowerContext::new(
        builder,
        type_ctx,
        module_builder,
        self_ty.clone(),
        generic_subs.clone(),
    );

    // Create entry block
    let entry_block = ctx.builder.new_block();
    ctx.builder.set_current_block(entry_block);

    for p in &func.parameters {
        if p.kind == tempra_ast::ParameterKind::Type {
            continue;
        }
        let ty = resolve_hir_type(&p.ty, type_ctx, self_ty.as_ref(), &generic_subs)?;
        let tir_ty = map_type(&ty)?;
        let val_id = ctx.builder.add_param(tir_ty.clone());

        let slot = ctx.builder.build_slot(tir_ty.clone(), Some(&p.name))?;
        ctx.builder.build_store(slot, val_id)?;
        // Parameters are immutable by default in Phase 12.2 lowering
        ctx.locals.insert(p.name.clone(), (slot, tir_ty, false));
    }

    if let Some(body) = &func.body {
        for stmt in &body.statements {
            ctx.lower_statement(stmt)?;
        }
    }

    if !ctx.builder.is_terminated() {
        if ctx.builder.func.return_type == TirType::Unit {
            ctx.builder.build_ret(None)?;
        } else {
            // If the current block is not the entry block and has no predecessors,
            // it's an unreachable block created by a fully terminating branch (e.g. if with both returns).
            // We must still terminate it for the verifier, so we use unreachable.
            let current_block_id = ctx.builder.get_current_block().unwrap();
            if current_block_id.0 == 0 || ctx.builder.has_predecessors(current_block_id) {
                let body_span = func.body.as_ref().map_or(func.span, |b| b.span);
                return Err(TirLowerError::MissingReturn(body_span));
            } else {
                // Unreachable join block, terminate it
                ctx.builder.build_unreachable()?;
            }
        }
    }

    Ok(ctx.builder.build())
}
