//! Expression-level type checking logic.

use crate::context::TypeContext;
use crate::types::{types_compatible, Type};
use fero_hir::*;
use fero_span::Span;

/// Check an expression and return its inferred type.
pub fn check_expression(ctx: &mut TypeContext, expr: &HirExpression) -> Type {
    match expr {
        HirExpression::Name(span, name) => check_name(ctx, name, *span),
        HirExpression::Integer(_, _) => Type::Int,
        HirExpression::Float(_, _) => Type::Float64,
        HirExpression::String(_, _) => Type::String,
        HirExpression::Bool(_, _) => Type::Bool,
        HirExpression::Unary(u) => check_unary(ctx, u),
        HirExpression::Binary(b) => check_binary(ctx, b),
        HirExpression::Is(i) => check_is(ctx, i),
        HirExpression::IsPattern(i) => check_is_pattern(ctx, i),
        HirExpression::AsOptional(a) => check_as_optional(ctx, a),
        HirExpression::AsForced(a) => check_as_forced(ctx, a),
        HirExpression::Call(c) => check_call(ctx, c),
        HirExpression::Member(m) => check_member(ctx, m),
        HirExpression::Index(i) => check_index(ctx, i),
        HirExpression::Slice(s) => check_slice(ctx, s),
        HirExpression::Block(b) => check_block_expr(ctx, b),
        HirExpression::If(i) => check_if(ctx, i),
        HirExpression::Match(m) => check_match(ctx, m),
        HirExpression::For(f) => check_for_expr(ctx, f),
        HirExpression::Tuple(span, elems) => check_tuple(ctx, elems, *span),
        HirExpression::Array(span, elems) => check_array(ctx, elems, *span),
        HirExpression::StructLiteral(s) => check_struct_literal(ctx, s),
        HirExpression::Try(e) => check_try(ctx, e),
        HirExpression::Unwrap(e) => check_unwrap(ctx, e),
    }
}

fn check_is(ctx: &mut TypeContext, i: &HirIsExpr) -> Type {
    let _ = check_expression(ctx, &i.value);
    
    if let HirType::Named(span, name) = &i.ty {
        if name.starts_with(|c: char| c.is_lowercase()) {
            ctx.error("Naked binding patterns are not allowed after 'is'", *span);
            return Type::Error;
        }
    }
    
    let ty = ctx.resolve_hir_type(&i.ty);
    if ty == Type::Unknown {
        ctx.error("Unknown type in 'is' check".to_string(), i.ty.span());
    }
    Type::Bool
}

fn check_is_pattern(ctx: &mut TypeContext, i: &HirIsPatternExpr) -> Type {
    let val_ty = check_expression(ctx, &i.value);
    if val_ty != Type::Error {
        validate_pattern_type(ctx, &val_ty, &i.pattern);
    }
    Type::Bool
}

fn validate_pattern_type(ctx: &mut TypeContext, val_ty: &Type, pattern: &HirPattern) {
    match pattern {
        HirPattern::Wildcard(_) | HirPattern::Identifier(_, _) => {}
        HirPattern::EnumCase(span, name, args) => {
            match name.as_str() {
                "Some" => {
                    if let Type::Option(inner) = val_ty {
                        if args.len() == 1 {
                            validate_pattern_type(ctx, inner, &args[0]);
                        } else {
                            ctx.error("Some pattern expects 1 argument", *span);
                        }
                    } else {
                        ctx.error(format!("Cannot match {} against Option", val_ty), *span);
                    }
                }
                "None" => {
                    if !matches!(val_ty, Type::Option(_)) {
                        ctx.error(format!("Cannot match {} against Option", val_ty), *span);
                    }
                    if !args.is_empty() {
                        ctx.error("None pattern expects 0 arguments", *span);
                    }
                }
                "Ok" => {
                    if let Type::Result(ok_ty, _) = val_ty {
                        if args.len() == 1 {
                            validate_pattern_type(ctx, ok_ty, &args[0]);
                        } else {
                            ctx.error("Ok pattern expects 1 argument", *span);
                        }
                    } else {
                        ctx.error(format!("Cannot match {} against Result", val_ty), *span);
                    }
                }
                "Err" => {
                    if let Type::Result(_, err_ty) = val_ty {
                        if args.len() == 1 {
                            validate_pattern_type(ctx, err_ty, &args[0]);
                        } else {
                            ctx.error("Err pattern expects 1 argument", *span);
                        }
                    } else {
                        ctx.error(format!("Cannot match {} against Result", val_ty), *span);
                    }
                }
                _ => {
                    ctx.error("Unsupported pattern case (only Some, None, Ok, Err allowed in v1)", *span);
                }
            }
        }
        HirPattern::Literal(span, _) => {
            ctx.error("Literal patterns not supported in v1", *span);
        }
    }
}

fn check_as_optional(ctx: &mut TypeContext, a: &HirAsOptionalExpr) -> Type {
    let _ = check_expression(ctx, &a.expr);
    let ty = ctx.resolve_hir_type(&a.ty);
    if ty == Type::Unknown {
        ctx.error("Unknown type in 'as?' cast".to_string(), a.ty.span());
    }
    Type::Option(Box::new(ty))
}

fn check_as_forced(ctx: &mut TypeContext, a: &HirAsForcedExpr) -> Type {
    let _ = check_expression(ctx, &a.expr);
    let ty = ctx.resolve_hir_type(&a.ty);
    if ty == Type::Unknown {
        ctx.error("Unknown type in 'as!' cast".to_string(), a.ty.span());
    }
    ty
}

