//! Expression-level type checking logic.

use fero_span::Span;
use fero_hir::*;
use crate::types::Type;
use crate::context::TypeContext;

/// Check an expression and return its inferred type.
pub fn check_expression(ctx: &mut TypeContext, expr: &HirExpression) -> Type {
    match expr {
        HirExpression::Name(span, name) => check_name(ctx, name, *span),
        HirExpression::Integer(_, _) => Type::Int64,
        HirExpression::Float(_, _) => Type::Float64,
        HirExpression::String(_, _) => Type::String,
        HirExpression::Bool(_, _) => Type::Bool,
        HirExpression::Unary(u) => check_unary(ctx, u),
        HirExpression::Binary(b) => check_binary(ctx, b),
        HirExpression::Call(c) => check_call(ctx, c),
        HirExpression::Member(m) => check_member(ctx, m),
        HirExpression::Index(i) => check_index(ctx, i),
        HirExpression::Block(b) => check_block_expr(ctx, b),
        HirExpression::If(i) => check_if(ctx, i),
        HirExpression::Match(m) => check_match(ctx, m),
        HirExpression::For(f) => check_for_expr(ctx, f),
        HirExpression::Tuple(span, elems) => check_tuple(ctx, elems, *span),
        HirExpression::Array(span, elems) => check_array(ctx, elems, *span),
        HirExpression::StructLiteral(s) => check_struct_literal(ctx, s),
    }
}

fn check_name(ctx: &mut TypeContext, name: &str, span: Span) -> Type {
    if let Some(ty) = ctx.scopes.lookup(name) {
        return ty.clone();
    }
    if ctx.functions.contains_key(name) {
        // Function used as a value — for now return Unknown (first-class fns later)
        return Type::Unknown;
    }
    ctx.error(format!("Undefined variable '{}'", name), span);
    Type::Error
}

fn check_unary(ctx: &mut TypeContext, u: &HirUnary) -> Type {
    let operand_ty = check_expression(ctx, &u.expr);
    match u.op {
        HirUnaryOp::Not => {
            if operand_ty != Type::Bool && operand_ty != Type::Error {
                ctx.error(format!("'!' expects Bool, found {}", operand_ty), u.span);
            }
            Type::Bool
        }
        HirUnaryOp::Negate => {
            if !operand_ty.is_numeric() && operand_ty != Type::Error {
                ctx.error(format!("Unary '-' expects numeric type, found {}", operand_ty), u.span);
                return Type::Error;
            }
            operand_ty
        }
    }
}

fn check_binary(ctx: &mut TypeContext, b: &HirBinary) -> Type {
    let left = check_expression(ctx, &b.left);
    let right = check_expression(ctx, &b.right);

    // Propagate errors without cascading
    if left == Type::Error || right == Type::Error {
        return Type::Error;
    }

    match b.op {
        HirBinaryOp::Add | HirBinaryOp::Sub | HirBinaryOp::Mul | HirBinaryOp::Div => {
            if !left.is_numeric() {
                ctx.error(format!("Arithmetic operator expects numeric type, found {}", left), b.span);
                return Type::Error;
            }
            if left != right {
                ctx.error(
                    format!("Arithmetic operands must have the same type: {} vs {}", left, right),
                    b.span,
                );
                return Type::Error;
            }
            left
        }
        HirBinaryOp::Eq | HirBinaryOp::NotEq => {
            if left != right {
                ctx.error(format!("Equality operands must have the same type: {} vs {}", left, right), b.span);
            }
            Type::Bool
        }
        HirBinaryOp::Less | HirBinaryOp::Greater => {
            if !left.is_numeric() {
                ctx.error(format!("Comparison operator expects numeric type, found {}", left), b.span);
            }
            if left != right {
                ctx.error(format!("Comparison operands must have the same type: {} vs {}", left, right), b.span);
            }
            Type::Bool
        }
        HirBinaryOp::And | HirBinaryOp::Or => {
            if left == Type::Command && right == Type::Command {
                return Type::CommandResult;
            }
            if left != Type::Bool && left != Type::Error {
                ctx.error(format!("Logical operator expects Bool, found {}", left), b.span);
            }
            if right != Type::Bool && right != Type::Error {
                ctx.error(format!("Logical operator expects Bool, found {}", right), b.span);
            }
            Type::Bool
        }
        HirBinaryOp::Assign => {
            if left != right {
                ctx.error(format!("Cannot assign {} to {}", right, left), b.span);
            }
            Type::Unit
        }
        HirBinaryOp::Pipe => {
            if left == Type::Command && right == Type::Command {
                return Type::CommandResult;
            }
            ctx.error("Pipe operator expects Command types".to_string(), b.span);
            Type::Error
        }
        HirBinaryOp::ShiftRight => {
            ctx.error("ShiftRight operator is not yet supported in type checking".to_string(), b.span);
            Type::Error
        }
    }
}

