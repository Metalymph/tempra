use crate::error::TirLowerError;
use std::collections::HashMap;
use tempra_hir::Type;
use tempra_hir::*;
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
        Type::BuiltinError => Err(TirLowerError::UnsupportedType("BuiltinError".to_string())),
        Type::Namespace(_) => Err(TirLowerError::UnsupportedType("Namespace".to_string())),
        Type::Function(_, _) => Err(TirLowerError::UnsupportedType("Function".to_string())),
        Type::GenericParam { .. } => Err(TirLowerError::UnsupportedType("GenericParam".to_string())),
        Type::Meta(_) => Err(TirLowerError::UnsupportedType("Meta".to_string())),
        Type::Projection { .. } => Err(TirLowerError::UnsupportedType("Projection".to_string())),
        Type::Interface(_) => Err(TirLowerError::UnsupportedType("Interface".to_string())),
        Type::ArrayIter(_) => Err(TirLowerError::UnsupportedType("ArrayIter".to_string())),
        Type::SliceIter(_) => Err(TirLowerError::UnsupportedType("SliceIter".to_string())),
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

/// Resolve an HirType to a semantic Type. Minimal — only handles what Phase 14.4 needs.
fn resolve_hir_type(hir_ty: &HirType, _type_ctx: &TypeContext, self_ty: Option<&Type>) -> Result<Type, TirLowerError> {
    match hir_ty {
        HirType::Named(_, name) => {
            let ty = tempra_typeck::types::resolve_named_type(name);
            if ty == Type::Unknown {
                if let Some(info) = _type_ctx.structs.get(name) {
                    Ok(Type::Struct(name.clone(), info.generic_params.iter().map(|p| Type::GenericParam { id: p.id, name: p.name.clone(), constraint: p.constraint.clone() }).collect()))
                } else if let Some(info) = _type_ctx.enums.get(name) {
                    Ok(Type::Enum(name.clone(), info.generic_params.iter().map(|p| Type::GenericParam { id: p.id, name: p.name.clone(), constraint: p.constraint.clone() }).collect()))
                } else {
                    Err(TirLowerError::UnsupportedType(format!("Unknown type name: {}", name)))
                }
            } else {
                Ok(ty)
            }
        }
        HirType::Result(_, ok_hir, err_hir) => {
            let ok_ty = resolve_hir_type(ok_hir, _type_ctx, self_ty)?;
            let err_ty = resolve_hir_type(err_hir, _type_ctx, self_ty)?;
            Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
        }
        HirType::Optional(_, inner_hir) => {
            let inner_ty = resolve_hir_type(inner_hir, _type_ctx, self_ty)?;
            Ok(Type::Option(Box::new(inner_ty)))
        }
        HirType::Array(_, inner_hir) => {
            let inner_ty = resolve_hir_type(inner_hir, _type_ctx, self_ty)?;
            Ok(Type::Array(Box::new(inner_ty)))
        }
        HirType::Apply(_, callee, args) => {
            if let HirType::Named(_, name) = &**callee {
                if name == "Slice" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty)?;
                    return Ok(Type::Slice(Box::new(inner_ty)));
                }
                if name == "Array" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty)?;
                    return Ok(Type::Array(Box::new(inner_ty)));
                }
                if name == "Option" && args.len() == 1 {
                    let inner_ty = resolve_hir_type(&args[0], _type_ctx, self_ty)?;
                    return Ok(Type::Option(Box::new(inner_ty)));
                }
                if name == "Result" && args.len() == 2 {
                    let ok_ty = resolve_hir_type(&args[0], _type_ctx, self_ty)?;
                    let err_ty = resolve_hir_type(&args[1], _type_ctx, self_ty)?;
                    return Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)));
                }
            }
            Err(TirLowerError::UnsupportedType(format!("Cannot resolve HirType::Apply: {:?}", hir_ty)))
        }
        HirType::SelfType(_) => {
            if let Some(sty) = self_ty {
                Ok(sty.clone())
            } else {
                Err(TirLowerError::UnsupportedType("Self type used outside of method".to_string()))
            }
        }
        _ => Err(TirLowerError::UnsupportedType(format!("Cannot resolve HirType: {:?}", hir_ty))),
    }
}

pub struct LowerContext<'a> {
    pub builder: FunctionBuilder,
    pub type_ctx: &'a TypeContext,
    pub module_builder: &'a mut ModuleBuilder,
    pub self_ty: Option<Type>,
    /// (SlotId, Type, is_mutable)
    pub locals: HashMap<String, (TirSlotId, TirType, bool)>,
}