fn check_unwrap(ctx: &mut TypeContext, e: &HirUnwrapExpr) -> Type {
    let ty = check_expression(ctx, &e.expr);
    match ty {
        Type::Result(ok, _) => *ok,
        Type::Error => Type::Error,
        _ => {
            ctx.error(
                format!("'!' operator expects a Result type, found {}", ty),
                e.expr.span(),
            );
            Type::Error
        }
    }
}

fn check_try(ctx: &mut TypeContext, e: &HirTryExpr) -> Type {
    let ty = check_expression(ctx, &e.expr);

    // 1. Must be a Result type
    let (ok_ty, _err_ty) = match ty {
        Type::Result(ok, err) => (*ok, *err),
        Type::Error => return Type::Error,
        _ => {
            ctx.error(
                format!("'?' operator expects a Result type, found {}", ty),
                e.span,
            );
            return Type::Error;
        }
    };

    // 2. Surrounding function must return Result
    if !ctx.current_is_fallible {
        ctx.error(
            "'?' operator can only be used in a fallible function".to_string(),
            e.span,
        );
    }

    ok_ty
}

fn check_name(ctx: &mut TypeContext, name: &str, span: Span) -> Type {
    if name == "None" {
        return Type::Option(Box::new(Type::Unknown));
    }
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
                ctx.error(
                    format!("Unary '-' expects numeric type, found {}", operand_ty),
                    u.span,
                );
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
            if b.op == HirBinaryOp::Add && left == Type::String && right == Type::String {
                return Type::String;
            }
            if !left.is_numeric() {
                ctx.error(
                    format!("Arithmetic operator expects numeric type, found {}", left),
                    b.span,
                );
                return Type::Error;
            }
            if left != right {
                // Allow Int and Int64 to be mixed
                let both_int = (left == Type::Int || left == Type::Int64)
                    && (right == Type::Int || right == Type::Int64);
                if !both_int {
                    ctx.error(
                        format!(
                            "Arithmetic operands must have the same type: {} vs {}",
                            left, right
                        ),
                        b.span,
                    );
                    return Type::Error;
                }
            }
            left
        }
        HirBinaryOp::Eq | HirBinaryOp::NotEq => {
            if left != right {
                let both_int = (left == Type::Int || left == Type::Int64)
                    && (right == Type::Int || right == Type::Int64);
                if !both_int {
                    ctx.error(
                        format!(
                            "Equality operands must have the same type: {} vs {}",
                            left, right
                        ),
                        b.span,
                    );
                }
            }
            Type::Bool
        }
        HirBinaryOp::Less | HirBinaryOp::Greater => {
            if !left.is_numeric() {
                ctx.error(
                    format!("Comparison operator expects numeric type, found {}", left),
                    b.span,
                );
            }
            if left != right {
                let both_int = (left == Type::Int || left == Type::Int64)
                    && (right == Type::Int || right == Type::Int64);
                if !both_int {
                    ctx.error(
                        format!(
                            "Comparison operands must have the same type: {} vs {}",
                            left, right
                        ),
                        b.span,
                    );
                }
            }
            Type::Bool
        }
        HirBinaryOp::And | HirBinaryOp::Or => {
            if left == Type::Command && right == Type::Command {
                return Type::CommandResult;
            }
            if left != Type::Bool && left != Type::Error {
                ctx.error(
                    format!("Logical operator expects Bool, found {}", left),
                    b.span,
                );
            }
            if right != Type::Bool && right != Type::Error {
                ctx.error(
                    format!("Logical operator expects Bool, found {}", right),
                    b.span,
                );
            }
            Type::Bool
        }
        HirBinaryOp::Assign => {
            if let HirExpression::Member(m) = &*b.left {
                let obj_ty = check_expression(ctx, &m.object);
                if let Type::Struct(name) = &obj_ty {
                    if let Some(info) = ctx.structs.get(name) {
                        if let Some(field_info) = info.fields.get(&m.field) {
                            if !field_info.is_mutable {
                                ctx.error(
                                    format!(
                                        "Cannot assign to immutable field '{}' in struct '{}'",
                                        m.field, name
                                    ),
                                    b.span,
                                );
                            }
                        }
                    }
                }
            } else if !matches!(&*b.left, HirExpression::Name(_, _)) {
                // For now only Name and Member are assignable
                ctx.error("Invalid assignment target".to_string(), b.span);
            }

            if !types_compatible(&left, &right) {
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
            ctx.error(
                "ShiftRight operator is not yet supported in type checking".to_string(),
                b.span,
            );
            Type::Error
        }
    }
}

