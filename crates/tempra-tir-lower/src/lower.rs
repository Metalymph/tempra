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
        Type::UInt8 | Type::UInt16 | Type::UInt32 | Type::UInt64 => Ok(TirType::UInt64),
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
        Type::Struct(name, _) => Err(TirLowerError::UnsupportedType(format!("Struct({})", name))),
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
        _ => Err(TirLowerError::UnsupportedType(format!("{:?}", tir_ty))),
    }
}

/// Resolve an HirType to a semantic Type. Minimal — only handles what Phase 14.4 needs.
fn resolve_hir_type(hir_ty: &HirType, _type_ctx: &TypeContext) -> Result<Type, TirLowerError> {
    match hir_ty {
        HirType::Named(_, name) => {
            let ty = tempra_typeck::types::resolve_named_type(name);
            if ty == Type::Unknown {
                Err(TirLowerError::UnsupportedType(format!("Unknown type name: {}", name)))
            } else {
                Ok(ty)
            }
        }
        HirType::Result(_, ok_hir, err_hir) => {
            let ok_ty = resolve_hir_type(ok_hir, _type_ctx)?;
            let err_ty = resolve_hir_type(err_hir, _type_ctx)?;
            Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
        }
        HirType::Optional(_, inner_hir) => {
            let inner_ty = resolve_hir_type(inner_hir, _type_ctx)?;
            Ok(Type::Option(Box::new(inner_ty)))
        }
        _ => Err(TirLowerError::UnsupportedType(format!("Cannot resolve HirType: {:?}", hir_ty))),
    }
}

pub struct LowerContext<'a> {
    pub builder: FunctionBuilder,
    pub type_ctx: &'a TypeContext,
    pub module_builder: &'a mut ModuleBuilder,
    /// (SlotId, Type, is_mutable)
    pub locals: HashMap<String, (TirSlotId, TirType, bool)>,
}

impl<'a> LowerContext<'a> {
    pub fn new(builder: FunctionBuilder, type_ctx: &'a TypeContext, module_builder: &'a mut ModuleBuilder) -> Self {
        Self {
            builder,
            type_ctx,
            module_builder,
            locals: HashMap::new(),
        }
    }