fn check_builtin(ctx: &mut TypeContext, name: &str, args: &[HirExpression], span: Span) -> Option<Type> {
    match name {
        "print" => {
            if args.len() != 1 {
                ctx.error(format!("Function 'print' expects 1 argument(s), found {}", args.len()), span);
            } else {
                let _ = check_expression(ctx, &args[0]);
            }
            Some(Type::Unit)
        }
        "read_line" => {
            if args.len() != 0 {
                ctx.error(format!("Function 'read_line' expects 0 argument(s), found {}", args.len()), span);
            }
            Some(Type::String)
        }
        "cmd" => {
            if args.len() != 2 {
                ctx.error(format!("Function 'cmd' expects 2 argument(s), found {}", args.len()), span);
            } else {
                let p = check_expression(ctx, &args[0]);
                if p != Type::String && p != Type::Error {
                    ctx.error(format!("Argument 1 of 'cmd': expected String, found {}", p), args[0].span());
                }
                let a = check_expression(ctx, &args[1]);
                if a != Type::Array(Box::new(Type::String)) && a != Type::Error { // For an empty array it might be Type::Array(Unknown), we should be careful.
                    // Empty array literal gives Array(Unknown)
                    if a != Type::Array(Box::new(Type::Unknown)) {
                        ctx.error(format!("Argument 2 of 'cmd': expected [String], found {}", a), args[1].span());
                    }
                }
            }
            Some(Type::Command)
        }
        _ => None,
    }
}

fn check_call(ctx: &mut TypeContext, c: &HirCall) -> Type {
    // For now we only support direct name calls
    let func_name = match &*c.callee {
        HirExpression::Name(_, name) => name.clone(),
        HirExpression::Member(m) => {
            let object_ty = check_expression(ctx, &m.object);
            if object_ty == Type::Command && m.field == "run" {
                if !c.args.is_empty() {
                    ctx.error(format!("Method 'run' on Command expects 0 argument(s), found {}", c.args.len()), c.span);
                }
                return Type::CommandResult;
            }
            ctx.error("Method calls only supported for Command.run()".to_string(), c.span);
            return Type::Error;
        }
        _ => {
            // Check the callee expression anyway
            let _ = check_expression(ctx, &c.callee);
            return Type::Unknown;
        }
    };

    if let Some(builtin_ty) = check_builtin(ctx, &func_name, &c.args, c.span) {
        return builtin_ty;
    }

    let sig = match ctx.functions.get(&func_name) {
        Some(sig) => sig.clone(),
        None => {
            ctx.error(format!("Undefined function '{}'", func_name), c.span);
            return Type::Error;
        }
    };

    if c.args.len() != sig.params.len() {
        ctx.error(
            format!(
                "Function '{}' expects {} argument(s), found {}",
                func_name,
                sig.params.len(),
                c.args.len()
            ),
            c.span,
        );
        return Type::Error;
    }

    for (i, arg) in c.args.iter().enumerate() {
        let arg_ty = check_expression(ctx, arg);
        let param_ty = &sig.params[i].1;
        if arg_ty != Type::Error && arg_ty != *param_ty {
            ctx.error(
                format!(
                    "Argument {} of '{}': expected {}, found {}",
                    i + 1,
                    func_name,
                    param_ty,
                    arg_ty
                ),
                arg.span(),
            );
        }
    }

    sig.return_type
}

fn check_member(ctx: &mut TypeContext, m: &HirMember) -> Type {
    let object_ty = check_expression(ctx, &m.object);
    if let Type::Struct(name) = &object_ty {
        if let Some(info) = ctx.structs.get(name.as_str()) {
            if let Some(field_ty) = info.fields.get(&m.field) {
                return field_ty.clone();
            }
            ctx.error(format!("Struct '{}' has no field '{}'", name, m.field), m.span);
            return Type::Error;
        }
    } else if object_ty == Type::CommandResult {
        match m.field.as_str() {
            "exit_code" => return Type::Int64,
            "stdout" | "stderr" => return Type::String,
            "success" => return Type::Bool,
            _ => {
                ctx.error(format!("CommandResult has no field '{}'", m.field), m.span);
                return Type::Error;
            }
        }
    }
    if object_ty != Type::Error {
        ctx.error(format!("Member access on non-struct type {}", object_ty), m.span);
    }
    Type::Error
}

fn check_index(ctx: &mut TypeContext, i: &HirIndex) -> Type {
    let object_ty = check_expression(ctx, &i.object);
    let index_ty = check_expression(ctx, &i.index);

    if let Type::Array(elem) = &object_ty {
        if !index_ty.is_integer() && index_ty != Type::Error {
            ctx.error(format!("Array index must be integer, found {}", index_ty), i.span);
        }
        return *elem.clone();
    }
    if object_ty != Type::Error {
        ctx.error(format!("Cannot index type {}", object_ty), i.span);
    }
    Type::Error
}

