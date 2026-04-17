use crate::check_expr::check_expression;
use crate::context::TypeContext;
use crate::types::{types_compatible, Type};
use fero_hir::*;

/// Check a block and return the type of its last expression (or Unit).
pub fn check_block(ctx: &mut TypeContext, block: &HirBlock) -> Type {
    ctx.scopes.push_scope();
    let mut last_ty = Type::Unit;
    for stmt in &block.statements {
        last_ty = check_statement(ctx, stmt);
    }
    ctx.scopes.pop_scope();
    last_ty
}

/// Check a single statement and return the type it produces.
pub fn check_statement(ctx: &mut TypeContext, stmt: &HirStatement) -> Type {
    match stmt {
        HirStatement::Let(l) => {
            let value_ty = check_expression(ctx, &l.value);
            if matches!(&l.value, HirExpression::Name(_, n) if n == "None") {
                ctx.error(
                    "Ambiguous 'None' without Option context; use it where an Option[T] is expected"
                        .to_string(),
                    l.value.span(),
                );
            }
            ctx.scopes.insert(l.name.clone(), value_ty);
            Type::Unit
        }
        HirStatement::Var(v) => {
            let value_ty = check_expression(ctx, &v.value);
            if matches!(&v.value, HirExpression::Name(_, n) if n == "None") {
                ctx.error(
                    "Ambiguous 'None' without Option context; use it where an Option[T] is expected"
                        .to_string(),
                    v.value.span(),
                );
            }
            ctx.scopes.insert(v.name.clone(), value_ty);
            Type::Unit
        }
        HirStatement::Return(value) => {
            let actual = match value {
                Some(expr) => check_expression(ctx, expr),
                None => Type::Unit,
            };
            if let Some(expected) = &ctx.current_return_type {
                if actual != Type::Error && !types_compatible(expected, &actual) {
                    let span = match value {
                        Some(expr) => expr.span(),
                        None => fero_span::Span::new(0, 0), // Use meaningful span if possible
                    };
                    ctx.error(
                        format!(
                            "Return type mismatch: expected {}, found {}",
                            expected, actual
                        ),
                        span,
                    );
                }
            } else {
                let span = match value {
                    Some(expr) => expr.span(),
                    None => fero_span::Span::new(0, 0),
                };
                ctx.error("Return statement outside of function", span);
            }
            Type::Unit
        }
        HirStatement::Break(_) | HirStatement::Continue(_) => Type::Unit,
        HirStatement::For(f) => {
            let iter_ty = check_expression(ctx, &f.iterable);
            let elem_ty = match &iter_ty {
                Type::Array(inner) => *inner.clone(),
                Type::Slice(inner) => *inner.clone(),
                Type::String => Type::String,
                _ => {
                    if iter_ty != Type::Error {
                        ctx.error(
                            format!(
                                "For iterable must be an array, slice, or string, found {}",
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

            if let Some(filter) = &f.filter {
                let filter_ty = check_expression(ctx, filter);
                if filter_ty != Type::Bool && filter_ty != Type::Error {
                    ctx.error(
                        format!("For filter must be Bool, found {}", filter_ty),
                        f.span,
                    );
                }
            }

            check_block(ctx, &f.body);
            ctx.scopes.pop_scope();
            Type::Unit
        }
        HirStatement::Expr(expr) => check_expression(ctx, expr),
    }
}