fn check_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Option<Type> {
    // Helper to get an argument by index (positional) or name
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        if idx < args.len() {
            match &args[idx] {
                HirCallArgument::Positional(e) => return Some(e),
                _ => {}
            }
        }
        args.iter().find_map(|a| match a {
            HirCallArgument::Named(n) if n.name == arg_name => Some(&n.value),
            _ => None,
        })
    };

    match name {
        "print" => {
            let mut newline_found = false;
            for arg in args {
                match arg {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e);
                        if !ty.is_printable() && ty != Type::Error {
                            ctx.error(
                                format!("'print' argument must be a printable type, found {}", ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        if n.name == "newline" {
                            if newline_found {
                                ctx.error("Duplicate named argument 'newline'", n.value.span());
                            }
                            newline_found = true;
                            let ty = check_expression(ctx, &n.value);
                            if ty != Type::Bool && ty != Type::Error {
                                ctx.error(
                                    format!("'newline' must be Bool, found {}", ty),
                                    n.value.span(),
                                );
                            }
                        } else {
                            ctx.error(
                                format!("Unknown named argument '{}' for 'print'", n.name),
                                n.value.span(),
                            );
                        }
                    }
                }
            }
            Some(Type::Unit)
        }
        "fmt" => {
            let mut separator_found = false;
            for arg in args {
                match arg {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e);
                        if !ty.is_printable() && ty != Type::Error {
                            ctx.error(
                                format!("'fmt' argument must be a printable type, found {}", ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        if n.name == "separator" {
                            if separator_found {
                                ctx.error("Duplicate named argument 'separator'", n.value.span());
                            }
                            separator_found = true;
                            let ty = check_expression(ctx, &n.value);
                            if ty != Type::String && ty != Type::Error {
                                ctx.error(
                                    format!("'separator' must be String, found {}", ty),
                                    n.value.span(),
                                );
                            }
                        } else {
                            ctx.error(
                                format!("Unknown named argument '{}' for 'fmt'", n.name),
                                n.value.span(),
                            );
                        }
                    }
                }
            }
            Some(Type::String)
        }
        "exit" => {
            if args.is_empty() {
                ctx.error(
                    format!(
                        "Function 'exit' expects at least 1 argument(s), found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                let arg_expr = get_arg(0, "code");
                if let Some(e) = arg_expr {
                    let p = check_expression(ctx, e);
                    if p != Type::Int && p != Type::Int64 && p != Type::Error {
                        ctx.error(
                            format!("Argument 1 of 'exit': expected Int or Int64, found {}", p),
                            e.span(),
                        );
                    }
                } else {
                    ctx.error("Missing argument 'code' for 'exit'", span);
                }
            }
            Some(Type::Unit)
        }
        "read_line" => {
            for arg in args {
                match arg {
                    HirCallArgument::Positional(_) => {
                        ctx.error("'read_line' expects 0 positional arguments", span);
                    }
                    HirCallArgument::Named(n) if n.name == "trim_newline" => {
                        let ty = check_expression(ctx, &n.value);
                        if ty != Type::Bool && ty != Type::Error {
                            ctx.error(
                                format!("'trim_newline' must be Bool, found {}", ty),
                                n.value.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        ctx.error(
                            format!("Unknown named argument '{}' for 'read_line'", n.name),
                            n.value.span(),
                        );
                    }
                }
            }
            Some(Type::Result(
                Box::new(Type::String),
                Box::new(Type::BuiltinError),
            ))
        }
        "cmd" => {
            if args.is_empty() {
                ctx.error(
                    format!(
                        "Function 'cmd' expects at least 1 positional argument (program), found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                let mut positional_count = 0;
                for arg in args {
                    match arg {
                        HirCallArgument::Positional(e) => {
                            let p = check_expression(ctx, e);
                            if p != Type::String && p != Type::Error {
                                ctx.error(
                                    format!(
                                        "'cmd' positional argument {}: expected String, found {}",
                                        positional_count + 1,
                                        p
                                    ),
                                    e.span(),
                                );
                            }
                            positional_count += 1;
                        }
                        HirCallArgument::Named(n) => {
                            let ty = check_expression(ctx, &n.value);
                            match n.name.as_str() {
                                "cwd" => {
                                    if ty != Type::String && ty != Type::Error {
                                        ctx.error(
                                            format!("'cwd' must be String, found {}", ty),
                                            n.value.span(),
                                        );
                                    }
                                }
                                "check" => {
                                    if ty != Type::Bool && ty != Type::Error {
                                        ctx.error(
                                            format!("'check' must be Bool, found {}", ty),
                                            n.value.span(),
                                        );
                                    }
                                }
                                _ => {
                                    ctx.error(
                                        format!("Unknown named argument '{}' for 'cmd'", n.name),
                                        n.value.span(),
                                    );
                                }
                            }
                        }
                    }
                }
                if positional_count == 0 {
                    ctx.error("Missing program name for 'cmd'", span);
                }
            }
            Some(Type::Command)
        }
        "Ok" => {
            if args.len() != 1 {
                ctx.error(
                    format!("Function 'Ok' expects 1 argument(s), found {}", args.len()),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &n.value,
                };
                let ty = check_expression(ctx, arg_expr);
                Some(Type::Result(Box::new(ty), Box::new(Type::Unknown)))
            }
        }
        "Err" => {
            if args.len() != 1 {
                ctx.error(
                    format!("Function 'Err' expects 1 argument(s), found {}", args.len()),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &n.value,
                };
                let err_ty = check_expression(ctx, arg_expr);
                Some(Type::Result(Box::new(Type::Unknown), Box::new(err_ty)))
            }
        }
        "Some" => {
            if args.len() != 1 {
                ctx.error(
                    format!(
                        "Function 'Some' expects 1 argument(s), found {}",
                        args.len()
                    ),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &n.value,
                };
                let inner_ty = check_expression(ctx, arg_expr);
                Some(Type::Option(Box::new(inner_ty)))
            }
        }
        "crash" => {
            if args.is_empty() {
                ctx.error(
                    format!(
                        "Function 'crash' expects at least 1 argument(s), found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "message") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("Argument 1 of 'crash': expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Some(Type::Unit)
        }
        "expect" => {
            if args.len() < 1 {
                ctx.error(
                    format!(
                        "Function 'expect' expects at least 1 argument(s), found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "condition") {
                    let p = check_expression(ctx, e);
                    if p != Type::Bool && p != Type::Error {
                        ctx.error(
                            format!("Argument 1 of 'expect': expected Bool, found {}", p),
                            e.span(),
                        );
                    }
                }
                if let Some(e) = get_arg(1, "message") {
                    let m = check_expression(ctx, e);
                    if m != Type::String && m != Type::Error {
                        ctx.error(
                            format!("Argument 2 of 'expect': expected String, found {}", m),
                            e.span(),
                        );
                    }
                }
            }
            Some(Type::Unit)
        }
        "dbg" => {
            if args.is_empty() {
                ctx.error(
                    format!(
                        "Function 'dbg' expects at least 1 argument(s), found {}",
                        args.len()
                    ),
                    span,
                );
                Some(Type::Error)
            } else {
                let mut label_found = false;
                let mut first_val_ty = Type::Error;
                for (i, arg) in args.iter().enumerate() {
                    match arg {
                        HirCallArgument::Positional(e) => {
                            if i == 0 {
                                first_val_ty = check_expression(ctx, e);
                            } else {
                                ctx.error("'dbg' expects only 1 positional argument", e.span());
                            }
                        }
                        HirCallArgument::Named(n) => {
                            if n.name == "label" {
                                if label_found {
                                    ctx.error("Duplicate named argument 'label'", n.value.span());
                                }
                                label_found = true;
                                let ty = check_expression(ctx, &n.value);
                                if ty != Type::String && ty != Type::Error {
                                    ctx.error(
                                        format!("'label' must be String, found {}", ty),
                                        n.value.span(),
                                    );
                                }
                            } else {
                                ctx.error(
                                    format!("Unknown named argument '{}' for 'dbg'", n.name),
                                    n.value.span(),
                                );
                            }
                        }
                    }
                }
                Some(first_val_ty)
            }
        }
        _ => None,
    }
}

fn check_fs_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        if idx < args.len() {
            match &args[idx] {
                HirCallArgument::Positional(e) => return Some(e),
                _ => {}
            }
        }
        args.iter().find_map(|a| match a {
            HirCallArgument::Named(n) if n.name == arg_name => Some(&n.value),
            _ => None,
        })
    };

    match name {
        "read" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'fs.read' expects 1 argument(s), found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "path") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.read' argument 1: expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::String), Box::new(Type::BuiltinError))
        }
        "write" | "append" => {
            if args.len() != 2 {
                ctx.error(
                    format!("'fs.{}' expects 2 argument(s), found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e1) = get_arg(0, "path") {
                    let p = check_expression(ctx, e1);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 1: expected String, found {}", name, p),
                            e1.span(),
                        );
                    }
                }
                if let Some(e2) = get_arg(1, "text") {
                    let t = check_expression(ctx, e2);
                    if t != Type::String && t != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 2: expected String, found {}", name, t),
                            e2.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError))
        }
        "exists" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'fs.exists' expects 1 argument(s), found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "path") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.exists' argument 1: expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::Bool), Box::new(Type::BuiltinError))
        }
        "remove" | "mkdir" | "list_dir" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'fs.{}' expects 1 argument(s), found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "path") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 1: expected String, found {}", name, p),
                            e.span(),
                        );
                    }
                }
            }
            if name == "list_dir" {
                Type::Result(
                    Box::new(Type::Array(Box::new(Type::String))),
                    Box::new(Type::BuiltinError),
                )
            } else {
                Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError))
            }
        }
        "rename" | "copy" => {
            if args.len() != 2 {
                ctx.error(
                    format!("'fs.{}' expects 2 argument(s), found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e1) = get_arg(0, "from") {
                    let p = check_expression(ctx, e1);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 1: expected String, found {}", name, p),
                            e1.span(),
                        );
                    }
                }
                if let Some(e2) = get_arg(1, "to") {
                    let t = check_expression(ctx, e2);
                    if t != Type::String && t != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 2: expected String, found {}", name, t),
                            e2.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError))
        }
        _ => {
            ctx.error(format!("Unknown filesystem operation 'fs.{}'", name), span);
            Type::Error
        }
    }
}

