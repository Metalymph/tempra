//! Expression-level type checking logic.

use crate::context::TypeContext;
use crate::types::{types_compatible, Type};
use fero_hir::*;
use fero_span::Span;

fn get_call_arg_mut<'a>(args: &'a mut [HirCallArgument], idx: usize, arg_name: &str) -> Option<&'a mut HirExpression> {
    let mut found_idx = None;
    if idx < args.len() && matches!(&args[idx], HirCallArgument::Positional(_)) {
        found_idx = Some(idx);
    } else {
        for i in 0..args.len() {
            if let HirCallArgument::Named(n) = &args[i] {
                if n.name == arg_name {
                    found_idx = Some(i);
                    break;
                }
            }
        }
    }
    
    if let Some(i) = found_idx {
        match &mut args[i] {
            HirCallArgument::Positional(e) => Some(e),
            HirCallArgument::Named(n) => Some(&mut n.value),
        }
    } else {
        None
    }
}

/// Check an expression and return its inferred type.
pub fn check_expression(ctx: &mut TypeContext, expr: &mut HirExpression, expected: Option<&Type>) -> Type {
    let name_info = if let HirExpression::Name(span, name) = expr {
        Some((*span, name.clone()))
    } else {
        None
    };
    if let Some((span, name)) = name_info {
        return check_name(ctx, &name, span, expr, expected);
    }

    if matches!(expr, HirExpression::Call(_)) {
        return check_call(ctx, expr, expected);
    }

    match expr {
        HirExpression::Name(_, _) => unreachable!(),
        HirExpression::Integer(_, _) => Type::Int,
        HirExpression::Float(_, _) => Type::Float64,
        HirExpression::String(_, _) => Type::String,
        HirExpression::Bool(_, _) => Type::Bool,
        HirExpression::Unary(u) => check_unary(ctx, u),
        HirExpression::Binary(b) => check_binary(ctx, b, expected),
        HirExpression::IsType(i) => check_is_type(ctx, i),
        HirExpression::IsPattern(_) => check_is_pattern(ctx, expr),
        HirExpression::AsOptional(a) => check_as_optional(ctx, a),
        HirExpression::AsForced(a) => check_as_forced(ctx, a),
        HirExpression::Call(_) => unreachable!(),
        HirExpression::Member(m) => check_member(ctx, m, expected),
        HirExpression::Index(i) => check_index(ctx, i),
        HirExpression::Slice(s) => check_slice(ctx, s),
        HirExpression::Block(b) => check_block_expr(ctx, b, expected),
        HirExpression::If(i) => check_if(ctx, i, expected),
        HirExpression::Match(m) => check_match(ctx, m, expected),
        HirExpression::For(f) => check_for_expr(ctx, f),
        HirExpression::Tuple(span, elems) => check_tuple(ctx, elems, *span),
        HirExpression::Array(span, elems) => check_array(ctx, elems, *span, expected),
        HirExpression::StructLiteral(s) => check_struct_literal(ctx, s),
        HirExpression::Try(e) => check_try(ctx, e, expected),
        HirExpression::Unwrap(e) => check_unwrap(ctx, e),
        HirExpression::EnumConstructor(e) => Type::Enum(e.enum_name.clone()),
    }
}

fn is_valid_type(ty: &Type) -> bool {
    match ty {
        Type::Unknown | Type::Error => false,
        Type::Array(inner) => is_valid_type(inner),
        Type::Slice(inner) => is_valid_type(inner),
        Type::Tuple(elems) => elems.iter().all(is_valid_type),
        Type::Option(inner) => is_valid_type(inner),
        Type::Result(ok, err) => is_valid_type(ok) && is_valid_type(err),
        _ => true,
    }
}

fn check_is_type(ctx: &mut TypeContext, i: &mut HirIsTypeExpr) -> Type {
    let _ = check_expression(ctx, &mut i.value, None);
    
    if let HirType::Named(span, name) = &i.ty {
        if name.starts_with(|c: char| c.is_lowercase()) {
            ctx.error("naked binding patterns are not allowed after 'is'", *span);
            return Type::Error;
        }
    }
    
    let ty = ctx.resolve_hir_type(&i.ty);
    if !is_valid_type(&ty) {
        ctx.error(format!("invalid or unknown type in 'is' check: {}", ty), i.ty.span());
    }
    Type::Bool
}

fn check_is_pattern(ctx: &mut TypeContext, i: &mut HirExpression) -> Type {
    let is_pattern = match i {
        HirExpression::IsPattern(expr) => expr,
        _ => unreachable!(),
    };

    let val_ty = check_expression(ctx, &mut is_pattern.value, None);
    if val_ty == Type::Error {
        return Type::Bool;
    }

    if !crate::pattern::pattern_allowed_in_is(ctx, &is_pattern.pattern) {
        // Restrictions already logged
    }

    // If crate::pattern::validate_pattern_against_type determines this is a Type Test, it returns Some(Type).
    if let Some(target_ty) = crate::pattern::validate_pattern_against_type(ctx, &val_ty, &mut is_pattern.pattern) {
        // REWRITE: Transform IsPattern -> IsType
        // This ensures the interpreter uses its unified value_is_type logic.
        let inner = match std::mem::replace(i, HirExpression::Bool(Span::new(0, 0), false)) {
            HirExpression::IsPattern(p) => p,
            _ => unreachable!(),
        };
        
        *i = HirExpression::IsType(HirIsTypeExpr {
            span: inner.span,
            value: inner.value,
            // Construct a Named type node for the resolved type
            // Note: We use a dummy span because the original pattern span is already handled.
            ty: HirType::Named(inner.pattern.span(), target_ty.to_string()),
        });
        
        // Re-check as a normal 'IsType' expression to ensure full validation.
        if let HirExpression::IsType(ref mut res_is) = i {
            return check_is_type(ctx, res_is);
        }
    } else {
        // If it didn't resolve to a Type Test, it must be a pattern match.
        // For EnumCase patterns with no payload that aren't variants or types, report "unknown type".
        if let HirPattern::EnumCase(span, name, args) = &is_pattern.pattern {
             if args.is_empty() {
                 let is_variant = if let Type::Enum(enum_name) = &val_ty {
                     ctx.enums.get(enum_name).map(|info| info.variants.contains_key(name)).unwrap_or(false)
                 } else {
                     false
                 };
                 
                 let is_builtin = matches!(name.as_str(), "Some" | "None" | "Ok" | "Err");
                 
                 // Only report our error if it's not a variant of val_ty and not a built-in.
                 // Note: crate::pattern::validate_pattern_against_type might have already errored if val_ty was an Enum.
                 // But if val_ty is not an Enum (e.g. Int), crate::pattern::validate_pattern_against_type returns None silently for EnumCase.
                 if !is_variant && !is_builtin && !matches!(val_ty, Type::Enum(_)) {
                     ctx.error(format!("invalid or unknown type in 'is' check: {}", name), *span);
                 }
             }
        }
    }

    Type::Bool
}