    fn infer_expr_type(&self, expr: &HirExpression) -> Result<Type, TirLowerError> {
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
                if m.field == "len" {
                    let obj_ty = self.infer_expr_type(&m.object)?;
                    if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                        return Ok(Type::Int64);
                    }
                }
                Err(TirLowerError::UnsupportedExpr(format!("Member access: {}", m.field), m.span))
            }
            HirExpression::Call(c) => {
                if let HirExpression::Member(m) = &*c.callee {
                    if m.field == "len" {
                        return Ok(Type::Int64);
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
                    // Manual resolution for simple types since we only have &TypeContext
                    match hir_ty {
                        HirType::Named(_, name) if name == "Int64" => Type::Int64,
                        HirType::Named(_, name) if name == "Bool" => Type::Bool,
                        HirType::Named(_, name) if name == "String" => Type::String,
                        HirType::Optional(_, inner) => {
                            if let HirType::Named(_, name) = &**inner {
                                if name == "Int64" {
                                    Type::Option(Box::new(Type::Int64))
                                } else if name == "Bool" {
                                    Type::Option(Box::new(Type::Bool))
                                } else if name == "String" {
                                    Type::Option(Box::new(Type::String))
                                } else {
                                    self.infer_expr_type(&l.value)?
                                }
                            } else {
                                self.infer_expr_type(&l.value)?
                            }
                        }
                        HirType::Result(_, ok_hir, err_hir) => {
                            let ok_ty = if let HirType::Named(_, name) = &**ok_hir {
                                if name == "Int64" {
                                    Type::Int64
                                } else if name == "Bool" {
                                    Type::Bool
                                } else if name == "String" {
                                    Type::String
                                } else {
                                    Type::Unknown
                                }
                            } else {
                                Type::Unknown
                            };
                            let err_ty = if let HirType::Named(_, name) = &**err_hir {
                                if name == "Int64" {
                                    Type::Int64
                                } else if name == "Bool" {
                                    Type::Bool
                                } else if name == "String" {
                                    Type::String
                                } else {
                                    Type::Unknown
                                }
                            } else {
                                Type::Unknown
                            };

                            if ok_ty != Type::Unknown && err_ty != Type::Unknown {
                                Type::Result(Box::new(ok_ty), Box::new(err_ty))
                            } else {
                                self.infer_expr_type(&l.value)?
                            }
                        }
                        _ => self.infer_expr_type(&l.value)?,
                    }
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
                    _ => self.lower_expression(&l.value)?,
                };
                self.builder.build_store(slot, val)?;
                Ok(())
            }
            HirStatement::Var(v) => {
                let val_ty = if let Some(hir_ty) = &v.ty {
                    match hir_ty {
                        HirType::Named(_, name) if name == "Int64" => Type::Int64,
                        HirType::Named(_, name) if name == "Bool" => Type::Bool,
                        HirType::Named(_, name) if name == "String" => Type::String,
                        HirType::Optional(_, inner) => {
                            if let HirType::Named(_, name) = &**inner {
                                if name == "Int64" {
                                    Type::Option(Box::new(Type::Int64))
                                } else if name == "Bool" {
                                    Type::Option(Box::new(Type::Bool))
                                } else if name == "String" {
                                    Type::Option(Box::new(Type::String))
                                } else {
                                    self.infer_expr_type(&v.value)?
                                }
                            } else {
                                self.infer_expr_type(&v.value)?
                            }
                        }
                        HirType::Result(_, ok_hir, err_hir) => {
                            let ok_ty = if let HirType::Named(_, name) = &**ok_hir {
                                if name == "Int64" {
                                    Type::Int64
                                } else if name == "Bool" {
                                    Type::Bool
                                } else if name == "String" {
                                    Type::String
                                } else {
                                    Type::Unknown
                                }
                            } else {
                                Type::Unknown
                            };
                            let err_ty = if let HirType::Named(_, name) = &**err_hir {
                                if name == "Int64" {
                                    Type::Int64
                                } else if name == "Bool" {
                                    Type::Bool
                                } else if name == "String" {
                                    Type::String
                                } else {
                                    Type::Unknown
                                }
                            } else {
                                Type::Unknown
                            };

                            if ok_ty != Type::Unknown && err_ty != Type::Unknown {
                                Type::Result(Box::new(ok_ty), Box::new(err_ty))
                            } else {
                                self.infer_expr_type(&v.value)?
                            }
                        }
                        _ => self.infer_expr_type(&v.value)?,
                    }
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
            _ => Err(TirLowerError::UnsupportedStmt(format!("Stmt"), tempra_span::Span::new(0, 0))),
        }
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
                self.builder.build_const(TirConst::String(s.clone())).map_err(|e| e.into())
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
                    Err(TirLowerError::UnknownLocal(name.clone(), *span))
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
                    } else {
                        return Err(TirLowerError::UnsupportedStmt("non-local assignment not supported in Phase 12.2".into(), b.span));
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
                    if m.field == "len" && c.args.is_empty() {
                        let obj_ty = self.infer_expr_type(&m.object)?;
                        if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                            let obj = self.lower_expression(&m.object)?;
                            return self.builder.build_len(obj).map_err(|e| e.into());
                        }
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
                let obj = self.lower_expression(&i.object)?;
                let idx = self.lower_expression(&i.index)?;
                self.builder.build_get_index(obj, idx).map_err(|e| e.into())
            }
            HirExpression::Slice(s) => {
                let obj = self.lower_expression(&s.object)?;
                let start = self.lower_expression(&s.start)?;
                let end = self.lower_expression(&s.end)?;
                self.builder.build_slice(obj, start, end).map_err(|e| e.into())
            }
            HirExpression::Member(m) => {
                if m.field == "len" {
                    let obj_ty = self.infer_expr_type(&m.object)?;
                    if matches!(obj_ty, Type::Array(_) | Type::Slice(_)) {
                        let obj = self.lower_expression(&m.object)?;
                        return self.builder.build_len(obj).map_err(|e| e.into());
                    }
                }
                Err(TirLowerError::UnsupportedExpr(format!("Member access: {}", m.field), m.span))
            }
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
}

pub fn lower_function(func: &HirFunction, type_ctx: &TypeContext, module_builder: &mut ModuleBuilder) -> Result<TirFunction, TirLowerError> {
    let ret_ty = if let Some(rt) = &func.return_type {
        match rt {
            HirType::Named(_, n) => {
                let ty = tempra_typeck::types::resolve_named_type(n);
                map_type(&ty)?
            }
            _ => {
                let ty = resolve_hir_type(rt, type_ctx)?;
                map_type(&ty)?
            }
        }
    } else {
        TirType::Unit
    };

    let builder = module_builder.new_function(&func.name, ret_ty);
    let mut ctx = LowerContext::new(builder, type_ctx, module_builder);
    
    // Create entry block
    let entry_block = ctx.builder.new_block();
    ctx.builder.set_current_block(entry_block);
    
    for p in &func.parameters {
        let ty = match &p.ty {
            HirType::Named(_, n) => tempra_typeck::types::resolve_named_type(n),
            _ => resolve_hir_type(&p.ty, type_ctx)?,
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