fn check_block_expr(ctx: &mut TypeContext, block: &HirBlock) -> Type {
    crate::check_stmt::check_block(ctx, block)
}

fn check_if(ctx: &mut TypeContext, i: &HirIf) -> Type {
    let cond = check_expression(ctx, &i.condition);
    if cond != Type::Bool && cond != Type::Error {
        ctx.error(format!("If condition must be Bool, found {}", cond), i.span);
    }

    let then_ty = crate::check_stmt::check_block(ctx, &i.then_block);

    if let Some(else_branch) = &i.else_branch {
        let else_ty = check_expression(ctx, else_branch);
        if then_ty != Type::Error && else_ty != Type::Error && then_ty != else_ty {
            ctx.error(
                format!("If branches have incompatible types: {} vs {}", then_ty, else_ty),
                i.span,
            );
        }
        then_ty
    } else {
        Type::Unit
    }
}

fn check_match(ctx: &mut TypeContext, m: &HirMatch) -> Type {
    let _value_ty = check_expression(ctx, &m.value);

    let mut result_ty: Option<Type> = None;
    for arm in &m.arms {
        let arm_ty = check_expression(ctx, &arm.value);
        if let Some(ref expected) = result_ty {
            if arm_ty != Type::Error && *expected != Type::Error && arm_ty != *expected {
                ctx.error(
                    format!("Match arms have incompatible types: {} vs {}", expected, arm_ty),
                    arm.span,
                );
            }
        } else {
            result_ty = Some(arm_ty);
        }
    }
    result_ty.unwrap_or(Type::Unit)
}

fn check_for_expr(ctx: &mut TypeContext, f: &HirForExpr) -> Type {
    let iter_ty = check_expression(ctx, &f.iterable);
    let elem_ty = match &iter_ty {
        Type::Array(inner) => *inner.clone(),
        _ => {
            if iter_ty != Type::Error {
                ctx.error(format!("For iterable must be an array, found {}", iter_ty), f.span);
            }
            Type::Error
        }
    };

    ctx.scopes.push_scope();
    ctx.scopes.insert(f.binding.clone(), elem_ty);

    if let Some(filter) = &f.filter {
        let filter_ty = check_expression(ctx, filter);
        if filter_ty != Type::Bool && filter_ty != Type::Error {
            ctx.error(format!("For filter must be Bool, found {}", filter_ty), f.span);
        }
    }

    let body_ty = crate::check_stmt::check_block(ctx, &f.body);
    ctx.scopes.pop_scope();

    Type::Array(Box::new(body_ty))
}

fn check_tuple(ctx: &mut TypeContext, elems: &[HirExpression], _span: Span) -> Type {
    let types: Vec<Type> = elems.iter().map(|e| check_expression(ctx, e)).collect();
    Type::Tuple(types)
}

fn check_array(ctx: &mut TypeContext, elems: &[HirExpression], span: Span) -> Type {
    if elems.is_empty() {
        return Type::Array(Box::new(Type::Unknown));
    }

    let first = check_expression(ctx, &elems[0]);
    for elem in &elems[1..] {
        let ty = check_expression(ctx, elem);
        if ty != Type::Error && first != Type::Error && ty != first {
            ctx.error(
                format!("Array elements must have the same type: expected {}, found {}", first, ty),
                span,
            );
        }
    }
    Type::Array(Box::new(first))
}

fn check_struct_literal(ctx: &mut TypeContext, s: &HirStructLiteral) -> Type {
    let info = match ctx.structs.get(&s.name) {
        Some(info) => info.clone(),
        None => {
            ctx.error(format!("Unknown struct '{}'", s.name), s.span);
            return Type::Error;
        }
    };

    // Check each provided field
    for (field_name, field_expr) in &s.fields {
        let actual_ty = check_expression(ctx, field_expr);
        match info.fields.get(field_name.as_str()) {
            Some(expected_ty) => {
                if actual_ty != Type::Error && actual_ty != *expected_ty {
                    ctx.error(
                        format!(
                            "Field '{}' of '{}': expected {}, found {}",
                            field_name, s.name, expected_ty, actual_ty
                        ),
                        field_expr.span(),
                    );
                }
            }
            None => {
                ctx.error(
                    format!("Struct '{}' has no field '{}'", s.name, field_name),
                    field_expr.span(),
                );
            }
        }
    }

    // Check that all required fields are present
    let provided: std::collections::HashSet<&str> =
        s.fields.iter().map(|(n, _)| n.as_str()).collect();
    for field_name in info.fields.keys() {
        if !provided.contains(field_name.as_str()) {
            ctx.error(
                format!("Missing field '{}' in struct literal '{}'", field_name, s.name),
                s.span,
            );
        }
    }

    Type::Struct(s.name.clone())
}
