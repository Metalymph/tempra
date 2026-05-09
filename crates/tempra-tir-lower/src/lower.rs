use crate::error::TirLowerError;
use std::collections::HashMap;
use tempra_hir::*;
use tempra_tir::builder::{FunctionBuilder, ModuleBuilder};
use tempra_tir::ir::*;
use tempra_typeck::context::TypeContext;
use tempra_typeck::Type;

pub fn map_type(ty: &Type) -> Result<TirType, TirLowerError> {
    match ty {
        Type::Unit => Ok(TirType::Unit),
        Type::Bool => Ok(TirType::Bool),
        Type::Int64 | Type::Int => Ok(TirType::Int64),
        Type::UInt64 => Ok(TirType::UInt64),
        Type::Float64 => Ok(TirType::Float64),
        Type::String => Ok(TirType::String),
        Type::Never => Ok(TirType::Never),
        _ => Err(TirLowerError::UnsupportedType(format!("{:?}", ty))),
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
                if let Some(info) = self.type_ctx.scopes.lookup(name) {
                    Ok(info.ty.clone())
                } else if let Some((_, tir_ty, _)) = self.locals.get(name) {
                    match tir_ty {
                        TirType::Int64 => Ok(Type::Int64),
                        TirType::Bool => Ok(Type::Bool),
                        TirType::String => Ok(Type::String),
                        _ => Err(TirLowerError::UnsupportedType(format!("{:?}", tir_ty))),
                    }
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
                let val_ty = self.infer_expr_type(&l.value)?;
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&l.name))?;
                self.locals.insert(l.name.clone(), (slot, tir_ty, false));
                
                let val = self.lower_expression(&l.value)?;
                self.builder.build_store(slot, val)?;
                Ok(())
            }
            HirStatement::Var(v) => {
                let val_ty = self.infer_expr_type(&v.value)?;
                let tir_ty = map_type(&val_ty)?;
                let slot = self.builder.build_slot(tir_ty.clone(), Some(&v.name))?;
                self.locals.insert(v.name.clone(), (slot, tir_ty, true));
                
                let val = self.lower_expression(&v.value)?;
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
                if let HirExpression::Name(span, name) = &*c.callee {
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
            _ => Err(TirLowerError::UnsupportedExpr("Unsupported expression".into(), expr.span())),
        }
    }
}

pub fn lower_function(func: &HirFunction, type_ctx: &TypeContext, module_builder: &mut ModuleBuilder) -> Result<TirFunction, TirLowerError> {
    let ret_ty = if let Some(rt) = &func.return_type {
        let ty = tempra_typeck::types::resolve_named_type(match rt {
            HirType::Named(_, n) => n,
            _ => return Err(TirLowerError::UnsupportedType("Complex return type".into())),
        });
        map_type(&ty)?
    } else {
        TirType::Unit
    };

    let builder = module_builder.new_function(&func.name, ret_ty);
    let mut ctx = LowerContext::new(builder, type_ctx, module_builder);
    
    // Create entry block
    let entry_block = ctx.builder.new_block();
    ctx.builder.set_current_block(entry_block);
    
    for p in &func.parameters {
        let ty = tempra_typeck::types::resolve_named_type(match &p.ty {
            HirType::Named(_, n) => n,
            _ => return Err(TirLowerError::UnsupportedType("Complex param type".into())),
        });
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
