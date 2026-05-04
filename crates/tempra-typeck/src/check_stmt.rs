use crate::check_expr::check_expression;
use crate::context::TypeContext;
use crate::types::{types_compatible, Type};
use tempra_hir::*;

/// Check a block and return the type of its last expression (or Unit).
pub fn check_block(ctx: &mut TypeContext, block: &mut HirBlock, expected: Option<&Type>) -> Type {
    ctx.scopes.push_scope();
    let mut last_ty = Type::Unit;
    let len = block.statements.len();
    for (i, stmt) in block.statements.iter_mut().enumerate() {
        let is_last = i == len - 1;
        last_ty = check_statement(ctx, stmt, if is_last { expected } else { None });
    }
    ctx.scopes.pop_scope();
    last_ty
}

/// Check a single statement and return the type it produces.
pub fn check_statement(
    ctx: &mut TypeContext,
    stmt: &mut HirStatement,
    expected: Option<&Type>,
) -> Type {
    match stmt {
        HirStatement::Let(l) => {
            let expected_ty = l.ty.as_ref().map(|t| ctx.resolve_hir_type(t));
            let value_ty = check_expression(ctx, &mut l.value, expected_ty.as_ref());

            if let Some(expected) = &expected_ty {
                if value_ty != Type::Error && !types_compatible(expected, &value_ty) {
                    ctx.error(
                        format!(
                            "Type mismatch in let: expected {}, found {}",
                            expected, value_ty
                        ),
                        l.value.span(),
                    );
                }
            }
            let ty = expected_ty.unwrap_or(value_ty.clone());
            if let Some(info) = ctx.scopes.lookup_anywhere(&l.name) {
                if info.is_capture {
                    ctx.error(format!("cannot shadow captured value '{}'", l.name), l.span);
                }
            }
            ctx.scopes
                .insert_info(l.name.clone(), ty, false, false, false);

            // Leakage check
            for param_id in value_ty.get_generic_params() {
                if !ctx.current_generic_params.contains(&param_id) {
                    ctx.error(
                        format!(
                            "cannot infer generic type for '{}'; add a type annotation",
                            l.name
                        ),
                        l.value.span(),
                    );
                    break;
                }
            }
            Type::Unit
        }
        HirStatement::Var(v) => {
            let expected_ty = v.ty.as_ref().map(|t| ctx.resolve_hir_type(t));
            let value_ty = check_expression(ctx, &mut v.value, expected_ty.as_ref());

            if let Some(expected) = &expected_ty {
                if value_ty != Type::Error && !types_compatible(expected, &value_ty) {
                    ctx.error(
                        format!(
                            "Type mismatch in var: expected {}, found {}",
                            expected, value_ty
                        ),
                        v.value.span(),
                    );
                }
            }
            let ty = expected_ty.unwrap_or(value_ty.clone());
            if let Some(info) = ctx.scopes.lookup_anywhere(&v.name) {
                if info.is_capture {
                    ctx.error(format!("cannot shadow captured value '{}'", v.name), v.span);
                }
            }
            ctx.scopes
                .insert_info(v.name.clone(), ty, true, false, false);

            // Leakage check
            for param_id in value_ty.get_generic_params() {
                if !ctx.current_generic_params.contains(&param_id) {
                    ctx.error(
                        format!(
                            "cannot infer generic type for '{}'; add a type annotation",
                            v.name
                        ),
                        v.value.span(),
                    );
                    break;
                }
            }
            Type::Unit
        }
        HirStatement::Return(value) => {
            let expected_ret = ctx.current_return_type.clone();
            let actual = match value {
                Some(expr) => check_expression(ctx, expr, expected_ret.as_ref()),
                None => Type::Unit,
            };
            if let Some(expected) = &ctx.current_return_type {
                if actual != Type::Error && !types_compatible(expected, &actual) {
                    let span = match value {
                        Some(expr) => expr.span(),
                        None => tempra_span::Span::new(0, 0), // Use meaningful span if possible
                    };
                    ctx.error(
                        format!(
                            "return type mismatch: expected {}, found {}",
                            expected, actual
                        ),
                        span,
                    );
                }
            } else {
                let span = match value {
                    Some(expr) => expr.span(),
                    None => tempra_span::Span::new(0, 0),
                };
                ctx.error("return statement outside of function", span);
            }
            Type::Never
        }
        HirStatement::Break(_) | HirStatement::Continue(_) => Type::Never,
        HirStatement::For(f) => {
            let iter_ty = check_expression(ctx, &mut f.iterable, None);
            let elem_ty = match &iter_ty {
                Type::Array(inner) => *inner.clone(),
                Type::Slice(inner) => *inner.clone(),
                Type::String => Type::String,
                _ => {
                    if iter_ty != Type::Error {
                        ctx.error(
                            format!(
                                "for iterable must be an array, slice, or string, found {}",
                                iter_ty
                            ),
                            f.span,
                        );
                    }
                    Type::Error
                }
            };

            ctx.scopes.push_scope();
            ctx.scopes.insert(f.binding.clone(), elem_ty);

            if let Some(ref mut filter) = f.filter {
                let filter_ty = check_expression(ctx, filter, Some(&Type::Bool));
                if filter_ty != Type::Bool && filter_ty != Type::Error {
                    ctx.error(
                        format!("for filter must be Bool, found {}", filter_ty),
                        f.span,
                    );
                }
            }

            check_block(ctx, &mut f.body, None);
            ctx.scopes.pop_scope();
            Type::Unit
        }
        HirStatement::Expr(expr) => check_expression(ctx, expr, expected),
    }
}