fn check_env_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        if idx < args.len() {
            match &args[idx] {
                HirCallArgument::Positional(e) => return Some(e),
                _ => {}
            }
        }
        args.iter().find_map(|a| match a {
            HirCallArgument::Named(n) if n.name == arg_name => Some(&n.value),
            _ => None,
        })
    };

    match name {
        "get" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'env.get' expects 1 argument(s), found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "name") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'env.get' argument 1: expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::String), Box::new(Type::BuiltinError))
        }
        _ => {
            ctx.error(
                format!("Unknown environment operation 'env.{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_os_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        if idx < args.len() {
            match &args[idx] {
                HirCallArgument::Positional(e) => return Some(e),
                _ => {}
            }
        }
        args.iter().find_map(|a| match a {
            HirCallArgument::Named(n) if n.name == arg_name => Some(&n.value),
            _ => None,
        })
    };

    match name {
        "args" => {
            if args.len() != 0 {
                ctx.error(
                    format!("'os.args' expects 0 argument(s), found {}", args.len()),
                    span,
                );
            }
            Type::Array(Box::new(Type::String))
        }
        "cwd" => {
            if args.len() != 0 {
                ctx.error(
                    format!("'os.cwd' expects 0 argument(s), found {}", args.len()),
                    span,
                );
            }
            Type::Result(Box::new(Type::String), Box::new(Type::BuiltinError))
        }
        "chdir" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'os.chdir' expects 1 argument(s), found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_arg(0, "path") {
                    let p = check_expression(ctx, e);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'os.chdir' argument 1: expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Type::Result(Box::new(Type::Unit), Box::new(Type::BuiltinError))
        }
        _ => {
            ctx.error(format!("Unknown OS operation 'os.{}'", name), span);
            Type::Error
        }
    }
}