impl<'a> LowerContext<'a> {
    pub fn new(builder: FunctionBuilder, type_ctx: &'a TypeContext, module_builder: &'a mut ModuleBuilder, self_ty: Option<Type>) -> Self {
        Self {
            builder,
            type_ctx,
            module_builder,
            self_ty,
            locals: HashMap::new(),
        }
    }

    fn collect_locals_from_statement(&mut self, stmt: &HirStatement) -> Result<(), TirLowerError> {
        match stmt {
            HirStatement::Let(l) => {
                let ty = if let Some(ht) = &l.ty {
                    resolve_hir_type(ht, self.type_ctx, self.self_ty.as_ref())?
                } else {
                    self.infer_expr_type(&l.value)?
                };
                let tir_ty = map_type(&ty)?;
                self.locals.insert(l.name.clone(), (TirSlotId(0), tir_ty, false));
            }
            HirStatement::Var(v) => {
                let ty = if let Some(ht) = &v.ty {
                    resolve_hir_type(ht, self.type_ctx, self.self_ty.as_ref())?
                } else {
                    self.infer_expr_type(&v.value)?
                };
                let tir_ty = map_type(&ty)?;
                self.locals.insert(v.name.clone(), (TirSlotId(0), tir_ty, true));
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
                } else if let Some((_, tir_ty, _)) = self.locals.get(name) {
                    unmap_type(tir_ty)
                } else {
                    Err(TirLowerError::UnknownLocal(name.clone(), *span))
                }
            }
            HirExpression::Binary(b) => {
                match b.op {
                    HirBinaryOp::Add | HirBinaryOp::Sub | HirBinaryOp::Mul | HirBinaryOp::Div => {
                        self.infer_expr_type(&b.left)
                    }
                    HirBinaryOp::Eq | HirBinaryOp::NotEq | HirBinaryOp::Less | HirBinaryOp::LessEq | HirBinaryOp::Greater | HirBinaryOp::GreaterEq | HirBinaryOp::And | HirBinaryOp::Or => Ok(Type::Bool),
                    _ => Err(TirLowerError::UnsupportedExpr("binary op type".into(), b.span)),
                }
            }
            HirExpression::Unary(u) => {
                if u.op == HirUnaryOp::Not { Ok(Type::Bool) } else { self.infer_expr_type(&u.expr) }
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
                    _ => Err(TirLowerError::UnsupportedExpr("Indexing non-array/slice".into(), i.span)),
                }
            }
            HirExpression::Slice(s) => {
                let obj_ty = self.infer_expr_type(&s.object)?;
                match obj_ty {
                    Type::Array(inner) | Type::Slice(inner) => Ok(Type::Slice(inner)),
                    _ => Err(TirLowerError::UnsupportedExpr("Slicing non-array/slice".into(), s.span)),
                }
            }
            HirExpression::Member(m) => {
                let obj_ty = self.infer_expr_type(&m.object)?;
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
                Err(TirLowerError::UnsupportedExpr(format!("Member access: {}", m.field), m.span))
            }
            HirExpression::StructLiteral(s) => {
                let ty = resolve_hir_type(&s.ty, self.type_ctx, self.self_ty.as_ref())?;
                Ok(ty)
            }
            HirExpression::Call(c) => {
                if let HirExpression::Member(m) = &*c.callee {
                    if m.field == "len" || m.field == "size" {
                        return Ok(Type::Int64);
                    }
                    if m.field == "bytes" {
                        return Ok(Type::Slice(Box::new(Type::UInt8)));
                    }

                    let obj_ty = self.infer_expr_type(&m.object)?;
                    if let Some(sig) = self.type_ctx.lookup_direct_method_signature(&obj_ty, &m.field) {
                        return Ok(sig.return_type.clone());
                    }
                }
                if let HirExpression::Name(_, name) = &*c.callee {
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
                                Err(TirLowerError::UnsupportedExpr(format!("Cannot infer call type for {}", name), c.span))
                            }
                        }
                    }
                } else {
                    Err(TirLowerError::UnsupportedExpr("Cannot infer call type for complex callee".into(), c.span))
                }
            }
            HirExpression::Try(e) => {
                let inner_ty = self.infer_expr_type(&e.expr)?;
                match inner_ty {
                    Type::Result(ok, _) => Ok(*ok),
                    _ => Err(TirLowerError::UnsupportedExpr(
                        "'?' operator expects a Result type".into(), e.span,
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
                self.locals.insert(f.binding.clone(), (TirSlotId(0), tir_element_ty, false));
                let body_ty = self.infer_block_type(&f.body)?;
                self.locals = old_locals;
                Ok(Type::Array(Box::new(body_ty)))
            }
            _ => Err(TirLowerError::UnsupportedExpr(format!("{:?}", expr), expr.span())),
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
                    resolve_hir_type(hir_ty, self.type_ctx, self.self_ty.as_ref())?
                } else {
                    self.infer_expr_type(&l.value)?
                };
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&l.name))?;
                self.locals.insert(l.name.clone(), (slot, tir_ty.clone(), false));

                let val = match &l.value {
                    HirExpression::Name(_, name) if name == "None" => {
                        self.builder.build_make(tir_ty.clone(), Some("None".to_string()), vec![])?
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
                    resolve_hir_type(hir_ty, self.type_ctx, self.self_ty.as_ref())?
                } else {
                    self.infer_expr_type(&v.value)?
                };
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&v.name))?;
                self.locals.insert(v.name.clone(), (slot, tir_ty.clone(), true));

                let val = match &v.value {
                    HirExpression::Name(_, name) if name == "None" => {
                        self.builder.build_make(tir_ty.clone(), Some("None".to_string()), vec![])?
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
            HirStatement::For(f) => {
                self.lower_for(f)
            }
            _ => Err(TirLowerError::UnsupportedStmt(format!("{:?}", stmt), stmt.span())),
        }
    }

    fn lower_for(&mut self, f: &HirFor) -> Result<(), TirLowerError> {
        let iterable_ty = self.infer_expr_type(&f.iterable)?;
        if !matches!(iterable_ty, Type::Array(_) | Type::Slice(_)) {
            return Err(TirLowerError::UnsupportedExpr(
                "for-in currently only supports Array/Slice in experimental VM".into(),
                f.iterable.span(),
            ));
        }

        let element_ty = match &iterable_ty {
            Type::Array(inner) | Type::Slice(inner) => inner.as_ref().clone(),
            _ => unreachable!(),
        };

        // 1. Lower iterable and store in a hidden slot
        let iter_val = self.lower_expression(&f.iterable)?;
        let tir_iterable_ty = map_type(&iterable_ty)?;
        let iter_obj_slot = self.builder.build_slot(tir_iterable_ty, Some(&format!("$iter_obj_{}", f.span.start)))?;
        self.builder.build_store(iter_obj_slot, iter_val)?;

        // 2. Initialize index slot
        let zero = self.builder.build_const(TirConst::Int64(0))?;
        let idx_slot = self.builder.build_slot(TirType::Int64, Some(&format!("$idx_{}", f.span.start)))?;
        self.builder.build_store(idx_slot, zero)?;

        // 3. Initialize length slot
        let loaded_iter = self.builder.build_load(iter_obj_slot)?;
        let len_val = self.builder.build_len(loaded_iter)?;
        let len_slot = self.builder.build_slot(TirType::Int64, Some(&format!("$len_{}", f.span.start)))?;
        self.builder.build_store(len_slot, len_val)?;

        // 4. Create blocks
        let cond_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Branch to cond
        self.builder.build_br(cond_block)?;

        // 5. Condition Block
        self.builder.set_current_block(cond_block);
        let curr_idx = self.builder.build_load(idx_slot)?;
        let curr_len = self.builder.build_load(len_slot)?;
        let is_less = self.builder.build_lt(curr_idx, curr_len)?;
        self.builder.build_condbr(is_less, body_block, exit_block)?;

        // 6. Body Block
        self.builder.set_current_block(body_block);
        
        // Load element
        let iter_obj_for_get = self.builder.build_load(iter_obj_slot)?;
        let idx_for_get = self.builder.build_load(idx_slot)?;
        let element_val = self.builder.build_get_index(iter_obj_for_get, idx_for_get)?;
        
        // Scope for loop variable
        let old_locals = self.locals.clone();
        
        let tir_element_ty = map_type(&element_ty)?;
        let x_slot = self.builder.build_slot(tir_element_ty.clone(), Some(&f.binding))?;
        self.builder.build_store(x_slot, element_val)?;
        self.locals.insert(f.binding.clone(), (x_slot, tir_element_ty, false));

        // Lower body statements
        for stmt in &f.body.statements {
            self.lower_statement(stmt)?;
        }
        
        // Restore locals scope
        self.locals = old_locals;

        // Increment index
        if !self.builder.is_terminated() {
            let idx_to_inc = self.builder.build_load(idx_slot)?;
            let one = self.builder.build_const(TirConst::Int64(1))?;
            let next_idx = self.builder.build_add(idx_to_inc, one)?;
            self.builder.build_store(idx_slot, next_idx)?;
            self.builder.build_br(cond_block)?;
        }

        // 7. Exit Block
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
            self.builder.build_const(TirConst::Unit).map_err(|e| e.into())
        }
    }

    fn lower_for_expr(&mut self, f: &HirForExpr) -> Result<TirValueId, TirLowerError> {
        let iterable_ty = self.infer_expr_type(&f.iterable)?;
        if !matches!(iterable_ty, Type::Array(_) | Type::Slice(_)) {
            return Err(TirLowerError::UnsupportedExpr(
                "for-expr currently only supports Array/Slice in experimental VM".into(),
                f.iterable.span(),
            ));
        }

        let element_ty = match &iterable_ty {
            Type::Array(inner) | Type::Slice(inner) => inner.as_ref().clone(),
            _ => unreachable!(),
        };
        
        let tir_element_ty = map_type(&element_ty)?;

        // 1. Temporarily insert binding for inference of result array type
        let old_locals_for_inf = self.locals.clone();
        // Use a dummy TirValueId(0) as it's not used during inference
        self.locals.insert(f.binding.clone(), (TirSlotId(0), tir_element_ty.clone(), false));
        let body_ty = self.infer_block_type(&f.body)?;
        self.locals = old_locals_for_inf;

        let tir_body_ty = map_type(&body_ty)?;
        let result_array_ty = TirType::Array(Box::new(tir_body_ty));

        // 2. Initialize result accumulator slot with empty array
        let empty_arr = self.builder.build_make(result_array_ty.clone(), None, vec![])?;
        let res_slot = self.builder.build_slot(result_array_ty, Some(&format!("$for_res_{}", f.span.start)))?;
        self.builder.build_store(res_slot, empty_arr)?;

        // 3. Lower iterable and store in a hidden slot
        let iter_val = self.lower_expression(&f.iterable)?;
        let tir_iterable_ty = map_type(&iterable_ty)?;
        let iter_obj_slot = self.builder.build_slot(tir_iterable_ty, Some(&format!("$iter_obj_{}", f.span.start)))?;
        self.builder.build_store(iter_obj_slot, iter_val)?;

        // 4. Initialize index slot
        let zero = self.builder.build_const(TirConst::Int64(0))?;
        let idx_slot = self.builder.build_slot(TirType::Int64, Some(&format!("$idx_{}", f.span.start)))?;
        self.builder.build_store(idx_slot, zero)?;

        // 5. Initialize length slot
        let loaded_iter = self.builder.build_load(iter_obj_slot)?;
        let len_val = self.builder.build_len(loaded_iter)?;
        let len_slot = self.builder.build_slot(TirType::Int64, Some(&format!("$len_{}", f.span.start)))?;
        self.builder.build_store(len_slot, len_val)?;

        // 6. Create blocks
        let cond_block = self.builder.new_block();
        let body_block = self.builder.new_block();
        let exit_block = self.builder.new_block();

        // Branch to cond
        self.builder.build_br(cond_block)?;

        // 7. Condition Block
        self.builder.set_current_block(cond_block);
        let curr_idx = self.builder.build_load(idx_slot)?;
        let curr_len = self.builder.build_load(len_slot)?;
        let is_less = self.builder.build_lt(curr_idx, curr_len)?;
        self.builder.build_condbr(is_less, body_block, exit_block)?;

        // 8. Body Block
        self.builder.set_current_block(body_block);
        
        // Load element
        let iter_obj_for_get = self.builder.build_load(iter_obj_slot)?;
        let idx_for_get = self.builder.build_load(idx_slot)?;
        let element_val = self.builder.build_get_index(iter_obj_for_get, idx_for_get)?;
        
        // Scope for loop variable
        let old_locals = self.locals.clone();
        
        let x_slot = self.builder.build_slot(tir_element_ty.clone(), Some(&f.binding))?;
        self.builder.build_store(x_slot, element_val)?;
        self.locals.insert(f.binding.clone(), (x_slot, tir_element_ty, false));

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
            self.builder.build_const(TirConst::Unit).map_err(TirLowerError::from)?
        };
        
        // Push result to accumulator
        let cur_res = self.builder.build_load(res_slot)?;
        let next_res = self.builder.build_array_push(cur_res, body_val)?;
        self.builder.build_store(res_slot, next_res)?;
        
        // Restore locals scope
        self.locals = old_locals;

        // Increment index
        if !self.builder.is_terminated() {
            let idx_to_inc = self.builder.build_load(idx_slot)?;
            let one = self.builder.build_const(TirConst::Int64(1))?;
            let next_idx = self.builder.build_add(idx_to_inc, one)?;
            self.builder.build_store(idx_slot, next_idx)?;
            self.builder.build_br(cond_block)?;
        }

        // 9. Exit Block
        self.builder.set_current_block(exit_block);
        self.builder.build_load(res_slot).map_err(TirLowerError::from)
    }

    pub fn lower_expression(&mut self, expr: &HirExpression) -> Result<TirValueId, TirLowerError> {
        match expr {
            HirExpression::Integer(span, s) => {
                let val = s.parse::<i64>().map_err(|_| TirLowerError::UnsupportedExpr("Invalid Int64 literal".into(), *span))?;
                self.builder.build_const(TirConst::Int64(val)).map_err(|e| e.into())
            }
            HirExpression::Bool(_, b) => {
                self.builder.build_const(TirConst::Bool(*b)).map_err(|e| e.into())
            }
            HirExpression::String(_, s) => {
                let unquoted = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    &s[1..s.len() - 1]
                } else {
                    s.as_str()
                };
                self.builder.build_const(TirConst::String(unquoted.to_string())).map_err(|e| e.into())
            }
            HirExpression::Name(span, name) => {
                if name == "None" {
                    let ty = self.infer_expr_type(expr)?;
                    let tir_ty = map_type(&ty)?;
                    return self.builder.build_make(tir_ty, Some("None".to_string()), vec![])
                        .map_err(|e| e.into());
                }
                if let Some((slot, _ty, _)) = self.locals.get(name) {
                    self.builder.build_load(*slot).map_err(|e| e.into())
                } else {
                    let keys: Vec<_> = self.locals.keys().cloned().collect();
                    Err(TirLowerError::UnsupportedExpr(format!("Unknown local: {}. Available: {:?}", name, keys), *span))
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
                            return Err(TirLowerError::UnsupportedStmt("cannot assign to let binding".into(), *span));
                        }

                        let val = self.lower_expression(&b.right)?;
                        self.builder.build_store(slot, val)?;
                        return self.builder.build_const(TirConst::Unit).map_err(|e| e.into());
                    } else if let HirExpression::Member(m) = &*b.left {
                        if let HirExpression::Name(span, name) = &*m.object {
                            let (slot, _ty, mutable) = if let Some(info) = self.locals.get(name) {
                                info.clone()
                            } else {
                                return Err(TirLowerError::UnknownLocal(name.clone(), *span));
                            };
                            if !mutable {
                                return Err(TirLowerError::UnsupportedStmt("cannot assign to let binding".into(), *span));
                            }
                            let val = self.lower_expression(&b.right)?;
                            let obj = self.lower_expression(&m.object)?;
                            let new_obj = self.builder.build_set(obj, tempra_tir::ir::TirKey::Field(m.field.clone()), val)?;
                            self.builder.build_store(slot, new_obj)?;
                            return self.builder.build_const(TirConst::Unit).map_err(|e| e.into());
                        } else {
                            return Err(TirLowerError::UnsupportedStmt("nested struct assignment not supported in v0".into(), b.span));
                        }
                    } else {
                        return Err(TirLowerError::UnsupportedStmt("non-local/non-member assignment not supported".into(), b.span));
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
                    HirBinaryOp::Greater => self.builder.build_gt(left, right).map_err(|e| e.into()),
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
                    _ => Err(TirLowerError::UnsupportedExpr(format!("Binary {:?}", b.op), b.span)),
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
                let t_block = self.builder.new_block();
                let f_block = self.builder.new_block();
                
                self.builder.build_condbr(cond, t_block, f_block)?;
                
                // Then block
                self.builder.set_current_block(t_block);
                for stmt in &i.then_block.statements {
                    self.lower_statement(stmt)?;
                }
                let t_terminated = self.builder.is_terminated();

                // Else block
                self.builder.set_current_block(f_block);
                if let Some(else_expr) = &i.else_branch {
                    if let HirExpression::Block(b) = &**else_expr {
                        for stmt in &b.statements {
                            self.lower_statement(stmt)?;
                        }
                    } else {
                        return Err(TirLowerError::UnsupportedExpr("Else is not a block".into(), else_expr.span()));
                    }
                }
                let f_terminated = self.builder.is_terminated();

                if t_terminated && f_terminated {
                    // Both branches terminate, no need for join block
                    // We return a dummy value since this path is unreachable anyway
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
                    self.builder.build_const(TirConst::Unit).map_err(|e| e.into())
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
                        }
                    }

                    // Static method call resolution
                    if let Some(sig) = self.type_ctx.lookup_direct_method_signature(&obj_ty, &m.field) {
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
                            return Err(TirLowerError::UnsupportedExpr(format!("wrong arity for method {}: expected {}, found {}", full_name, sig.params.len(), arg_values.len()), c.span));
                        }

                        let func_id = self.module_builder.declare_function(&full_name);
                        return self.builder.build_call(func_id, arg_values).map_err(|e| e.into());
                    }
                }

                if let HirExpression::Name(span, name) = &*c.callee {
                    if matches!(name.as_str(), "Some" | "Ok" | "Err") {
                        if c.args.len() != 1 {
                            return Err(TirLowerError::UnsupportedExpr(format!("wrong arity for {}", name), *span));
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
                        return self.builder.build_make(ty, Some(name.clone()), vec![arg_val]).map_err(|e| e.into());
                    }

                    if let Some(sig) = self.type_ctx.functions.get(name) {
                        if sig.params.len() != c.args.len() {
                            return Err(TirLowerError::UnsupportedExpr(format!("wrong arity for call to {}: expected {}, found {}", name, sig.params.len(), c.args.len()), c.span));
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
                        return self.builder.build_call(func_id, arg_values).map_err(|e| e.into());
                    } else {
                        return Err(TirLowerError::UnknownFunction(name.clone(), *span));
                    }
                }
                Err(TirLowerError::UnsupportedExpr("Complex call resolution not supported in Phase 12.2".into(), c.span))
            }
            HirExpression::Try(e) => {
                // Desugar expr? into: tag/eq/condbr/unwrap/make/ret
                // Step 1: Lower the inner expression
                let r_val = self.lower_expression(&e.expr)?;

                // Step 2: Store in a slot so both branches can access it
                let tmp_slot = self.builder.build_slot(TirType::Result(Box::new(TirType::Unit), Box::new(TirType::Unit)), Some("__try_tmp"))?;
                self.builder.build_store(tmp_slot, r_val)?;

                // Step 3: Tag the result and compare to "Err"
                let r_for_tag = self.builder.build_load(tmp_slot)?;
                let tag_val = self.builder.build_tag(r_for_tag)?;
                let err_str = self.builder.build_const(TirConst::String("Err".to_string()))?;
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
                let err_out = self.builder.build_make(fn_ret_ty, Some("Err".to_string()), vec![err_payload])?;
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
                    _ => return Err(TirLowerError::UnsupportedExpr("StructLiteral with non-struct type".into(), s.span)),
                };
                self.builder.build_make_struct(struct_name, field_vals).map_err(|e| e.into())
            }
            HirExpression::IsPattern(i) => self.lower_is_pattern(i),
            HirExpression::Array(_, elements) => {
                let mut vals = Vec::new();
                for e in elements {
                    vals.push(self.lower_expression(e)?);
                }
                let hir_ty = self.infer_expr_type(expr)?;
                let tir_ty = map_type(&hir_ty)?;
                self.builder.build_make(tir_ty, None, vals).map_err(|e| e.into())
            }
            HirExpression::Index(i) => {
                let obj_ty = self.infer_expr_type(&i.object)?;
                if matches!(obj_ty, Type::String) {
                    return Err(TirLowerError::UnsupportedExpr("String indexing is not supported in the experimental VM".into(), i.span));
                }
                let obj = self.lower_expression(&i.object)?;
                let idx = self.lower_expression(&i.index)?;
                self.builder.build_get_index(obj, idx).map_err(|e| e.into())
            }
            HirExpression::Slice(s) => {
                let obj_ty = self.infer_expr_type(&s.object)?;
                if matches!(obj_ty, Type::String) {
                    return Err(TirLowerError::UnsupportedExpr("String slicing is not supported in the experimental VM".into(), s.span));
                }
                let obj = self.lower_expression(&s.object)?;
                let start = self.lower_expression(&s.start)?;
                let end = self.lower_expression(&s.end)?;
                self.builder.build_slice(obj, start, end).map_err(|e| e.into())
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
                    return self.builder.build_get(obj, tempra_tir::ir::TirKey::Field(m.field.clone())).map_err(|e| e.into());
                }
                
                Err(TirLowerError::UnsupportedExpr(format!("Member access: {}", m.field), m.span))
            }
            HirExpression::Block(b) => self.lower_block_expr(b),
            HirExpression::For(f) => self.lower_for_expr(f),
            HirExpression::Match(m) => self.lower_match(m),
            _ => Err(TirLowerError::UnsupportedExpr(format!("Unsupported expression: {:?}", expr), expr.span())),
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
        self.builder.build_eq(tag_val, wanted_tag).map_err(|e| e.into())
    }

    fn lower_narrowing_if(&mut self, i: &HirIf, is_pat: &HirIsPatternExpr) -> Result<TirValueId, TirLowerError> {
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
        let wanted_tag = self.builder.build_const(TirConst::String(variant_name.clone()))?;
        let is_match = self.builder.build_eq(tag_val, wanted_tag)?;

        let t_block = self.builder.new_block();
        let f_block = self.builder.new_block();
        self.builder.build_condbr(is_match, t_block, f_block)?;

        // Save locals for scoping
        let old_locals = self.locals.clone();

        // Then block
        self.builder.set_current_block(t_block);
        if let Some(binding) = binding_name {
            let payload = self.builder.build_unwrap(subject_val, variant_name.clone(), 0)?;

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

            let slot = self.builder.build_slot(tir_payload_ty.clone(), Some(&binding))?;
            self.builder.build_store(slot, payload)?;
            self.locals.insert(binding, (slot, tir_payload_ty, false));
        }

        for stmt in &i.then_block.statements {
            self.lower_statement(stmt)?;
        }
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
        let f_terminated = self.builder.is_terminated();

        if t_terminated && f_terminated {
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
            self.builder.build_const(TirConst::Unit).map_err(|e| e.into())
        }
    }

    fn extract_simple_pattern(&self, pattern: &HirPattern) -> Result<(String, Option<String>), TirLowerError> {
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

    pub fn lower_expression_base(&mut self, expr: &HirExpression) -> Result<TirValueId, TirLowerError> {
        // This is just a marker for where the original code ended
        self.lower_expression(expr)
    }

    fn lower_match(&mut self, m: &HirMatch) -> Result<TirValueId, TirLowerError> {
        let scrutinee_val = self.lower_expression(&m.value)?;
        let scrutinee_ty = self.infer_expr_type(&m.value)?;

        // 1. Infer and verify result type from arms
        let mut match_ty = Type::Unknown;
        for arm in &m.arms {
            // Temporary bindings for inference
            let old_locals_inf = self.locals.clone();
            self.bind_pattern_for_inference(&arm.pattern, &scrutinee_ty)?;
            
            let arm_body_ty = self.infer_expr_type(&arm.value)?;
            self.locals = old_locals_inf;

            if match_ty == Type::Unknown {
                match_ty = arm_body_ty;
            } else if arm_body_ty != match_ty && arm_body_ty != Type::Never && match_ty != Type::Never {
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
             return Err(TirLowerError::UnsupportedExpr("Match must have at least one arm".into(), m.span));
        }

        let tir_match_ty = map_type(&match_ty)?;
        let res_slot = self.builder.build_slot(tir_match_ty.clone(), Some("__match_result"))?;
        let exit_block = self.builder.new_block();

        // 2. Track exhaustiveness
        let mut seen_variants = std::collections::HashSet::new();
        let mut has_wildcard = false;

        // 3. Sequential checks
        for arm in &m.arms {
            if has_wildcard { break; }

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
                    self.builder.build_condbr(cond, then_block, next_arm_block)?;
                    
                    if let HirExpression::Bool(_, b) = &**lit_expr {
                        seen_variants.insert(b.to_string());
                    }
                }
                HirPattern::EnumConstructor(e) => {
                    let tag = self.builder.build_tag(scrutinee_val)?;
                    let wanted = self.builder.build_const(TirConst::String(e.variant_name.clone()))?;
                    let cond = self.builder.build_eq(tag, wanted)?;
                    self.builder.build_condbr(cond, then_block, next_arm_block)?;
                    seen_variants.insert(e.variant_name.clone());
                }
                HirPattern::EnumCase(_, name, _) => {
                    let tag = self.builder.build_tag(scrutinee_val)?;
                    let wanted = self.builder.build_const(TirConst::String(name.clone()))?;
                    let cond = self.builder.build_eq(tag, wanted)?;
                    self.builder.build_condbr(cond, then_block, next_arm_block)?;
                    seen_variants.insert(name.clone());
                }
                _ => return Err(TirLowerError::UnsupportedPattern(format!("{:?}", arm.pattern), arm.pattern.span())),
            }

            // Body
            self.builder.set_current_block(then_block);
            let old_locals = self.locals.clone();

            // Bindings
            match &arm.pattern {
                HirPattern::Identifier(_span, name) => {
                    let tir_scrutinee_ty = map_type(&scrutinee_ty)?;
                    let slot = self.builder.build_slot(tir_scrutinee_ty.clone(), Some(name))?;
                    self.builder.build_store(slot, scrutinee_val)?;
                    self.locals.insert(name.clone(), (slot, tir_scrutinee_ty, false));
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

    fn infer_payload_type(&self, enum_ty: &Type, variant_name: &str, index: usize) -> Result<Type, TirLowerError> {
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
        Err(TirLowerError::UnsupportedType(format!("cannot infer payload type for {}::{} at index {}", enum_ty, variant_name, index)))
    }

    fn check_match_exhaustiveness(&self, ty: &Type, seen: &std::collections::HashSet<String>, has_wildcard: bool, span: tempra_span::Span) -> Result<(), TirLowerError> {
        if has_wildcard { return Ok(()); }

        match ty {
            Type::Bool => {
                if seen.contains("true") && seen.contains("false") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr("Match on Bool must be exhaustive (true AND false, or wildcard)".into(), span))
            }
            Type::Option(_) => {
                if seen.contains("Some") && seen.contains("None") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr("Match on Option must be exhaustive (Some AND None, or wildcard)".into(), span))
            }
            Type::Result(_, _) => {
                if seen.contains("Ok") && seen.contains("Err") {
                    return Ok(());
                }
                Err(TirLowerError::UnsupportedExpr("Match on Result must be exhaustive (Ok AND Err, or wildcard)".into(), span))
            }
            _ => {
                Err(TirLowerError::UnsupportedExpr(format!("Non-exhaustive match for type {}; wildcard '_' is required", ty), span))
            }
        }
    }

    fn bind_pattern_for_inference(&mut self, pattern: &HirPattern, scrutinee_ty: &Type) -> Result<(), TirLowerError> {
        match pattern {
            HirPattern::Identifier(_, name) => {
                let tir_ty = map_type(scrutinee_ty)?;
                self.locals.insert(name.clone(), (TirSlotId(0), tir_ty, false));
            }
            HirPattern::EnumConstructor(e) => {
                for (i, payload_pat) in e.payloads.iter().enumerate() {
                    match payload_pat {
                        HirPattern::Identifier(_, name) => {
                            let payload_ty = self.infer_payload_type(scrutinee_ty, &e.variant_name, i)?;
                            let tir_payload_ty = map_type(&payload_ty)?;
                            self.locals.insert(name.clone(), (TirSlotId(0), tir_payload_ty, false));
                        }
                        HirPattern::Wildcard(_) => {}
                        _ => return Err(TirLowerError::UnsupportedPattern("Nested patterns not supported in inference".into(), payload_pat.span())),
                    }
                }
            }
            HirPattern::EnumCase(_, variant_name, payloads) => {
                for (i, payload_pat) in payloads.iter().enumerate() {
                    match payload_pat {
                        HirPattern::Identifier(_, name) => {
                            let payload_ty = self.infer_payload_type(scrutinee_ty, variant_name, i)?;
                            let tir_payload_ty = map_type(&payload_ty)?;
                            self.locals.insert(name.clone(), (TirSlotId(0), tir_payload_ty, false));
                        }
                        HirPattern::Wildcard(_) => {}
                        _ => return Err(TirLowerError::UnsupportedPattern("Nested patterns not supported in inference".into(), payload_pat.span())),
                    }
                }
            }
            _ => return Ok(()),
        }
        Ok(())
    }
}

pub fn lower_function(func: &HirFunction, type_ctx: &TypeContext, module_builder: &mut ModuleBuilder, self_ty: Option<Type>) -> Result<TirFunction, TirLowerError> {
    let ret_ty = if let Some(rt) = &func.return_type {
        match rt {
            HirType::Named(_, n) => {
                let ty = tempra_typeck::types::resolve_named_type(n);
                map_type(&ty)?
            }
            _ => {
                let ty = resolve_hir_type(rt, type_ctx, self_ty.as_ref())?;
                map_type(&ty)?
            }
        }
    } else {
        TirType::Unit
    };

    let builder = module_builder.new_function(&func.name, ret_ty);
    let mut ctx = LowerContext::new(builder, type_ctx, module_builder, self_ty.clone());
    
    // Create entry block
    let entry_block = ctx.builder.new_block();
    ctx.builder.set_current_block(entry_block);
    
    for p in &func.parameters {
        let ty = match &p.ty {
            HirType::Named(_, n) => tempra_typeck::types::resolve_named_type(n),
            _ => resolve_hir_type(&p.ty, type_ctx, self_ty.as_ref())?,
        };
        let tir_ty = map_type(&ty)?;
        let val_id = ctx.builder.add_param(tir_ty.clone());
        
        let slot = ctx.builder.build_slot(tir_ty.clone(), Some(&p.name))?;
        ctx.builder.build_store(slot, val_id)?;
        // Parameters are immutable by default in Phase 12.2 lowering
        ctx.locals.insert(p.name.clone(), (slot, tir_ty, false));
    }
    
    for stmt in &func.body.statements {
        ctx.lower_statement(stmt)?;
    }
    
    if !ctx.builder.is_terminated() {
        if ctx.builder.func.return_type == TirType::Unit {
            ctx.builder.build_ret(None)?;
        } else {
            return Err(TirLowerError::MissingReturn(func.body.span));
        }
    }
    
    Ok(ctx.builder.build())
}