fn check_as_optional(ctx: &mut TypeContext, a: &mut HirAsOptionalExpr) -> Type {
    let _ = check_expression(ctx, &mut a.expr, None);
    let ty = ctx.resolve_hir_type(&a.ty);
    if ty == Type::Unknown {
        ctx.error("unknown type in 'as?' cast".to_string(), a.ty.span());
    }
    Type::Option(Box::new(ty))
}

fn check_as_forced(ctx: &mut TypeContext, a: &mut HirAsForcedExpr) -> Type {
    let expr_ty = check_expression(ctx, &mut a.expr, None);
    let target_ty = ctx.resolve_hir_type(&a.ty);
    if target_ty == Type::Unknown {
        ctx.error("unknown type in 'as!' cast".to_string(), a.ty.span());
    }
    // Note: we track expr_ty only for the diagnostic annotation; at runtime as! can still fail
    let _ = expr_ty;
    target_ty
}

fn check_unwrap(ctx: &mut TypeContext, e: &mut HirUnwrapExpr) -> Type {
    let ty = check_expression(ctx, &mut e.expr, None);
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

fn check_try(ctx: &mut TypeContext, e: &mut HirTryExpr, _expected: Option<&Type>) -> Type {
    let ty = check_expression(ctx, &mut e.expr, None);

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

/// Semantic category of a resolved name.
enum ResolvedName {
    LocalVariable(Type),
    Namespace(String),
    BuiltinConstant(Type),
    BuiltinFunction(String),
    EnumConstructor(String, String), // enum_name, variant_name
    Function(String),
    Unknown,
}

/// Helper: Resolves a name to its fundamental semantic category.
fn resolve_name(ctx: &TypeContext, name: &str, expected: Option<&Type>) -> Result<ResolvedName, String> {
    if let Some(ty) = ctx.scopes.lookup(name) {
        return Ok(ResolvedName::LocalVariable(ty.clone()));
    }

    if matches!(name, "fs" | "os" | "env" | "Regex") {
        return Ok(ResolvedName::Namespace(name.to_string()));
    }

    if name == "None" {
        return Ok(ResolvedName::BuiltinConstant(Type::Option(Box::new(Type::Unknown))));
    }

    if matches!(name, "print" | "fmt" | "dbg" | "crash" | "expect" | "exit" | "cmd" | "read_line" | "Some" | "Ok" | "Err") {
        return Ok(ResolvedName::BuiltinFunction(name.to_string()));
    }

    if let Ok(enum_name) = ctx.find_enum_for_variant(name, expected) {
        return Ok(ResolvedName::EnumConstructor(enum_name, name.to_string()));
    } else if ctx.variant_to_enums.contains_key(name) {
        return Err(ctx.find_enum_for_variant(name, expected).unwrap_err());
    }

    if ctx.functions.contains_key(name) {
        return Ok(ResolvedName::Function(name.to_string()));
    }

    Ok(ResolvedName::Unknown)
}

fn check_name(
    ctx: &mut TypeContext,
    name: &str,
    span: Span,
    expr: &mut HirExpression,
    expected: Option<&Type>,
) -> Type {
    match resolve_name(ctx, name, expected) {
        Ok(ResolvedName::LocalVariable(ty)) => ty,
        Ok(ResolvedName::Namespace(ns)) => Type::Namespace(ns),
        Ok(ResolvedName::BuiltinConstant(ty)) => ty,
        Ok(ResolvedName::EnumConstructor(enum_name, variant_name)) => {
            // Type Validation
            let info = &ctx.enums[&enum_name];
            let payload = &info.variants[&variant_name];
            if !payload.is_empty() {
                ctx.error(
                    format!(
                        "Constructor '{}' of enum '{}' requires {} payload(s)",
                        variant_name,
                        enum_name,
                        payload.len()
                    ),
                    span,
                );
                return Type::Error;
            }

            // HIR Rewrite
            *expr = HirExpression::EnumConstructor(HirEnumConstructor {
                span,
                enum_name: enum_name.clone(),
                variant_name,
                payloads: vec![],
            });

            Type::Enum(enum_name)
        }
        Ok(ResolvedName::BuiltinFunction(f)) => {
            ctx.error(format!("built-in function '{}' cannot be used as a value", f), span);
            Type::Error
        }
        Ok(ResolvedName::Function(_)) => Type::Unknown,
        Ok(ResolvedName::Unknown) => {
            // Give a richer hint: if lowercase it might be a binding pattern misuse
            if name.starts_with(|c: char| c.is_uppercase()) {
                ctx.error(format!("unknown constructor or type '{}': no enum defines this variant and it is not in scope", name), span);
            } else {
                ctx.error(format!("undefined variable '{}'", name), span);
            }
            Type::Error
        }
        Err(msg) => {
            ctx.error(msg, span);
            Type::Error
        }
    }
}

/// Semantic category of a resolved callee in a call expression.
enum ResolvedCallee {
    Function(String),
    EnumConstructor(String, String), // enum_name, variant_name
    Method(Type, String),             // receiver_type, method_name
    Namespace(String, String),       // namespace_name, method_name
    BuiltinFunction(String),
    Unknown,
}

fn resolve_callee(ctx: &mut TypeContext, callee: &mut HirExpression, expected: Option<&Type>) -> ResolvedCallee {
    match callee {
        HirExpression::Name(_, name) => {
            match resolve_name(ctx, name, expected) {
                Ok(ResolvedName::Function(f)) => ResolvedCallee::Function(f),
                Ok(ResolvedName::BuiltinFunction(f)) => ResolvedCallee::BuiltinFunction(f),
                Ok(ResolvedName::EnumConstructor(e, v)) => ResolvedCallee::EnumConstructor(e, v),
                _ => ResolvedCallee::Unknown,
            }
        }
        HirExpression::Member(m) => {
            let obj_ty = check_expression(ctx, &mut m.object, None);
            match obj_ty {
                Type::Namespace(ns) => ResolvedCallee::Namespace(ns, m.field.clone()),
                _ => ResolvedCallee::Method(obj_ty, m.field.clone()),
            }
        }
        _ => ResolvedCallee::Unknown,
    }
}

fn check_unary(ctx: &mut TypeContext, u: &mut HirUnary) -> Type {
    let operand_ty = check_expression(ctx, &mut u.expr, None);
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

fn check_binary(ctx: &mut TypeContext, b: &mut HirBinary, _expected: Option<&Type>) -> Type {
    let left = check_expression(ctx, &mut b.left, None);
    let right = check_expression(ctx, &mut b.right, None);

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
            if let HirExpression::Member(m) = &mut *b.left {
                let obj_ty = check_expression(ctx, &mut m.object, None);
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
                ctx.error("invalid assignment target".to_string(), b.span);
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
            ctx.error("pipe operator expects Command types".to_string(), b.span);
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
    args: &mut [HirCallArgument],
    span: Span,
) -> Option<Type> {
    match name {
        "print" => {
            let mut newline_found = false;
            for arg in args {
                match arg {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e, None);
                        if !ty.is_printable() && ty != Type::Error {
                            ctx.error(
                                format!("'print' argument must be printable, found {}", ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        if n.name == "newline" {
                            if newline_found {
                                ctx.error("duplicate named argument 'newline'", n.value.span());
                            }
                            newline_found = true;
                            let ty = check_expression(ctx, &mut n.value, None);
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
                    HirCallArgument::Positional(ref mut e) => {
                        let ty = check_expression(ctx, e, None);
                        if !ty.is_printable() && ty != Type::Error {
                            ctx.error(
                                format!("'fmt' argument must be printable, found {}", ty),
                                e.span(),
                            );
                        }
                    }
                    HirCallArgument::Named(n) => {
                        if n.name == "separator" {
                            if separator_found {
                                ctx.error("duplicate named argument 'separator'", n.value.span());
                            }
                            separator_found = true;
                            let ty = check_expression(ctx, &mut n.value, None);
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
                        "function 'exit' expects at least 1 argument, found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "code") {
                    let p = check_expression(ctx, e, None);
                    if p != Type::Int && p != Type::Int64 && p != Type::Error {
                        ctx.error(
                            format!("argument 1 of 'exit': expected Int, found {}", p),
                            e.span(),
                        );
                    }
                } else {
                    ctx.error("missing argument 'code' for 'exit'", span);
                }
            }
            Some(Type::Never)
        }
        "read_line" => {
            for arg in args {
                match arg {
                    HirCallArgument::Positional(_) => {
                        ctx.error("'read_line' expects 0 positional arguments", span);
                    }
                    HirCallArgument::Named(n) if n.name == "trim_newline" => {
                        let ty = check_expression(ctx, &mut n.value, None);
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
                        "function 'cmd' expects at least 1 positional argument (program), found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                let mut positional_count = 0;
                for arg in args {
                    match arg {
                        HirCallArgument::Positional(e) => {
                            let p = check_expression(ctx, e, None);
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
                            let ty = check_expression(ctx, &mut n.value, None);
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
                    ctx.error("missing program name for 'cmd'", span);
                }
            }
            Some(Type::Command)
        }
        "Ok" => {
            if args.len() != 1 {
                ctx.error(
                    format!("function 'Ok' expects 1 argument, found {}", args.len()),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &mut args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &mut n.value,
                };
                let ty = check_expression(ctx, arg_expr, None);
                Some(Type::Result(Box::new(ty), Box::new(Type::Unknown)))
            }
        }
        "Err" => {
            if args.len() != 1 {
                ctx.error(
                    format!("function 'Err' expects 1 argument, found {}", args.len()),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &mut args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &mut n.value,
                };
                let err_ty = check_expression(ctx, arg_expr, None);
                Some(Type::Result(Box::new(Type::Unknown), Box::new(err_ty)))
            }
        }
        "Some" => {
            if args.len() != 1 {
                ctx.error(
                    format!(
                        "function 'Some' expects 1 argument, found {}",
                        args.len()
                    ),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &mut args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &mut n.value,
                };
                let inner_ty = check_expression(ctx, arg_expr, None);
                Some(Type::Option(Box::new(inner_ty)))
            }
        }
        "crash" => {
            if args.is_empty() {
                ctx.error(
                    format!(
                        "function 'crash' expects at least 1 argument, found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "message") {
                    let p = check_expression(ctx, e, None);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("Argument 1 of 'crash': expected String, found {}", p),
                            e.span(),
                        );
                    }
                }
            }
            Some(Type::Never)
        }
        "expect" => {
            if args.len() < 1 {
                ctx.error(
                    format!(
                        "function 'expect' expects at least 1 argument, found {}",
                        args.len()
                    ),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "condition") {
                    let p = check_expression(ctx, e, None);
                    if p != Type::Bool && p != Type::Error {
                        ctx.error(
                            format!("Argument 1 of 'expect': expected Bool, found {}", p),
                            e.span(),
                        );
                    }
                }
                if let Some(e) = get_call_arg_mut(args, 1, "message") {
                    let m = check_expression(ctx, e, None);
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
                        "function 'dbg' expects at least 1 argument, found {}",
                        args.len()
                    ),
                    span,
                );
                Some(Type::Error)
            } else {
                let mut label_found = false;
                let mut first_val_ty = Type::Error;
                for (i, arg) in args.iter_mut().enumerate() {
                    match arg {
                        HirCallArgument::Positional(e) => {
                            if i == 0 {
                                first_val_ty = check_expression(ctx, e, None);
                            } else {
                                ctx.error("'dbg' expects only 1 positional argument", e.span());
                            }
                        }
                        HirCallArgument::Named(n) => {
                            if n.name == "label" {
                                if label_found {
                                    ctx.error("duplicate named argument 'label'", n.value.span());
                                }
                                label_found = true;
                                let ty = check_expression(ctx, &mut n.value, None);
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
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    match name {
        "read" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'fs.read' expects 1 argument, found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "path") {
                    let p = check_expression(ctx, e, None);
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
                    format!("'fs.{}' expects 2 arguments, found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e1) = get_call_arg_mut(args, 0, "path") {
                    let p = check_expression(ctx, e1, None);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 1: expected String, found {}", name, p),
                            e1.span(),
                        );
                    }
                }
                if let Some(e2) = get_call_arg_mut(args, 1, "text") {
                    let t = check_expression(ctx, e2, None);
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
                    format!("'fs.exists' expects 1 argument, found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "path") {
                    let p = check_expression(ctx, e, None);
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
                    format!("'fs.{}' expects 1 argument, found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "path") {
                    let p = check_expression(ctx, e, None);
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
                    format!("'fs.{}' expects 2 arguments, found {}", name, args.len()),
                    span,
                );
            } else {
                if let Some(e1) = get_call_arg_mut(args, 0, "from") {
                    let p = check_expression(ctx, e1, None);
                    if p != Type::String && p != Type::Error {
                        ctx.error(
                            format!("'fs.{}' argument 1: expected String, found {}", name, p),
                            e1.span(),
                        );
                    }
                }
                if let Some(e2) = get_call_arg_mut(args, 1, "to") {
                    let t = check_expression(ctx, e2, None);
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
            ctx.error(
                format!("namespace 'fs' has no member '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_env_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    match name {
        "get" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'env.get' expects 1 argument, found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "name") {
                    let p = check_expression(ctx, e, None);
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
                format!("namespace 'env' has no member '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_os_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    match name {
        "args" => {
            if args.len() != 0 {
                ctx.error(
                    format!("'os.args' expects 0 arguments, found {}", args.len()),
                    span,
                );
            }
            Type::Array(Box::new(Type::String))
        }
        "cwd" => {
            if args.len() != 0 {
                ctx.error(
                    format!("'os.cwd' expects 0 arguments, found {}", args.len()),
                    span,
                );
            }
            Type::Result(Box::new(Type::String), Box::new(Type::BuiltinError))
        }
        "chdir" => {
            if args.len() != 1 {
                ctx.error(
                    format!("'os.chdir' expects 1 argument, found {}", args.len()),
                    span,
                );
            } else {
                if let Some(e) = get_call_arg_mut(args, 0, "path") {
                    let p = check_expression(ctx, e, None);
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
            ctx.error(
                format!("namespace 'os' has no member '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_string_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
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
            } else {
                let mut arg_opt = get_call_arg_mut(args, 0, "pattern");
                if arg_opt.is_none() { arg_opt = get_call_arg_mut(args, 0, "substr"); }
                if arg_opt.is_none() { arg_opt = get_call_arg_mut(args, 0, "prefix"); }
                if arg_opt.is_none() { arg_opt = get_call_arg_mut(args, 0, "suffix"); }

                if let Some(arg) = arg_opt {
                    let ty = check_expression(ctx, arg, None);
                    if ty != Type::String && ty != Type::Error {
                        ctx.error(format!("'{}' expects String, found {}", name, ty), arg.span());
                    }
                }
            }
            Type::Bool
        }
        "split" => {
            if args.len() != 1 {
                ctx.error("'split' method expects 1 argument", span);
            } else if let Some(arg) = get_call_arg_mut(args, 0, "separator") {
                let ty = check_expression(ctx, arg, None);
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
                if let Some(arg) = get_call_arg_mut(args, 0, "from") {
                    let ty = check_expression(ctx, arg, None);
                    if ty != Type::String && ty != Type::Error {
                        ctx.error(
                            format!("'replace' from expects String, found {}", ty),
                            arg.span(),
                        );
                    }
                }
                if let Some(arg) = get_call_arg_mut(args, 1, "to") {
                    let ty = check_expression(ctx, arg, None);
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
            ctx.error(
                format!("String has no method '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_array_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
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
            ctx.error(
                format!("Array/Slice has no method '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_command_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    match name {
        "run" => {
            if !args.is_empty() {
                ctx.error("'run' expects 0 arguments", span);
            }
            Type::Result(Box::new(Type::CommandResult), Box::new(Type::BuiltinError))
        }
        _ => {
            ctx.error(
                format!("Command has no method '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_call(ctx: &mut TypeContext, expr: &mut HirExpression, expected: Option<&Type>) -> Type {
    let callee_copy = if let HirExpression::Call(c) = expr {
        Some((&mut *c.callee, c.span))
    } else {
        None
    };

    let (callee_node, call_span) = match callee_copy {
        Some(res) => res,
        None => unreachable!(),
    };

    let resolved = resolve_callee(ctx, callee_node, expected);

    match resolved {
        ResolvedCallee::Function(name) | ResolvedCallee::BuiltinFunction(name) => {
            if let HirExpression::Call(ref mut c) = expr {
                let type_args = c.type_args.as_ref().map(|args| {
                    args.iter().map(|a| ctx.resolve_hir_type(a)).collect()
                });
                return check_function_call(ctx, c, &name, type_args, expected);
            }
        }
        ResolvedCallee::EnumConstructor(enum_name, variant_name) => {
            // Type Validation & Rewrite
            let payload_tys = ctx.enums[&enum_name].variants[&variant_name].clone();
            let mut error_occurred = false;

            if let HirExpression::Call(ref mut c) = expr {
                // Arity & Arity v1 limit
                if c.args.len() != payload_tys.len() {
                    ctx.error(format!("constructor '{}' of enum '{}' expects {} arguments, found {}", variant_name, enum_name, payload_tys.len(), c.args.len()), c.span);
                    error_occurred = true;
                }
                if c.args.len() > 1 {
                    ctx.error(format!("Constructor '{}' has {} payloads; Fero v1 only supports 0 or 1 payload.", variant_name, c.args.len()), c.span);
                }

                // Arg Types
                for (i, arg) in c.args.iter_mut().enumerate() {
                    let arg_expr = get_call_arg_mut_legacy(arg, ctx);
                    let expected_arg_ty = payload_tys.get(i);
                    let arg_ty = check_expression(ctx, arg_expr, expected_arg_ty);
                    
                    if let Some(target_ty) = expected_arg_ty {
                        if arg_ty != Type::Error && !types_compatible(target_ty, &arg_ty) {
                            ctx.error(format!("Constructor '{}' argument {}: expected {}, found {}", variant_name, i + 1, target_ty, arg_ty), arg_expr.span());
                        }
                    }
                }
            }

            if error_occurred { return Type::Error; }

            // REWRITE
            if let HirExpression::Call(c) = std::mem::replace(expr, HirExpression::Integer(call_span, "0".to_string())) {
                 let payloads = c.args.into_iter().filter_map(|a| if let HirCallArgument::Positional(e) = a { Some(e) } else { None }).collect();
                 *expr = HirExpression::EnumConstructor(HirEnumConstructor {
                     span: call_span,
                     enum_name: enum_name.clone(),
                     variant_name,
                     payloads,
                 });
            }
            return Type::Enum(enum_name);
        }
        ResolvedCallee::Namespace(ns, method) => {
            if let HirExpression::Call(ref mut c) = expr {
                match ns.as_str() {
                    "fs" => return check_fs_builtin(ctx, &method, &mut c.args, c.span),
                    "env" => return check_env_builtin(ctx, &method, &mut c.args, c.span),
                    "os" => return check_os_builtin(ctx, &method, &mut c.args, c.span),
                    "Regex" => return check_regex_constructor(ctx, &method, &mut c.args, c.span),
                    _ => {
                        ctx.error(format!("Unknown namespace '{}'", ns), c.span);
                        return Type::Error;
                    }
                }
            }
        }
        ResolvedCallee::Method(receiver_ty, method) => {
            if let HirExpression::Call(ref mut c) = expr {
                match &receiver_ty {
                    Type::String => return check_string_method(ctx, &method, &mut c.args, c.span),
                    Type::Array(_) | Type::Slice(_) => return check_array_method(ctx, &method, &mut c.args, c.span),
                    Type::Regex => return check_regex_method(ctx, &method, &mut c.args, c.span),
                    Type::Command => return check_command_method(ctx, &method, &mut c.args, c.span),
                    _ => {
                        // Regular member call on struct literal or something similar?
                        // Fero doesn't have methods on structs yet, but we check field access.
                        // If it's a field of type function, we'd call it.
                        // For now, return error as "Method not found"
                        ctx.error(format!("type {} has no method '{}'", receiver_ty, method), c.span);
                        return Type::Error;
                    }
                }
            }
        }
        ResolvedCallee::Unknown => {
            if let HirExpression::Call(ref mut c) = expr {
                // Try to evaluate the callee to produce a better error
                let callee_ty = check_expression(ctx, &mut c.callee, None);
                if callee_ty != Type::Error {
                    ctx.error(
                        format!("value of type {} is not callable", callee_ty),
                        c.span,
                    );
                }
                // If callee_ty is Error, an error was already emitted by check_name/check_expression
                return Type::Error;
            }
        }
    }
    unreachable!()
}

fn match_generic_types(
    param_ty: &Type,
    arg_ty: &Type,
    inferred: &mut std::collections::HashMap<crate::types::GenericParamId, Type>,
) -> Result<(), ()> {
    match (param_ty, arg_ty) {
        (Type::GenericParam { id, .. }, _) => {
            if let Some(existing) = inferred.get(id) {
                if existing != arg_ty {
                    return Err(());
                }
            } else {
                inferred.insert(*id, arg_ty.clone());
            }
            Ok(())
        }
        (Type::Array(p), Type::Array(a)) => match_generic_types(p, a, inferred),
        (Type::Slice(p), Type::Slice(a)) => match_generic_types(p, a, inferred),
        (Type::Option(p), Type::Option(a)) => match_generic_types(p, a, inferred),
        (Type::Result(p_ok, p_err), Type::Result(a_ok, a_err)) => {
            match_generic_types(p_ok, a_ok, inferred)?;
            match_generic_types(p_err, a_err, inferred)
        }
        (Type::Tuple(p), Type::Tuple(a)) if p.len() == a.len() => {
            for (pt, at) in p.iter().zip(a.iter()) {
                match_generic_types(pt, at, inferred)?;
            }
            Ok(())
        }
        _ => {
            if param_ty == arg_ty {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

fn substitute_generics(ty: &Type, map: &std::collections::HashMap<crate::types::GenericParamId, Type>) -> Type {
    match ty {
        Type::GenericParam { id, .. } => {
            map.get(id).cloned().unwrap_or_else(|| ty.clone())
        }
        Type::Array(inner) => Type::Array(Box::new(substitute_generics(inner, map))),
        Type::Slice(inner) => Type::Slice(Box::new(substitute_generics(inner, map))),
        Type::Option(inner) => Type::Option(Box::new(substitute_generics(inner, map))),
        Type::Result(ok, err) => Type::Result(Box::new(substitute_generics(ok, map)), Box::new(substitute_generics(err, map))),
        Type::Tuple(elems) => Type::Tuple(elems.iter().map(|e| substitute_generics(e, map)).collect()),
        _ => ty.clone(),
    }
}

fn get_call_arg_mut_legacy<'a>(arg: &'a mut HirCallArgument, ctx: &mut TypeContext) -> &'a mut HirExpression {
    match arg {
        HirCallArgument::Positional(ref mut e) => e,
        HirCallArgument::Named(n) => {
                ctx.error("named arguments not supported here", n.value.span());
            &mut n.value
        }
    }
}

fn check_function_call(
    ctx: &mut TypeContext,
    c: &mut HirCall,
    func_name: &str,
    explicit_type_args: Option<Vec<Type>>,
    _expected: Option<&Type>,
) -> Type {
    if let Some(builtin_ty) = check_builtin(ctx, func_name, &mut c.args, c.span) {
        return builtin_ty;
    }

    let sig = match ctx.functions.get(func_name) {
        Some(sig) => sig.clone(),
        None => {
            ctx.error(format!("undefined function '{}'", func_name), c.span);
            return Type::Error;
        }
    };

    // Generic Named Argument Matching for user functions
    let mut resolved_args: Vec<Option<&mut HirExpression>> = (0..sig.params.len()).map(|_| None).collect();
    let mut named_keys = std::collections::HashSet::new();

    for (i, arg) in c.args.iter_mut().enumerate() {
        match arg {
            HirCallArgument::Positional(e) => {
                if i >= sig.params.len() {
                    ctx.error(
                        format!("too many positional arguments for '{}'", func_name),
                        e.span(),
                    );
                    continue;
                }
                resolved_args[i] = Some(e);
            }
            HirCallArgument::Named(n) => {
                if !named_keys.insert(n.name.clone()) {
                    ctx.error(
                        format!("duplicate named argument '{}'", n.name),
                        n.value.span(),
                    );
                    continue;
                }

                let param_idx = sig.params.iter().position(|(name, _)| name == &n.name);
                match param_idx {
                    Some(idx) => {
                        if resolved_args[idx].is_some() {
                            ctx.error(
                                format!("argument for parameter '{}' already provided", n.name),
                                n.value.span(),
                            );
                        } else {
                            resolved_args[idx] = Some(&mut n.value);
                        }
                    }
                    None => {
                        ctx.error(
                            format!(
                                "unknown named argument '{}' for function '{}'",
                                n.name, func_name
                            ),
                            n.value.span(),
                        );
                    }
                }
            }
        }
    }

    // Evaluate arguments to infer types
    let mut arg_types = Vec::new();
    for resolved in &mut resolved_args {
        match resolved {
            Some(arg_expr) => {
                let arg_ty = check_expression(ctx, *arg_expr, None);
                arg_types.push(arg_ty);
            }
            None => arg_types.push(Type::Error),
        }
    }

    let mut inferred = std::collections::HashMap::new();
    if let Some(t_args) = explicit_type_args {
        if t_args.len() != sig.generic_params.len() {
            ctx.error(format!("function '{}' expects {} type arguments, found {}", func_name, sig.generic_params.len(), t_args.len()), c.span);
            return Type::Error;
        }
        for (param, ty) in sig.generic_params.iter().zip(t_args.into_iter()) {
            inferred.insert(param.id, ty);
        }
    } else {
        if !sig.generic_params.is_empty() {
            for (i, (_, param_ty)) in sig.params.iter().enumerate() {
                if let Some(arg_ty) = arg_types.get(i) {
                    if *arg_ty != Type::Error {
                        if match_generic_types(param_ty, arg_ty, &mut inferred).is_err() {
                            ctx.error(format!("type mismatch during generic inference for '{}'", func_name), c.span);
                        }
                    }
                }
            }
            for param in &sig.generic_params {
                if !inferred.contains_key(&param.id) {
                    ctx.error(format!("cannot infer type for generic parameter '{}'", param.name), c.span);
                    return Type::Error;
                }
            }
        }
    }

    // Check types and missing arguments
    for (i, resolved) in resolved_args.iter_mut().enumerate() {
        let (param_name, param_ty) = &sig.params[i];
        let substituted_param_ty = substitute_generics(param_ty, &inferred);
        let arg_ty = &arg_types[i];

        if resolved.is_some() {
            if *arg_ty != Type::Error && !crate::types::types_compatible(&substituted_param_ty, arg_ty) {
                ctx.error(
                    format!(
                        "argument for '{}' in '{}': expected {}, found {}",
                        param_name, func_name, substituted_param_ty, arg_ty
                    ),
                    resolved.as_ref().unwrap().span(),
                );
            }
        } else {
            ctx.error(
                format!(
                    "missing required argument '{}' for function '{}'",
                    param_name, func_name
                ),
                c.span,
            );
        }
    }

    substitute_generics(&sig.return_type, &inferred)
}

fn check_member(ctx: &mut TypeContext, m: &mut HirMember, _expected: Option<&Type>) -> Type {
    let object_ty = check_expression(ctx, &mut m.object, None);
    if let Type::Struct(name) = &object_ty {
        if let Some(info) = ctx.structs.get(name.as_str()) {
            if let Some(field_info) = info.fields.get(&m.field) {
                return field_info.ty.clone();
            }
            ctx.error(
                format!("struct '{}' has no field '{}'", name, m.field),
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
                ctx.error(
                    format!("CommandResult has no field '{}'", m.field),
                    m.span,
                );
                return Type::Error;
            }
        }
    }
    if object_ty != Type::Error {
        // Distinguish: shadowed namespace used as a value vs. plain wrong type
        ctx.error(
            format!("type {} has no field or method '{}'", object_ty, m.field),
            m.span,
        );
    }
    Type::Error
}

fn check_index(ctx: &mut TypeContext, i: &mut HirIndex) -> Type {
    let object_ty = check_expression(ctx, &mut i.object, None);
    let index_ty = check_expression(ctx, &mut i.index, None);

    if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
        if !index_ty.is_integer() && index_ty != Type::Error {
            ctx.error(format!("index must be integer, found {}", index_ty), i.span);
        }
        return *elem.clone();
    }
    ctx.error(format!("Cannot index type {}", object_ty), i.span);
    Type::Error
}

fn check_slice(ctx: &mut TypeContext, s: &mut HirSlice) -> Type {
    let object_ty = check_expression(ctx, &mut s.object, None);
    let start_ty = check_expression(ctx, &mut s.start, None);
    let end_ty = check_expression(ctx, &mut s.end, None);

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
        ctx.error(format!("cannot slice type {}", object_ty), s.span);
    }
    Type::Error
}

fn check_block_expr(ctx: &mut TypeContext, block: &mut HirBlock, expected: Option<&Type>) -> Type {
    crate::check_stmt::check_block(ctx, block, expected)
}

fn build_refinement_from_condition(
    ctx: &mut TypeContext,
    expr: &mut HirExpression,
) -> crate::pattern::Refinement {
    match expr {
        HirExpression::IsType(i) => crate::pattern::refinement_from_is_type(ctx, &i.value, &i.ty),
        HirExpression::IsPattern(i) => {
            let val_ty = check_expression(ctx, &mut i.value, None);
            crate::pattern::refinement_from_is_pattern(ctx, &i.value, &mut i.pattern, val_ty)
        }
        HirExpression::Binary(b) if b.op == HirBinaryOp::And => {
            let mut left = build_refinement_from_condition(ctx, &mut b.left);
            let right = build_refinement_from_condition(ctx, &mut b.right);
            left.bindings.extend(right.bindings);
            // v2 behavior: keep first narrowing found
            left.narrowed_value = left.narrowed_value.or(right.narrowed_value);
            left
        }
        _ => crate::pattern::Refinement::empty(),
    }
}

fn unify_branch_types(t1: Type, t2: Type) -> Type {
    if t1 == Type::Error || t2 == Type::Error {
        return Type::Error;
    }
    if t1 == t2 {
        return t1;
    }
    if t1 == Type::Never {
        return t2;
    }
    if t2 == Type::Never {
        return t1;
    }
    Type::Error
}

fn check_if(ctx: &mut TypeContext, i: &mut HirIf, expected: Option<&Type>) -> Type {
    let cond = check_expression(ctx, &mut i.condition, Some(&Type::Bool));
    if cond != Type::Bool && cond != Type::Error {
        ctx.error(format!("If condition must be Bool, found {}", cond), i.span);
    }

    ctx.scopes.push_scope();
    let refinement = build_refinement_from_condition(ctx, &mut i.condition);
    refinement.apply(ctx);

    let then_ty = crate::check_stmt::check_block(ctx, &mut i.then_block, expected);
    ctx.scopes.pop_scope();

    if let Some(ref mut else_branch) = i.else_branch {
        let else_ty = check_expression(ctx, else_branch, expected);
        let unified = unify_branch_types(then_ty.clone(), else_ty.clone());
        if unified == Type::Error && then_ty != Type::Error && else_ty != Type::Error {
            ctx.error(
                format!(
                    "if branches have incompatible types: {} vs {}",
                    then_ty, else_ty
                ),
                i.span,
            );
            return Type::Error;
        }
        unified
    } else {
        Type::Unit
    }
}



fn check_match(ctx: &mut TypeContext, m: &mut HirMatch, expected: Option<&Type>) -> Type {
    let val_ty = check_expression(ctx, &mut m.value, None);
    let mut result_ty: Option<Type> = None;

    // ── Coverage Accumulation / Unreachable Detection ───────────────────────
    let mut seen_constructors: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut has_unconditional_wildcard = false;
    let mut covered_constructors: std::collections::HashSet<crate::pattern::MatchCoverageKey> =
        std::collections::HashSet::new();
    let mut is_fully_covered = false;

    for arm in m.arms.iter_mut() {
        if val_ty != Type::Error {
            crate::pattern::validate_pattern_against_type(ctx, &val_ty, &mut arm.pattern);
        }

        let has_guard = arm.guard.is_some();
        let sub_patterns = match &arm.pattern {
            HirPattern::Or(_, alts) => alts.clone(),
            _ => vec![arm.pattern.clone()],
        };

        for sub_pat in sub_patterns {
            let structural_space = crate::pattern::pattern_structural_space(&sub_pat);
            let coverage_space = crate::pattern::pattern_coverage_space(&sub_pat, has_guard);

            // 1. Unreachable Detection
            if has_unconditional_wildcard {
                ctx.error("unreachable match arm", arm.span);
            } else if let crate::pattern::Space::Constructor(name) = &structural_space {
                if seen_constructors.contains(name) && !has_guard {
                    ctx.error(
                        format!("duplicate match arm for variant '{}'", name),
                        arm.span,
                    );
                } else if seen_constructors.contains(name) {
                    ctx.error("unreachable match arm", arm.span);
                }
            }

            // 2. State Update (Ordering matters: non-guarded arms block same constructors later)
            if !has_guard {
                match &structural_space {
                    crate::pattern::Space::Wildcard => has_unconditional_wildcard = true,
                    crate::pattern::Space::Constructor(name) => {
                        seen_constructors.insert(name.clone());
                    }
                    _ => {}
                }
            }

            // 3. Exhaustiveness / Coverage Contribution
            match &coverage_space {
                crate::pattern::Space::Wildcard => is_fully_covered = true,
                crate::pattern::Space::Constructor(name) => {
                    // Map the top-level constructor string back to a typed key based on the scrutinee type
                    if let Some(key) = match &crate::pattern::domain_of_type(&val_ty) {
                        crate::pattern::Domain::Bool => match name.as_str() {
                            "true" => Some(crate::pattern::MatchCoverageKey::Bool(true)),
                            "false" => Some(crate::pattern::MatchCoverageKey::Bool(false)),
                            _ => None,
                        },
                        crate::pattern::Domain::Enum(enum_name) => Some(crate::pattern::MatchCoverageKey::Variant {
                            enum_name: enum_name.clone(),
                            variant: name.clone(),
                        }),
                        _ => None,
                    } {
                        covered_constructors.insert(key);
                    }
                }
                _ => {}
            }
        }

        // ── Pattern validation + arm typechecking ────────────────────────────
        ctx.scopes.push_scope();
        if val_ty != Type::Error {
            let mut bindings = Vec::new();
            crate::pattern::extract_pattern_bindings(ctx, &val_ty, &mut arm.pattern, &mut bindings);
            for (name, ty) in bindings {
                ctx.scopes.insert(name, ty);
            }
        }
        if let Some(ref mut guard) = arm.guard {
            let guard_ty = check_expression(ctx, guard, Some(&Type::Bool));
            if guard_ty != Type::Bool && guard_ty != Type::Error {
                ctx.error(
                    format!("match guard must be Bool, found {}", guard_ty),
                    guard.span(),
                );
            }
        }
        let arm_ty = check_expression(ctx, &mut arm.value, expected);
        ctx.scopes.pop_scope();

        if let Some(exp) = result_ty {
            let unified = unify_branch_types(exp.clone(), arm_ty.clone());
            if unified == Type::Error && exp != Type::Error && arm_ty != Type::Error {
                ctx.error(
                    format!("match arms have incompatible types: {} vs {}", exp, arm_ty),
                    arm.span,
                );
                result_ty = Some(Type::Error);
            } else {
                result_ty = Some(unified);
            }
        } else {
            result_ty = Some(arm_ty);
        }
    }

    // ── Final Exhaustiveness Check ───────────────────────────────────────────
    if val_ty != Type::Error && !is_fully_covered {
        let domain = crate::pattern::domain_of_type(&val_ty);
        if let Some(expected_cases) = crate::pattern::cases_of_domain(&domain, ctx) {
            let missing: Vec<_> = expected_cases
                .iter()
                .filter(|k| !covered_constructors.contains(k))
                .collect();

            if !missing.is_empty() {
                match domain {
                    crate::pattern::Domain::Bool => {
                        ctx.error(
                            format!(
                                "non-exhaustive match on Bool: missing arm for {}",
                                missing[0].display()
                            ),
                            m.span,
                        );
                    }
                    crate::pattern::Domain::Enum(name) => {
                        let message = if missing.len() == 1 {
                            format!(
                                "non-exhaustive match on enum '{}': missing arm for variant '{}'",
                                name,
                                missing[0].display()
                            )
                        } else {
                            let variants = missing
                                .iter()
                                .map(|v| format!("'{}'", v.display()))
                                .collect::<Vec<_>>()
                                .join(", ");
                            format!(
                                "non-exhaustive match on enum '{}': missing arms for variants {}",
                                name, variants
                            )
                        };
                        ctx.error(message, m.span);
                    }
                    _ => unreachable!("domain with cases must be Bool or Enum"),
                }
            }
        } else {
            // crate::pattern::Domain cannot be proven complete (Tuple, Struct, Unknown)
            // and we know !is_fully_covered.
            ctx.error(
                "non-exhaustive match: this domain requires an unguarded catch-all",
                m.span,
            );
        }
    }

    result_ty.unwrap_or(Type::Unit)
}


fn check_for_expr(ctx: &mut TypeContext, f: &mut HirForExpr) -> Type {
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

    let body_ty = crate::check_stmt::check_block(ctx, &mut f.body, None);
    ctx.scopes.pop_scope();

    Type::Array(Box::new(body_ty))
}

fn check_tuple(ctx: &mut TypeContext, elems: &mut [HirExpression], _span: Span) -> Type {
    let types: Vec<Type> = elems.iter_mut().map(|e| check_expression(ctx, e, None)).collect();
    Type::Tuple(types)
}

fn check_array(ctx: &mut TypeContext, elems: &mut [HirExpression], span: Span, expected: Option<&Type>) -> Type {
    if elems.is_empty() {
        return Type::Array(Box::new(Type::Unknown));
    }

    let item_expected = match expected {
        Some(Type::Array(inner)) => Some(&**inner),
        _ => None,
    };

    let first = check_expression(ctx, &mut elems[0], item_expected);
    for elem in &mut elems[1..] {
        let ty = check_expression(ctx, elem, item_expected);
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
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    let check_all_regex = |ctx: &mut TypeContext, args: &mut [HirCallArgument]| {
        for arg in args {
            match arg {
                HirCallArgument::Positional(ref mut e) => {
                    let ty = check_expression(ctx, e, None);
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
                match &mut args[0] {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e, None);
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
                match &mut args[0] {
                    HirCallArgument::Positional(e) => {
                        let ty = check_expression(ctx, e, None);
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
            ctx.error(
                format!("namespace 'Regex' has no constructor '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_regex_method(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
) -> Type {
    let mut arg_tys: Vec<Type> = Vec::with_capacity(args.len());
    for arg in args.iter_mut() {
        match arg {
            HirCallArgument::Positional(e) => arg_tys.push(check_expression(ctx, e, None)),
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
                for (idx, arg) in args.iter_mut().enumerate() {
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
            ctx.error(
                format!("Regex has no method '{}'", name),
                span,
            );
            Type::Error
        }
    }
}

fn check_struct_literal(ctx: &mut TypeContext, s: &mut HirStructLiteral) -> Type {
    let info = match ctx.structs.get(&s.name) {
        Some(info) => info.clone(),
        None => {
            ctx.error(format!("Unknown struct '{}'", s.name), s.span);
            return Type::Error;
        }
    };

    // Check each provided field
    let mut provided = std::collections::HashSet::new();
    for (field_name, field_expr) in s.fields.iter_mut() {
        if !provided.insert(field_name.as_str()) {
            ctx.error(
                format!(
                    "Duplicate field '{}' in struct literal '{}'",
                    field_name, s.name
                ),
                field_expr.span(),
            );
        }

        let expected_ty = info.fields.get(field_name.as_str()).map(|f| &f.ty);
        let actual_ty = check_expression(ctx, field_expr, expected_ty);
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
    let mut missing_fields: Vec<&String> = info
        .fields
        .iter()
        .filter(|(name, info)| !info.has_default && !provided.contains(name.as_str()))
        .map(|(name, _)| name)
        .collect();
    missing_fields.sort();

    for field_name in missing_fields {
        ctx.error(
            format!(
                "Missing required field '{}' in struct literal '{}'",
                field_name, s.name
            ),
            s.span,
        );
    }

    Type::Struct(s.name.clone())
}