fn check_string_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        if idx < args.len() {
            match &args[idx] {
                HirCallArgument::Positional(e) => return Some(e),
                _ => {}
            }
        }
        args.iter().find_map(|a| match a {
            HirCallArgument::Named(n) if n.name == arg_name => Some(&n.value),
            _ => None,
        })
    };

    match name {
        "len" | "size" => {
            if !args.is_empty() {
                ctx.error(format!("'{}' method expects 0 arguments", name), span);
            }
            Type::Int
        }
        "bytes" => {
            if !args.is_empty() {
                ctx.error("'bytes' method expects 0 arguments", span);
            }
            Type::Slice(Box::new(Type::UInt8))
        }
        "trim" | "trim_start" | "trim_end" | "to_lower" | "to_upper" => {
            if !args.is_empty() {
                ctx.error(format!("'{}' method expects 0 arguments", name), span);
            }
            Type::String
        }
        "contains" | "starts_with" | "ends_with" => {
            if args.len() != 1 {
                ctx.error(format!("'{}' method expects 1 argument", name), span);
            } else if let Some(arg) = get_arg(0, "pattern")
                .or_else(|| get_arg(0, "substr"))
                .or_else(|| get_arg(0, "prefix"))
                .or_else(|| get_arg(0, "suffix"))
            {
                let ty = check_expression(ctx, arg);
                if ty != Type::String && ty != Type::Error {
                    ctx.error(
                        format!("'{}' expects String, found {}", name, ty),
                        arg.span(),
                    );
                }
            }
            Type::Bool
        }
        "split" => {
            if args.len() != 1 {
                ctx.error("'split' method expects 1 argument", span);
            } else if let Some(arg) = get_arg(0, "separator") {
                let ty = check_expression(ctx, arg);
                if ty != Type::String && ty != Type::Error {
                    ctx.error(format!("'split' expects String, found {}", ty), arg.span());
                }
            }
            Type::Array(Box::new(Type::String))
        }
        "replace" => {
            if args.len() != 2 {
                ctx.error("'replace' method expects 2 arguments", span);
            } else {
                if let Some(arg) = get_arg(0, "from") {
                    let ty = check_expression(ctx, arg);
                    if ty != Type::String && ty != Type::Error {
                        ctx.error(
                            format!("'replace' from expects String, found {}", ty),
                            arg.span(),
                        );
                    }
                }
                if let Some(arg) = get_arg(1, "to") {
                    let ty = check_expression(ctx, arg);
                    if ty != Type::String && ty != Type::Error {
                        ctx.error(
                            format!("'replace' to expects String, found {}", ty),
                            arg.span(),
                        );
                    }
                }
            }
            Type::String
        }
        _ => {
            ctx.error(format!("Unknown String method '{}'", name), span);
            Type::Error
        }
    }
}

fn check_array_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    match name {
        "len" => {
            if !args.is_empty() {
                ctx.error("'len' method expects 0 arguments", span);
            }
            Type::Int
        }
        _ => {
            ctx.error(format!("Unknown Array method '{}'", name), span);
            Type::Error
        }
    }
}

fn check_call(ctx: &mut TypeContext, c: &HirCall) -> Type {
    // For now we only support direct name calls
    let func_name = match &*c.callee {
        HirExpression::Name(_, name) => name.clone(),
        HirExpression::Member(m) => {
            if let HirExpression::Name(_, ref obj_name) = *m.object {
                if obj_name == "fs" {
                    return self::check_fs_builtin(ctx, &m.field, &c.args, c.span);
                }
                if obj_name == "env" {
                    return self::check_env_builtin(ctx, &m.field, &c.args, c.span);
                }
                if obj_name == "os" {
                    return self::check_os_builtin(ctx, &m.field, &c.args, c.span);
                }
                if obj_name == "Regex" {
                    return self::check_regex_constructor(ctx, &m.field, &c.args, c.span);
                }
            }
            let object_ty = check_expression(ctx, &m.object);
            if object_ty == Type::String {
                return self::check_string_method(ctx, &m.field, &c.args, c.span);
            }
            if let Type::Array(_) | Type::Slice(_) = object_ty {
                return self::check_array_method(ctx, &m.field, &c.args, c.span);
            }
            if object_ty == Type::Regex {
                return self::check_regex_method(ctx, &m.field, &c.args, c.span);
            }
            if object_ty == Type::Command && m.field == "run" {
                if !c.args.is_empty() {
                    ctx.error(
                        format!(
                            "Method 'run' on Command expects 0 argument(s), found {}",
                            c.args.len()
                        ),
                        c.span,
                    );
                }
                return Type::Result(Box::new(Type::CommandResult), Box::new(Type::BuiltinError));
            }
            ctx.error(
                format!(
                    "Method '{}' is not supported for type {}",
                    m.field, object_ty
                ),
                c.span,
            );
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

    // (Future: visibility enforcement here for cross-module)

    // Generic Named Argument Matching for user functions
    let mut resolved_args = vec![None; sig.params.len()];
    let mut named_keys = std::collections::HashSet::new();

    for (i, arg) in c.args.iter().enumerate() {
        match arg {
            HirCallArgument::Positional(e) => {
                if i >= sig.params.len() {
                    ctx.error(
                        format!("Too many positional arguments for '{}'", func_name),
                        e.span(),
                    );
                    continue;
                }
                resolved_args[i] = Some(e);
            }
            HirCallArgument::Named(n) => {
                if !named_keys.insert(n.name.clone()) {
                    // Parser already rejects duplicate named args in CallExpr,
                    // but good to be safe if HIR somehow bypasses it in future.
                    ctx.error(
                        format!("Duplicate named argument '{}'", n.name),
                        n.value.span(),
                    );
                    continue;
                }

                let param_idx = sig.params.iter().position(|(name, _)| name == &n.name);
                match param_idx {
                    Some(idx) => {
                        if resolved_args[idx].is_some() {
                            ctx.error(
                                format!("Argument for parameter '{}' already provided", n.name),
                                n.value.span(),
                            );
                        } else {
                            resolved_args[idx] = Some(&n.value);
                        }
                    }
                    None => {
                        ctx.error(
                            format!(
                                "Unknown named argument '{}' for function '{}'",
                                n.name, func_name
                            ),
                            n.value.span(),
                        );
                    }
                }
            }
        }
    }

    // Check types and missing arguments
    for (i, resolved) in resolved_args.iter().enumerate() {
        let (param_name, param_ty) = &sig.params[i];
        match resolved {
            Some(arg_expr) => {
                let arg_ty = check_expression(ctx, arg_expr);
                if arg_ty != Type::Error && !types_compatible(param_ty, &arg_ty) {
                    ctx.error(
                        format!(
                            "Argument for '{}' in '{}': expected {}, found {}",
                            param_name, func_name, param_ty, arg_ty
                        ),
                        arg_expr.span(),
                    );
                }
            }
            None => {
                ctx.error(
                    format!(
                        "Missing required argument '{}' for function '{}'",
                        param_name, func_name
                    ),
                    c.span,
                );
            }
        }
    }

    sig.return_type
}

fn check_member(ctx: &mut TypeContext, m: &HirMember) -> Type {
    let object_ty = check_expression(ctx, &m.object);
    if let Type::Struct(name) = &object_ty {
        if let Some(info) = ctx.structs.get(name.as_str()) {
            if let Some(field_info) = info.fields.get(&m.field) {
                // (Future: visibility enforcement here)
                return field_info.ty.clone();
            }
            ctx.error(
                format!("Struct '{}' has no field '{}'", name, m.field),
                m.span,
            );
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
        ctx.error(
            format!("Member access on non-struct type {}", object_ty),
            m.span,
        );
    }
    Type::Error
}

fn check_index(ctx: &mut TypeContext, i: &HirIndex) -> Type {
    let object_ty = check_expression(ctx, &i.object);
    let index_ty = check_expression(ctx, &i.index);

    if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
        if !index_ty.is_integer() && index_ty != Type::Error {
            ctx.error(format!("Index must be integer, found {}", index_ty), i.span);
        }
        return *elem.clone();
    }
    ctx.error(format!("Cannot index type {}", object_ty), i.span);
    Type::Error
}

fn check_slice(ctx: &mut TypeContext, s: &HirSlice) -> Type {
    let object_ty = check_expression(ctx, &s.object);
    let start_ty = check_expression(ctx, &s.start);
    let end_ty = check_expression(ctx, &s.end);

    if !start_ty.is_integer() && start_ty != Type::Error {
        ctx.error(
            format!("Slice start bound must be integer, found {}", start_ty),
            s.start.span(),
        );
    }
    if !end_ty.is_integer() && end_ty != Type::Error {
        ctx.error(
            format!("Slice end bound must be integer, found {}", end_ty),
            s.end.span(),
        );
    }

    if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
        return Type::Slice(elem.clone());
    }

    if object_ty != Type::Error {
        ctx.error(format!("Cannot slice type {}", object_ty), s.span);
    }
    Type::Error
}

fn check_block_expr(ctx: &mut TypeContext, block: &HirBlock) -> Type {
    crate::check_stmt::check_block(ctx, block)
}

fn get_branch_bindings(ctx: &mut TypeContext, expr: &HirExpression) -> Vec<(String, Type)> {
    match expr {
        HirExpression::Is(is_expr) => {
            if let HirExpression::Name(_, var_name) = &*is_expr.value {
                let narrowed_ty = ctx.resolve_hir_type(&is_expr.ty);
                if narrowed_ty != Type::Unknown {
                    if ctx.scopes.lookup(var_name).is_some() {
                        return vec![(var_name.clone(), narrowed_ty)];
                    }
                }
            }
            vec![]
        }
        HirExpression::IsPattern(i) => {
            let mut bindings = Vec::new();
            let val_ty = check_expression(ctx, &i.value);
            extract_pattern_bindings(&val_ty, &i.pattern, &mut bindings);
            bindings
        }
        _ => vec![],
    }
}

fn extract_pattern_bindings(val_ty: &Type, pattern: &HirPattern, out: &mut Vec<(String, Type)>) {
    match pattern {
        HirPattern::Identifier(_, name) => {
            out.push((name.clone(), val_ty.clone()));
        }
        HirPattern::EnumCase(_, name, args) => {
            match name.as_str() {
                "Some" => {
                    if let Type::Option(inner) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(inner, &args[0], out);
                        }
                    }
                }
                "Ok" => {
                    if let Type::Result(ok_ty, _) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(ok_ty, &args[0], out);
                        }
                    }
                }
                "Err" => {
                    if let Type::Result(_, err_ty) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(err_ty, &args[0], out);
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
}

fn check_if(ctx: &mut TypeContext, i: &HirIf) -> Type {
    let cond = check_expression(ctx, &i.condition);
    if cond != Type::Bool && cond != Type::Error {
        ctx.error(format!("If condition must be Bool, found {}", cond), i.span);
    }

    ctx.scopes.push_scope();
    let bindings = get_branch_bindings(ctx, &i.condition);
    for (name, ty) in bindings {
        ctx.scopes.insert(name, ty);
    }

    let then_ty = crate::check_stmt::check_block(ctx, &i.then_block);
    ctx.scopes.pop_scope();

    if let Some(else_branch) = &i.else_branch {
        let else_ty = check_expression(ctx, else_branch);
        if then_ty != Type::Error && else_ty != Type::Error && then_ty != else_ty {
            ctx.error(
                format!(
                    "If branches have incompatible types: {} vs {}",
                    then_ty, else_ty
                ),
                i.span,
            );
        }
        then_ty
    } else {
        Type::Unit
    }
}

fn check_match(ctx: &mut TypeContext, m: &HirMatch) -> Type {
    let val_ty = check_expression(ctx, &m.value);

    let mut result_ty: Option<Type> = None;

    let track_option = matches!(val_ty, Type::Option(_));
    let track_result = matches!(val_ty, Type::Result(_, _));

    let mut seen_some = false;
    let mut seen_none = false;
    let mut seen_ok   = false;
    let mut seen_err  = false;
    let mut seen_catch_all = false;

    for arm in &m.arms {
        // Arms after a catch-all are unreachable.
        if seen_catch_all {
            ctx.error(
                "Unreachable match arm: a previous catch-all already covers all remaining cases",
                arm.span,
            );
        }

        // Duplicate finite-constructor detection for Option / Result.
        if track_option || track_result {
            if let Some(name) = match_arm_constructor_name(&arm.pattern) {
                let is_dup = match name {
                    "Some" => seen_some,
                    "None" => seen_none,
                    "Ok"   => seen_ok,
                    "Err"  => seen_err,
                    _      => false,
                };
                if is_dup {
                    ctx.error(
                        format!("Duplicate '{}' arm: this constructor was already matched", name),
                        arm.span,
                    );
                }
            }
        }

        // Update coverage flags.
        match match_arm_kind(&arm.pattern) {
            MatchArmKind::CatchAll => seen_catch_all = true,
            MatchArmKind::Some     => seen_some = true,
            MatchArmKind::None     => seen_none = true,
            MatchArmKind::Ok       => seen_ok   = true,
            MatchArmKind::Err      => seen_err  = true,
            MatchArmKind::Other    => {}
        }

        // Validate pattern and typecheck arm body.
        if val_ty != Type::Error {
            validate_pattern_type(ctx, &val_ty, &arm.pattern);
        }

        ctx.scopes.push_scope();
        if val_ty != Type::Error {
            let mut bindings = Vec::new();
            extract_pattern_bindings(&val_ty, &arm.pattern, &mut bindings);
            for (name, ty) in bindings {
                ctx.scopes.insert(name, ty);
            }
        }
        let arm_ty = check_expression(ctx, &arm.value);
        ctx.scopes.pop_scope();

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

    // Emit non-exhaustive errors for supported scrutinee types.
    if val_ty != Type::Error && !seen_catch_all {
        if track_option && !(seen_some && seen_none) {
            let missing = match (seen_some, seen_none) {
                (false, false) => "Some(_) and None",
                (true,  false) => "None",
                (false, true)  => "Some(_)",
                _              => unreachable!(),
            };
            ctx.error(
                format!("Non-exhaustive match on Option: missing arm for {}", missing),
                m.span,
            );
        }
        if track_result && !(seen_ok && seen_err) {
            let missing = match (seen_ok, seen_err) {
                (false, false) => "Ok(_) and Err(_)",
                (true,  false) => "Err(_)",
                (false, true)  => "Ok(_)",
                _              => unreachable!(),
            };
            ctx.error(
                format!("Non-exhaustive match on Result: missing arm for {}", missing),
                m.span,
            );
        }
    }

    result_ty.unwrap_or(Type::Unit)
}

#[derive(Debug, PartialEq)]
enum MatchArmKind { CatchAll, Some, None, Ok, Err, Other }

fn match_arm_kind(pat: &HirPattern) -> MatchArmKind {
    match pat {
        HirPattern::Wildcard(_)            => MatchArmKind::CatchAll,
        HirPattern::Identifier(_, _)       => MatchArmKind::CatchAll,
        HirPattern::EnumCase(_, name, _) => match name.as_str() {
            "Some" => MatchArmKind::Some,
            "None" => MatchArmKind::None,
            "Ok"   => MatchArmKind::Ok,
            "Err"  => MatchArmKind::Err,
            _      => MatchArmKind::Other,
        },
        _ => MatchArmKind::Other,
    }
}

fn match_arm_constructor_name(pat: &HirPattern) -> Option<&'static str> {
    if let HirPattern::EnumCase(_, name, _) = pat {
        return match name.as_str() {
            "Some" => Some("Some"),
            "None" => Some("None"),
            "Ok"   => Some("Ok"),
            "Err"  => Some("Err"),
            _      => None,
        };
    }
    None
}

fn check_for_expr(ctx: &mut TypeContext, f: &HirForExpr) -> Type {
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
                format!(
                    "Array elements must have the same type: expected {}, found {}",
                    first, ty
                ),
                span,
            );
        }
    }
    Type::Array(Box::new(first))
}

fn check_regex_constructor(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let check_all_regex = |ctx: &mut TypeContext, args: &[HirCallArgument]| {
        for arg in args {
            match arg {
                HirCallArgument::Positional(e) => {
                    let ty = check_expression(ctx, e);
                    if ty != Type::Regex && ty != Type::Error {
                        ctx.error(format!("Expected Regex argument, found {}", ty), e.span());
                    }
                }
                HirCallArgument::Named(n) => {
                    ctx.error(
                        format!(
                            "Regex constructors do not support named argument '{}'",
                            n.name
                        ),
                        n.value.span(),
                    );
                }
            }
        }
    };

    match name {
        "text" => {
            if args.len() != 1 {
                ctx.error("'Regex.text' expects 1 String argument", span);
            } else {
                match &args[0] {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e);
                        if ty != Type::String && ty != Type::Error {
                            ctx.error(
                                format!("'Regex.text' expects String, found {}", ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        ctx.error(
                            format!(
                                "Regex constructors do not support named argument '{}'",
                                n.name
                            ),
                            n.value.span(),
                        );
                    }
                }
            }
            Type::Regex
        }
        "any" | "start" | "end" | "digit" | "space" | "word" => {
            if !args.is_empty() {
                ctx.error(format!("'Regex.{}' expects 0 arguments", name), span);
            }
            Type::Regex
        }
        "optional" | "zero_or_more" | "one_or_more" => {
            if args.len() != 1 {
                ctx.error(format!("'Regex.{}' expects 1 Regex argument", name), span);
            } else {
                match &args[0] {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e);
                        if ty != Type::Regex && ty != Type::Error {
                            ctx.error(
                                format!("'Regex.{}' expects Regex, found {}", name, ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        ctx.error(
                            format!(
                                "Regex constructors do not support named argument '{}'",
                                n.name
                            ),
                            n.value.span(),
                        );
                    }
                }
            }
            Type::Regex
        }
        "seq" | "alt" => {
            if args.len() < 2 {
                ctx.error(
                    format!("'Regex.{}' requires at least 2 Regex arguments", name),
                    span,
                );
            } else {
                check_all_regex(ctx, args);
            }
            Type::Regex
        }
        _ => {
            ctx.error(format!("Unknown Regex constructor 'Regex.{}'", name), span);
            Type::Error
        }
    }
}

fn check_regex_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &[HirCallArgument],
    span: Span,
) -> Type {
    let mut arg_tys: Vec<Type> = Vec::with_capacity(args.len());
    for arg in args {
        match arg {
            HirCallArgument::Positional(e) => arg_tys.push(check_expression(ctx, e)),
            HirCallArgument::Named(n) => {
                ctx.error(
                    format!("Regex methods do not support named argument '{}'", n.name),
                    n.value.span(),
                );
                arg_tys.push(Type::Error);
            }
        }
    }

    match name {
        "is_match" => {
            if args.len() != 1 {
                ctx.error("'is_match' expects 1 String argument", span);
            } else {
                let ty = arg_tys.first().cloned().unwrap_or(Type::Error);
                if ty != Type::String && ty != Type::Error {
                    ctx.error(format!("'is_match' expects String, found {}", ty), span);
                }
            }
            Type::Bool
        }
        "replace" => {
            if args.len() != 2 {
                ctx.error("'replace' expects 2 String arguments (text, with)", span);
            } else {
                for (idx, arg) in args.iter().enumerate() {
                    let ty = arg_tys.get(idx).cloned().unwrap_or(Type::Error);
                    if ty != Type::String && ty != Type::Error {
                        let at = match arg {
                            HirCallArgument::Positional(e) => e.span(),
                            HirCallArgument::Named(n) => n.value.span(),
                        };
                        ctx.error(
                            format!("'replace' expects String arguments, found {}", ty),
                            at,
                        );
                    }
                }
            }
            Type::String
        }
        _ => {
            ctx.error(format!("Unknown Regex method '{}'", name), span);
            Type::Error
        }
    }
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
    let mut provided = std::collections::HashSet::new();
    for (field_name, field_expr) in &s.fields {
        if !provided.insert(field_name.as_str()) {
            ctx.error(
                format!(
                    "Duplicate field '{}' in struct literal '{}'",
                    field_name, s.name
                ),
                field_expr.span(),
            );
        }

        let actual_ty = check_expression(ctx, field_expr);
        match info.fields.get(field_name.as_str()) {
            Some(field_info) => {
                let expected_ty = &field_info.ty;
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

    // Check that all required fields are present (those without defaults)
    for (field_name, field_info) in &info.fields {
        if !field_info.has_default && !provided.contains(field_name.as_str()) {
            ctx.error(
                format!(
                    "Missing required field '{}' in struct literal '{}'",
                    field_name, s.name
                ),
                s.span,
            );
        }
    }

    Type::Struct(s.name.clone())
}
