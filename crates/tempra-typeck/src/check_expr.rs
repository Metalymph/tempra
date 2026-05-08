//! Expression-level type checking logic.

use crate::context::TypeContext;
use crate::env::VariableInfo;
use crate::types::{types_compatible, Type};
use std::collections::HashMap;
use tempra_hir::*;
use tempra_span::Span;

impl TypeContext {
    pub fn eval_comptime_expr(
        &mut self,
        expr: &HirExpression,
    ) -> Result<tempra_interpreter::value::Value, String> {
        if let Some(interpreter) = &mut self.interpreter {
            let mut env = tempra_interpreter::env::RuntimeEnv::new();
            let old_mode = interpreter.mode;
            let old_fuel = interpreter.fuel;
            interpreter.mode = tempra_interpreter::interpreter::EvalMode::Comptime;
            interpreter.fuel = Some(1000000); // 1M steps
            let res = match interpreter.eval_expression(expr, &mut env) {
                Ok(tempra_interpreter::control_flow::ControlFlow::Value(v)) => Ok(v),
                Ok(cf) => Err(format!(
                    "comptime evaluation produced non-value control flow: {:?}",
                    cf
                )),
                Err(e) => Err(e),
            };

            interpreter.mode = old_mode;
            interpreter.fuel = old_fuel;
            res
        } else {
            Err("interpreter not available for comptime evaluation".into())
        }
    }
}

fn get_call_arg_mut<'a>(
    args: &'a mut [HirCallArgument],
    idx: usize,
    arg_name: &str,
) -> Option<&'a mut HirExpression> {
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
pub fn check_expression(
    ctx: &mut TypeContext,
    expr: &mut HirExpression,
    expected: Option<&Type>,
) -> Type {
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
        HirExpression::EnumConstructor(e) => {
            if let Some(Type::Enum(expected_name, args)) = expected {
                if expected_name == &e.enum_name {
                    return Type::Enum(e.enum_name.clone(), args.clone());
                }
            }
            Type::Enum(e.enum_name.clone(), vec![])
        }
        HirExpression::Closure(c) => check_closure(ctx, c, expected),
        HirExpression::Comptime(_) => check_comptime_expr(ctx, expr, expected),
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
        ctx.error(
            format!("invalid or unknown type in 'is' check: {}", ty),
            i.ty.span(),
        );
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
    if let Some(target_ty) =
        crate::pattern::validate_pattern_against_type(ctx, &val_ty, &mut is_pattern.pattern)
    {
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
                let is_variant = if let Type::Enum(enum_name, _) = &val_ty {
                    ctx.enums
                        .get(enum_name)
                        .map(|info| info.variants.contains_key(name))
                        .unwrap_or(false)
                } else {
                    false
                };

                let is_builtin = matches!(name.as_str(), "Some" | "None" | "Ok" | "Err");

                // Only report our error if it's not a variant of val_ty and not a built-in.
                // Note: crate::pattern::validate_pattern_against_type might have already errored if val_ty was an Enum.
                // But if val_ty is not an Enum (e.g. Int), crate::pattern::validate_pattern_against_type returns None silently for EnumCase.
                if !is_variant && !is_builtin && !matches!(val_ty, Type::Enum(_, _)) {
                    ctx.error(
                        format!("invalid or unknown type in 'is' check: {}", name),
                        *span,
                    );
                }
            }
        }
    }

    Type::Bool
}

fn check_as_optional(ctx: &mut TypeContext, a: &mut HirAsOptionalExpr) -> Type {
    let target_ty = ctx.resolve_hir_type(&a.ty);
    let _ = check_expression(ctx, &mut a.expr, Some(&target_ty));
    if target_ty == Type::Unknown {
        ctx.error("unknown type in 'as?' cast".to_string(), a.ty.span());
    }
    Type::Option(Box::new(target_ty))
}

fn check_as_forced(ctx: &mut TypeContext, a: &mut HirAsForcedExpr) -> Type {
    let target_ty = ctx.resolve_hir_type(&a.ty);
    let _ = check_expression(ctx, &mut a.expr, Some(&target_ty));
    if target_ty == Type::Unknown {
        ctx.error("unknown type in 'as!' cast".to_string(), a.ty.span());
    }
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
    LocalVariable(VariableInfo),
    Namespace(String),
    BuiltinConstant(Type),
    BuiltinFunction(String),
    EnumConstructor(String, String), // enum_name, variant_name
    Function(String),
    Type(Type),
    Unknown,
}

/// Helper: Resolves a name to its fundamental semantic category.
fn resolve_name(
    ctx: &TypeContext,
    name: &str,
    expected: Option<&Type>,
) -> Result<ResolvedName, String> {
    if let Some(info) = ctx.scopes.lookup(name) {
        if info.is_consumed {
            return Err(format!("use of consumed value '{}'", name));
        }
        return Ok(ResolvedName::LocalVariable(info.clone()));
    }

    // Check if it exists outside isolation boundary for closures
    if let Some(_) = ctx.scopes.lookup_anywhere(name) {
        return Err(format!(
            "use of external value '{}' requires explicit capture",
            name
        ));
    }

    if matches!(name, "fs" | "os" | "env" | "Regex") {
        return Ok(ResolvedName::Namespace(name.to_string()));
    }

    let ty = tempra_hir::types::resolve_named_type(name);
    if ty != Type::Unknown {
        return Ok(ResolvedName::Type(ty));
    }

    if let Some(_) = ctx.structs.get(name) {
        return Ok(ResolvedName::Type(Type::Struct(name.to_string(), vec![])));
    }
    if let Some(_) = ctx.enums.get(name) {
        return Ok(ResolvedName::Type(Type::Enum(name.to_string(), vec![])));
    }
    if let Some(_) = ctx.interfaces.get(name) {
        // Interface names can be used as type values (e.g. for constraints)
        return Ok(ResolvedName::Type(Type::Struct(name.to_string(), vec![])));
    }

    if let Some(param) = ctx
        .current_generic_params_info
        .iter()
        .find(|p| p.name == name)
    {
        return Ok(ResolvedName::Type(Type::GenericParam {
            id: param.id,
            name: name.to_string(),
            constraint: param.constraint.clone(),
        }));
    }

    let enum_res = ctx.find_enum_for_variant(name, expected);
    if let Ok(enum_name) = enum_res {
        return Ok(ResolvedName::EnumConstructor(enum_name, name.to_string()));
    }

    // If it's a built-in name, try that before reporting ambiguity/unknown for user enums
    // UNLESS it's truly ambiguous between user enums.
    let is_builtin = matches!(
        name,
        "print"
            | "fmt"
            | "dbg"
            | "crash"
            | "expect"
            | "exit"
            | "cmd"
            | "read_line"
            | "Some"
            | "None"
            | "Ok"
            | "Err"
            | "compile_error"
    );

    if is_builtin {
        let is_generic_builtin = matches!(name, "Some" | "None" | "Ok" | "Err");
        let has_user_candidates = ctx.variant_to_enums.contains_key(name);

        let expects_builtin = match expected {
            Some(Type::Option(_)) => matches!(name, "Some" | "None"),
            Some(Type::Result(_, _)) => matches!(name, "Ok" | "Err"),
            _ => false,
        };

        if is_generic_builtin && !expects_builtin {
            // Fall through to error reporting if there are user candidates,
            // or report missing expected type error if it's purely built-in but generic.
            if !has_user_candidates {
                return Err(format!("cannot infer generic enum type for built-in constructor '{}'; add a type annotation", name));
            }
        } else if !has_user_candidates || expects_builtin {
            if name == "None" {
                return Ok(ResolvedName::BuiltinConstant(Type::Option(Box::new(
                    Type::Unknown,
                ))));
            }
            return Ok(ResolvedName::BuiltinFunction(name.to_string()));
        }
    }

    if let Err(e) = enum_res {
        if ctx.variant_to_enums.contains_key(name) {
            return Err(e);
        }
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
        Ok(ResolvedName::LocalVariable(info)) => info.ty,
        Ok(ResolvedName::Namespace(ns)) => Type::Namespace(ns),
        Ok(ResolvedName::Type(ty)) => Type::Meta(Box::new(ty)),
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

            let result_ty = if let Some(Type::Enum(exp_name, exp_args)) = expected {
                if exp_name == &enum_name {
                    Type::Enum(enum_name.clone(), exp_args.clone())
                } else {
                    Type::Enum(enum_name.clone(), vec![])
                }
            } else {
                Type::Enum(enum_name.clone(), vec![])
            };

            // HIR Rewrite
            *expr = HirExpression::EnumConstructor(HirEnumConstructor {
                span,
                enum_name: enum_name.clone(),
                variant_name,
                payloads: vec![],
            });

            result_ty
        }
        Ok(ResolvedName::BuiltinFunction(f)) => {
            ctx.error(
                format!("built-in function '{}' cannot be used as a value", f),
                span,
            );
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
    Method(Type, String),            // receiver_type, method_name
    Namespace(String, String),       // namespace_name, method_name
    BuiltinFunction(String),
    Closure(Vec<Type>, Type),
    Unknown,
}

fn resolve_callee(
    ctx: &mut TypeContext,
    callee: &mut HirExpression,
    expected: Option<&Type>,
) -> ResolvedCallee {
    match callee {
        HirExpression::Name(_, name) => match resolve_name(ctx, name, expected) {
            Ok(ResolvedName::Function(f)) => ResolvedCallee::Function(f),
            Ok(ResolvedName::BuiltinFunction(f)) => ResolvedCallee::BuiltinFunction(f),
            Ok(ResolvedName::EnumConstructor(e, v)) => ResolvedCallee::EnumConstructor(e, v),
            Ok(ResolvedName::LocalVariable(info)) => {
                if let Type::Function(params, ret) = info.ty {
                    ResolvedCallee::Closure(params, *ret)
                } else {
                    ResolvedCallee::Unknown
                }
            }
            _ => ResolvedCallee::Unknown,
        },
        HirExpression::Member(m) => {
            let obj_ty = check_expression(ctx, &mut m.object, None);
            match &obj_ty {
                Type::Namespace(ns) => ResolvedCallee::Namespace(ns.clone(), m.field.clone()),
                Type::Struct(name, _) => {
                    if let Some(field_ty) = ctx.lookup_field(name, &m.field) {
                        if let Type::Function(params, ret) = field_ty {
                            return ResolvedCallee::Closure(params, *ret);
                        }
                    }
                    ResolvedCallee::Method(obj_ty.clone(), m.field.clone())
                }
                _ => ResolvedCallee::Method(obj_ty.clone(), m.field.clone()),
            }
        }
        _ => {
            let ty = check_expression(ctx, callee, None);
            if let Type::Function(params, ret) = ty {
                ResolvedCallee::Closure(params, *ret)
            } else {
                ResolvedCallee::Unknown
            }
        }
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
    if b.op == HirBinaryOp::Assign {
        return check_assignment(ctx, b);
    }

    let left = check_expression(ctx, &mut b.left, None);
    let right = check_expression(ctx, &mut b.right, None);

    // Propagate errors without cascading
    if left == Type::Error || right == Type::Error {
        return Type::Error;
    }

    match b.op {
        HirBinaryOp::Assign => unreachable!(),
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
            if let Type::Function(_, _) = left {
                ctx.error("closures are not comparable".to_string(), b.span);
            }
            if left != right {
                let both_int = (left == Type::Int || left == Type::Int64)
                    && (right == Type::Int || right == Type::Int64);
                let both_meta = matches!(left, Type::Meta(_)) && matches!(right, Type::Meta(_));
                if !both_int && !both_meta {
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
fn check_assignment(ctx: &mut TypeContext, b: &mut HirBinary) -> Type {
    let right = check_expression(ctx, &mut b.right, None);
    if right == Type::Error {
        return Type::Error;
    }

    enum TargetKind {
        Name(Span, String),
        Member,
        Index,
        Slice,
        Other,
    }
    let kind = match &*b.left {
        HirExpression::Name(span, name) => TargetKind::Name(*span, name.clone()),
        HirExpression::Member(_) => TargetKind::Member,
        HirExpression::Index(_) => TargetKind::Index,
        HirExpression::Slice(_) => TargetKind::Slice,
        _ => TargetKind::Other,
    };

    match kind {
        TargetKind::Name(span, name) => {
            let info = ctx.scopes.lookup(&name).cloned();
            if let Some(info) = info {
                if !info.is_mutable {
                    if info.is_capture {
                        ctx.error(format!("cannot assign to ref capture '{}'", name), span);
                    } else {
                        ctx.error(format!("cannot assign to immutable value '{}'", name), span);
                    }
                }
                if !types_compatible(&info.ty, &right) {
                    ctx.error(format!("Cannot assign {} to {}", right, info.ty), b.span);
                }
            } else {
                ctx.error(format!("undefined variable '{}'", name), span);
            }
        }
        TargetKind::Member => {
            let left_ty = check_expression(ctx, &mut b.left, None);
            if let HirExpression::Member(m) = &mut *b.left {
                // Phase 7: Check field mutability.
                let obj_ty = check_expression(ctx, &mut m.object, None);
                if let Type::Struct(name, _) = obj_ty {
                    if let Some(info) = ctx.structs.get(name.as_str()) {
                        if let Some(field_info) = info.fields.get(&m.field) {
                            if !field_info.is_mutable {
                                ctx.error(
                                    format!("cannot mutate immutable field '{}' of struct '{}'", m.field, name),
                                    b.span,
                                );
                            }
                        }
                    }
                }
                // NOTE: Tempra allows mutating 'var' fields even on 'let' instances.
            }
            if left_ty != Type::Error && !types_compatible(&left_ty, &right) {
                ctx.error(format!("Cannot assign {} to {}", right, left_ty), b.span);
            }
        }
        TargetKind::Index => {
            if let HirExpression::Index(idx) = &mut *b.left {
                let object_ty = check_expression(ctx, &mut idx.object, None);
                let index_ty = check_expression(ctx, &mut idx.index, None);
                check_receiver_mutable(ctx, &idx.object, b.span);

                if object_ty == Type::String {
                    ctx.error("string indexing is intentionally unsupported".to_string(), b.span);
                    return Type::Error;
                }

                if matches!(&*idx.object, HirExpression::Index(_) | HirExpression::Slice(_)) {
                    ctx.error("nested subscript assignment is not supported in v1".to_string(), b.span);
                }

                if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
                    if !index_ty.is_integer() && index_ty != Type::Error {
                        ctx.error(format!("index must be integer, found {}", index_ty), idx.index.span());
                    }
                    if !types_compatible(elem, &right) {
                        ctx.error(format!("Cannot assign {} to element of type {}", right, elem), b.span);
                    }
                    return Type::Unit;
                }

                if let Some(sig) = ctx.lookup_method_signature(&object_ty, "set") {
                    if sig.params.len() != 3 {
                        ctx.error("'set' hook must take exactly 2 arguments (index, value)".to_string(), b.span);
                        return Type::Error;
                    }
                    if !sig.params[0].is_inout {
                        ctx.error("'set' hook receiver must be 'inout self'".to_string(), b.span);
                    }
                    if sig.return_type != Type::Unit && sig.return_type != Type::Error {
                        ctx.error("'set' hook must return Unit".to_string(), b.span);
                    }
                    if !types_compatible(&sig.params[1].ty, &index_ty) {
                        ctx.error(format!("Index type mismatch: expected {}, found {}", sig.params[1].ty, index_ty), idx.index.span());
                    }
                    if !types_compatible(&sig.params[2].ty, &right) {
                        ctx.error(format!("Cannot assign {} to {}", right, sig.params[2].ty), b.right.span());
                    }
                    return Type::Unit;
                }

                if object_ty != Type::Error {
                    ctx.error(format!("Cannot assign to index of type {} (missing 'set' method)", object_ty), b.span);
                }
            }
        }
        TargetKind::Slice => {
            if let HirExpression::Slice(slc) = &mut *b.left {
                let object_ty = check_expression(ctx, &mut slc.object, None);
                let start_ty = check_expression(ctx, &mut slc.start, None);
                let end_ty = check_expression(ctx, &mut slc.end, None);
                check_receiver_mutable(ctx, &slc.object, b.span);

                if !start_ty.is_integer() && start_ty != Type::Error {
                    ctx.error(format!("slice start bound must be integer, found {}", start_ty), slc.start.span());
                }
                if !end_ty.is_integer() && end_ty != Type::Error {
                    ctx.error(format!("slice end bound must be integer, found {}", end_ty), slc.end.span());
                }

                if matches!(&*slc.object, HirExpression::Index(_) | HirExpression::Slice(_)) {
                    ctx.error("nested subscript assignment is not supported in v1".to_string(), b.span);
                }

                if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
                    let compatible = match &right {
                        Type::Array(rhs_elem) | Type::Slice(rhs_elem) => types_compatible(elem, rhs_elem),
                        _ => false,
                    };
                    if !compatible {
                        ctx.error(format!("Cannot assign {} to slice of type {}", right, object_ty), b.span);
                    }
                    return Type::Unit;
                }

                if let Some(sig) = ctx.lookup_method_signature(&object_ty, "set_slice") {
                    if sig.params.len() != 4 {
                        ctx.error("'set_slice' hook must take exactly 3 arguments (start, end, value)".to_string(), b.span);
                        return Type::Error;
                    }
                    if !sig.params[0].is_inout {
                        ctx.error("'set_slice' hook receiver must be 'inout self'".to_string(), b.span);
                    }
                    if sig.return_type != Type::Unit && sig.return_type != Type::Error {
                        ctx.error("'set_slice' hook must return Unit".to_string(), b.span);
                    }
                    if !types_compatible(&sig.params[1].ty, &start_ty) {
                        ctx.error(format!("Start bound type mismatch: expected {}, found {}", sig.params[1].ty, start_ty), slc.start.span());
                    }
                    if !types_compatible(&sig.params[2].ty, &end_ty) {
                        ctx.error(format!("End bound type mismatch: expected {}, found {}", sig.params[2].ty, end_ty), slc.end.span());
                    }
                    if !types_compatible(&sig.params[3].ty, &right) {
                        ctx.error(format!("Cannot assign {} to {}", right, sig.params[3].ty), b.right.span());
                    }
                    return Type::Unit;
                }

                if object_ty != Type::Error {
                    ctx.error(format!("Cannot assign to slice of type {} (missing 'set_slice' method)", object_ty), b.span);
                }
            }
        }
        TargetKind::Other => {
            ctx.error("invalid assignment target".to_string(), b.span);
        }
    }
    Type::Unit
}

fn check_builtin(
    ctx: &mut TypeContext,
    name: &str,
    args: &mut [HirCallArgument],
    span: Span,
    expected: Option<&Type>,
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
                let expected_inner = match expected {
                    Some(Type::Result(ok, _)) => Some(ok.as_ref()),
                    _ => None,
                };
                let ty = check_expression(ctx, arg_expr, expected_inner);
                let err_ty = match expected {
                    Some(Type::Result(_, err)) => err.as_ref().clone(),
                    _ => Type::Unknown,
                };
                Some(Type::Result(Box::new(ty), Box::new(err_ty)))
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
                let expected_err = match expected {
                    Some(Type::Result(_, err)) => Some(err.as_ref()),
                    _ => None,
                };
                let err_ty = check_expression(ctx, arg_expr, expected_err);
                let ok_ty = match expected {
                    Some(Type::Result(ok, _)) => ok.as_ref().clone(),
                    _ => Type::Unknown,
                };
                Some(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
            }
        }
        "Some" => {
            if args.len() != 1 {
                ctx.error(
                    format!("function 'Some' expects 1 argument, found {}", args.len()),
                    span,
                );
                Some(Type::Error)
            } else {
                let arg_expr = match &mut args[0] {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &mut n.value,
                };
                let expected_inner = match expected {
                    Some(Type::Option(inner)) => Some(inner.as_ref()),
                    _ => None,
                };
                let inner_ty = check_expression(ctx, arg_expr, expected_inner);
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
            ctx.error(format!("namespace 'fs' has no member '{}'", name), span);
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
            ctx.error(format!("namespace 'env' has no member '{}'", name), span);
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
            ctx.error(format!("namespace 'os' has no member '{}'", name), span);
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
                if arg_opt.is_none() {
                    arg_opt = get_call_arg_mut(args, 0, "substr");
                }
                if arg_opt.is_none() {
                    arg_opt = get_call_arg_mut(args, 0, "prefix");
                }
                if arg_opt.is_none() {
                    arg_opt = get_call_arg_mut(args, 0, "suffix");
                }

                if let Some(arg) = arg_opt {
                    let ty = check_expression(ctx, arg, None);
                    if ty != Type::String && ty != Type::Error {
                        ctx.error(
                            format!("'{}' expects String, found {}", name, ty),
                            arg.span(),
                        );
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
            ctx.error(format!("String has no method '{}'", name), span);
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
        "is_empty" => {
            if !args.is_empty() {
                ctx.error("'is_empty' method expects 0 arguments", span);
            }
            Type::Bool
        }
        _ => {
            ctx.error(format!("Array/Slice has no method '{}'", name), span);
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
            ctx.error(format!("Command has no method '{}'", name), span);
            Type::Error
        }
    }
}

fn check_call(ctx: &mut TypeContext, expr: &mut HirExpression, expected: Option<&Type>) -> Type {
    let call = if let HirExpression::Call(c) = expr {
        c
    } else {
        unreachable!()
    };

    if let HirExpression::Name(span, ref name) = *call.callee {
        if name == "compile_error" {
            for arg in &mut call.args {
                let ty = match arg {
                    HirCallArgument::Positional(e) => check_expression(ctx, e, Some(&Type::String)),
                    HirCallArgument::Named(n) => {
                        check_expression(ctx, &mut n.value, Some(&Type::String))
                    }
                };
                if ty != Type::String && ty != Type::Error {
                    let span = match arg {
                        HirCallArgument::Positional(e) => e.span(),
                        HirCallArgument::Named(n) => n.value.span(),
                    };
                    ctx.error(format!("compile_error expects String, found {}", ty), span);
                }
            }

            if let Some(arg) = call.args.first_mut() {
                let expr = match arg {
                    HirCallArgument::Positional(e) => e,
                    HirCallArgument::Named(n) => &mut n.value,
                };

                match ctx.eval_comptime_expr(expr) {
                    Ok(tempra_interpreter::value::Value::String(s)) => {
                        ctx.error(s, span);
                    }
                    _ => {
                        ctx.error("compile_error", span);
                    }
                }
            }

            return Type::Never;
        }
    }
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
                let type_args = c
                    .type_args
                    .as_ref()
                    .map(|args| args.iter().map(|a| ctx.resolve_hir_type(a)).collect());
                return check_function_call(ctx, c, &name, type_args, expected);
            }
        }
        ResolvedCallee::EnumConstructor(enum_name, variant_name) => {
            // Determine type arguments from expected type
            let mut type_args = vec![];
            if let Some(Type::Enum(name, args)) = expected {
                if name == &enum_name {
                    type_args = args.clone();
                }
            }

            let info = ctx
                .enums
                .get(&enum_name)
                .cloned()
                .expect("Enum info missing");

            // If enum is generic but no type arguments provided by expected, we can't fully resolve payloads yet.
            // In v1, we strictly require expected type or we error on generic mismatches.
            let mut sub_map = std::collections::HashMap::new();
            if info.generic_params.len() != type_args.len() {
                if !info.generic_params.is_empty() {
                    // For now, we don't implement full inference from payload to enum type args in this pass.
                    // But we should at least not crash.
                }
            } else {
                for (param, arg) in info.generic_params.iter().zip(type_args.iter()) {
                    sub_map.insert(param.id, arg.clone());
                }
            }

            let mut payload_tys = Vec::new();
            for t in &info.variants[&variant_name] {
                payload_tys.push(substitute_generics(ctx, t, &sub_map));
            }

            let mut error_occurred = false;

            if let HirExpression::Call(ref mut c) = expr {
                if c.args.len() != payload_tys.len() {
                    ctx.error(
                        format!(
                            "constructor '{}' of enum '{}' expects {} arguments, found {}",
                            variant_name,
                            enum_name,
                            payload_tys.len(),
                            c.args.len()
                        ),
                        c.span,
                    );
                    error_occurred = true;
                }

                // Arg Types
                for (i, arg) in c.args.iter_mut().enumerate() {
                    let arg_expr = match arg {
                        HirCallArgument::Positional(e) => e,
                        HirCallArgument::Named(n) => &mut n.value,
                    };
                    let target_ty = payload_tys.get(i);
                    let arg_ty = check_expression(ctx, arg_expr, target_ty);

                    if let Some(target_ty) = target_ty {
                        if arg_ty != Type::Error && !types_compatible(target_ty, &arg_ty) {
                            ctx.error(
                                format!(
                                    "Constructor '{}' argument {}: expected {}, found {}",
                                    variant_name,
                                    i + 1,
                                    target_ty,
                                    arg_ty
                                ),
                                arg_expr.span(),
                            );
                        }
                    }
                }
            }

            if error_occurred {
                return Type::Error;
            }

            let result_ty = Type::Enum(enum_name.clone(), type_args);

            // REWRITE
            if let HirExpression::Call(c) =
                std::mem::replace(expr, HirExpression::Integer(call_span, "0".to_string()))
            {
                let payloads = c
                    .args
                    .into_iter()
                    .filter_map(|a| {
                        if let HirCallArgument::Positional(e) = a {
                            Some(e)
                        } else {
                            None
                        }
                    })
                    .collect();
                *expr = HirExpression::EnumConstructor(HirEnumConstructor {
                    span: call_span,
                    enum_name: enum_name.clone(),
                    variant_name,
                    payloads,
                });
            }
            return result_ty;
        }
        ResolvedCallee::Closure(params, ret) => {
            if let HirExpression::Call(ref mut c) = expr {
                if c.args.len() != params.len() {
                    ctx.error(
                        format!(
                            "expected {} arguments, found {}",
                            params.len(),
                            c.args.len()
                        ),
                        c.span,
                    );
                }
                for (i, arg) in c.args.iter_mut().enumerate() {
                    let arg_expr = get_call_arg_mut_legacy(arg, ctx);
                    let target_ty = params.get(i);
                    let arg_ty = check_expression(ctx, arg_expr, target_ty);
                    if let Some(target_ty) = target_ty {
                        if arg_ty != Type::Error && !types_compatible(target_ty, &arg_ty) {
                            ctx.error(
                                format!(
                                    "argument type mismatch: expected {}, found {}",
                                    target_ty, arg_ty
                                ),
                                arg_expr.span(),
                            );
                        }
                    }
                }
                return ret;
            }
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
                if let Some(sig) = ctx.lookup_method_signature(&receiver_ty, &method) {
                    // Check parameter count (excluding self)
                    if c.args.len() != sig.params.len() - 1 {
                        ctx.error(
                            format!(
                                "method '{}' expects {} arguments, found {}",
                                method,
                                sig.params.len() - 1,
                                c.args.len()
                            ),
                            c.span,
                        );
                    }

                    // Check arguments
                    for (i, arg) in c.args.iter_mut().enumerate() {
                        let arg_expr = get_call_arg_mut_legacy(arg, ctx);
                        let target_ty = &sig.params[i + 1].ty; // Skip self
                        let arg_ty = check_expression(ctx, arg_expr, Some(target_ty));
                        if arg_ty != Type::Error && !types_compatible(target_ty, &arg_ty) {
                            ctx.error(
                                format!(
                                    "argument type mismatch for method '{}': expected {}, found {}",
                                    method, target_ty, arg_ty
                                ),
                                arg_expr.span(),
                            );
                        }
                    }

                    return sig.return_type;
                }

                // Fallback to builtins
                match &receiver_ty {
                    Type::String => return check_string_method(ctx, &method, &mut c.args, c.span),
                    Type::Array(_) | Type::Slice(_) => {
                        return check_array_method(ctx, &method, &mut c.args, c.span)
                    }
                    Type::Regex => return check_regex_method(ctx, &method, &mut c.args, c.span),
                    Type::Command => {
                        return check_command_method(ctx, &method, &mut c.args, c.span)
                    }
                    _ => {
                        ctx.error(
                            format!("type {} has no method '{}'", receiver_ty, method),
                            c.span,
                        );
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
        (Type::Meta(p), Type::Meta(a)) => match_generic_types(p, a, inferred),
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
        (Type::Projection { base: b1, assoc: a1 }, Type::Projection { base: b2, assoc: a2 }) => {
            if b1 == b2 && a1 == a2 { Ok(()) } else { Err(()) }
        }
        (Type::Projection { .. }, _) => Ok(()),
        (_, Type::Projection { .. }) => Ok(()),
        _ => {
            if param_ty == arg_ty {
                Ok(())
            } else {
                Err(())
            }
        }
    }
}

pub(crate) fn substitute_generics(
    ctx: &mut TypeContext,
    ty: &Type,
    map: &std::collections::HashMap<crate::types::GenericParamId, Type>,
) -> Type {
    match ty {
        Type::GenericParam { id, .. } => map.get(id).cloned().unwrap_or_else(|| ty.clone()),
        Type::Array(inner) => Type::Array(Box::new(substitute_generics(ctx, inner, map))),
        Type::Slice(inner) => Type::Slice(Box::new(substitute_generics(ctx, inner, map))),
        Type::Option(inner) => Type::Option(Box::new(substitute_generics(ctx, inner, map))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_generics(ctx, ok, map)),
            Box::new(substitute_generics(ctx, err, map)),
        ),
        Type::Tuple(elems) => {
            let mut resolved = Vec::new();
            for e in elems {
                resolved.push(substitute_generics(ctx, e, map));
            }
            Type::Tuple(resolved)
        }
        Type::Struct(name, args) => {
            let mut resolved_args = Vec::new();
            for a in args {
                resolved_args.push(substitute_generics(ctx, a, map));
            }
            Type::Struct(name.clone(), resolved_args)
        }
        Type::Enum(name, args) => {
            let mut resolved_args = Vec::new();
            for a in args {
                resolved_args.push(substitute_generics(ctx, a, map));
            }
            Type::Enum(name.clone(), resolved_args)
        }
        Type::Meta(inner) => Type::Meta(Box::new(substitute_generics(ctx, inner, map))),
        Type::Projection { base, assoc } => {
            if let Some(substituted_base) = map.get(base) {
                if !substituted_base.contains_generic_param() {
                    // Try to resolve concrete associated type
                    // We need to know which interface was constraining 'base'.
                    // For now, we search all interfaces satisfied by 'substituted_base'
                    // that have this associated type.
                    let itf_candidates: Vec<String> = ctx.interfaces.iter()
                        .filter(|(_, itf)| itf.associated_types.contains(assoc))
                        .map(|(name, _)| name.clone())
                        .collect();

                    for itf_name in itf_candidates {
                        if satisfies_interface_no_errors(ctx, substituted_base, &itf_name) {
                            if let Some(mappings) = ctx.infer_associated_types(substituted_base, &itf_name) {
                                if let Some(resolved) = mappings.get(assoc) {
                                    return resolved.clone();
                                }
                            }
                        }
                    }
                }
            }
            Type::Projection {
                base: *base,
                assoc: assoc.clone(),
            }
        }
        _ => ty.clone(),
    }
}

fn get_call_arg_mut_legacy<'a>(
    arg: &'a mut HirCallArgument,
    ctx: &mut TypeContext,
) -> &'a mut HirExpression {
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
    if let Some(builtin_ty) = check_builtin(ctx, func_name, &mut c.args, c.span, _expected) {
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
    let mut resolved_args: Vec<Option<&mut HirExpression>> =
        (0..sig.params.len()).map(|_| None).collect();
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

                let param_idx = sig.params.iter().position(|p| p.name == n.name);
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

    // Prepare inferred/explicit map early if possible to provide context to arguments
    let mut pre_inferred = std::collections::HashMap::new();
    if let Some(t_args) = explicit_type_args.as_ref() {
        if t_args.len() == sig.generic_params.len() {
            for (param, ty) in sig.generic_params.iter().zip(t_args.iter()) {
                pre_inferred.insert(param.id, ty.clone());
            }
        }
    }

    // Evaluate arguments to infer types
    let mut arg_types = Vec::new();
    for (i, resolved) in resolved_args.iter_mut().enumerate() {
        match resolved {
            Some(arg_expr) => {
                let expected_arg_ty = if !pre_inferred.is_empty() && i < sig.params.len() {
                    Some(substitute_generics(ctx, &sig.params[i].ty, &pre_inferred))
                } else {
                    None
                };
                let arg_ty = check_expression(ctx, *arg_expr, expected_arg_ty.as_ref());
                arg_types.push(arg_ty);
            }
            None => arg_types.push(Type::Error),
        }
    }

    let mut inferred = std::collections::HashMap::new();
    if let Some(t_args) = explicit_type_args {
        if t_args.len() != sig.generic_params.len() {
            ctx.error(
                format!(
                    "function '{}' expects {} type arguments, found {}",
                    func_name,
                    sig.generic_params.len(),
                    t_args.len()
                ),
                c.span,
            );
            return Type::Error;
        }
        for (param, ty) in sig.generic_params.iter().zip(t_args.into_iter()) {
            inferred.insert(param.id, ty);
        }
    } else {
        if !sig.generic_params.is_empty() {
            for (i, param) in sig.params.iter().enumerate() {
                if let Some(arg_ty) = arg_types.get(i) {
                    if *arg_ty != Type::Error {
                        if match_generic_types(&param.ty, arg_ty, &mut inferred).is_err() {
                            ctx.error(
                                format!(
                                    "type mismatch during generic inference for '{}'",
                                    func_name
                                ),
                                c.span,
                            );
                        }
                    }
                }
            }
        }
    }

    for param in &sig.generic_params {
        if let Some(ty) = inferred.get(&param.id) {
            if let Some(constraint) = &param.constraint {
                if !satisfies_interface(ctx, ty, constraint) {
                    ctx.error(
                        format!("type '{}' does not satisfy {}", ty, constraint),
                        c.span,
                    );
                }
            }
        } else {
            ctx.error(
                format!("cannot infer type for generic parameter '{}'", param.name),
                c.span,
            );
            return Type::Error;
        }
    }

    // Check types and missing arguments
    let mut comptime_args = Vec::new();
    for (i, resolved) in resolved_args.iter_mut().enumerate() {
        let param = &sig.params[i];
        let substituted_param_ty = substitute_generics(ctx, &param.ty, &inferred);
        let arg_ty = &arg_types[i];

        if let Some(arg_expr) = resolved {
            if *arg_ty != Type::Error
                && !crate::types::types_compatible(&substituted_param_ty, arg_ty)
            {
                // Structural satisfaction check for Meta(Interface) vs Meta(Struct)
                let mut satisfied = false;
                if let (Type::Meta(exp_inner), Type::Meta(act_inner)) =
                    (&substituted_param_ty, arg_ty)
                {
                    if let Type::Struct(interface_name, _) = &**exp_inner {
                        if satisfies_interface(ctx, act_inner, interface_name) {
                            satisfied = true;
                        }
                    }
                }

                if !satisfied {
                    ctx.error(
                        format!(
                            "argument for '{}' in '{}': expected {}, found {}",
                            param.name, func_name, substituted_param_ty, arg_ty
                        ),
                        arg_expr.span(),
                    );
                }
            }

            if param.kind == tempra_ast::ParameterKind::Comptime {
                match evaluate_comptime_arg(ctx, *arg_expr, &substituted_param_ty) {
                    Ok(val) => comptime_args.push(val),
                    Err((msg, span)) => {
                        ctx.error(msg, span);
                        comptime_args.push(crate::types::ComptimeValue::Unit);
                    }
                }
            }
        } else {
            ctx.error(
                format!(
                    "missing required argument '{}' for function '{}'",
                    param.name, func_name
                ),
                c.span,
            );
        }
    }

    // Bookkeep specialization identity
    let mut type_args_list = Vec::new();
    for param in &sig.generic_params {
        if let Some(ty) = inferred.get(&param.id) {
            type_args_list.push(ty.clone());
        }
    }

    let spec_key = crate::context::SpecializationKey {
        function_name: func_name.to_string(),
        type_args: type_args_list,
        comptime_args,
    };
    ctx.specialization_log.insert(spec_key);

    substitute_generics(ctx, &sig.return_type, &inferred)
}

fn evaluate_comptime_arg(
    ctx: &mut crate::context::TypeContext,
    expr: &mut HirExpression,
    _expected: &Type,
) -> Result<crate::types::ComptimeValue, (String, tempra_span::Span)> {
    // 1. Semantic check: must not use runtime variables
    semantic_check_comptime_expr(expr, ctx)?;

    // 2. Evaluate using restricted interpreter
    let val = ctx.eval_comptime_expr(expr).map_err(|e| (e, expr.span()))?;

    // 3. Convert Value to ComptimeValue
    match val {
        tempra_interpreter::value::Value::Unit => Ok(crate::types::ComptimeValue::Unit),
        tempra_interpreter::value::Value::Bool(b) => Ok(crate::types::ComptimeValue::Bool(b)),
        tempra_interpreter::value::Value::Int64(i) => Ok(crate::types::ComptimeValue::Int(i)),
        tempra_interpreter::value::Value::String(s) => Ok(crate::types::ComptimeValue::String(s)),
        _ => Err((
            format!("unsupported comptime value type: {}", val),
            expr.span(),
        )),
    }
}

fn check_member(ctx: &mut TypeContext, m: &mut HirMember, _expected: Option<&Type>) -> Type {
    let object_ty = check_expression(ctx, &mut m.object, None);
    if let Type::Struct(name, args) = &object_ty {
        if let Some(info) = ctx.structs.get(name.as_str()).cloned() {
            if let Some(field_info) = info.fields.get(&m.field) {
                let mut sub_map = std::collections::HashMap::new();
                if info.generic_params.len() == args.len() {
                    for (param, arg) in info.generic_params.iter().zip(args.iter()) {
                        sub_map.insert(param.id, arg.clone());
                    }
                }
                return substitute_generics(ctx, &field_info.ty, &sub_map);
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
                ctx.error(format!("CommandResult has no field '{}'", m.field), m.span);
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

    // Phase 7: String indexing is intentionally unsupported
    if object_ty == Type::String {
        ctx.error(
            "string indexing is intentionally unsupported".to_string(),
            i.span,
        );
        return Type::Error;
    }

    if let Type::Array(elem) | Type::Slice(elem) = &object_ty {
        if !index_ty.is_integer() && index_ty != Type::Error {
            ctx.error(format!("index must be integer, found {}", index_ty), i.span);
        }
        return *elem.clone();
    }

    // Phase 7: User-defined get hook
    if let Some(sig) = ctx.lookup_method_signature(&object_ty, "get") {
        // Verify index type matches the first non-self parameter
        if sig.params.len() >= 2 {
            let expected_idx_ty = &sig.params[1].ty;
            if !index_ty.is_integer() && index_ty != Type::Error {
                ctx.error(
                    format!(
                        "index must be integer, found {} (expected {})",
                        index_ty, expected_idx_ty
                    ),
                    i.span,
                );
            }
        }
        return sig.return_type.clone();
    }

    if object_ty != Type::Error {
        ctx.error(
            format!(
                "Cannot index type {} (missing 'get' method)",
                object_ty
            ),
            i.span,
        );
    }
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

    // Phase 7: User-defined slice hook
    if let Some(sig) = ctx.lookup_method_signature(&object_ty, "slice") {
        return sig.return_type.clone();
    }

    if object_ty != Type::Error {
        ctx.error(
            format!(
                "cannot slice type {} (missing 'slice' method)",
                object_ty
            ),
            s.span,
        );
    }
    Type::Error
}

/// Phase 7: Check that the receiver expression is mutable (for index/slice assignment).
fn check_receiver_mutable(ctx: &mut TypeContext, expr: &HirExpression, span: Span) {
    match expr {
        HirExpression::Name(_, name) => {
            if let Some(info) = ctx.scopes.lookup(name) {
                if !info.is_mutable {
                    ctx.error(
                        format!(
                            "cannot mutate immutable value '{}'",
                            name
                        ),
                        span,
                    );
                }
            }
        }
        HirExpression::Member(m) => {
            check_receiver_mutable(ctx, &m.object, span);
        }
        _ => {
            ctx.error(
                "invalid assignment target: receiver is not an assignable value".to_string(),
                span,
            );
        }
    }
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

fn depends_on_generic_param(expr: &HirExpression, ctx: &TypeContext) -> bool {
    match expr {
        HirExpression::Name(_, name) => {
            if ctx
                .current_generic_params_info
                .iter()
                .any(|p| &p.name == name)
            {
                return true;
            }
            if let Some(info) = ctx.scopes.lookup(name) {
                if info.is_parameter {
                    return true;
                }
            }
            false
        }
        HirExpression::Unary(u) => depends_on_generic_param(&u.expr, ctx),
        HirExpression::Binary(b) => {
            depends_on_generic_param(&b.left, ctx) || depends_on_generic_param(&b.right, ctx)
        }
        HirExpression::Call(c) => {
            depends_on_generic_param(&c.callee, ctx)
                || c.args.iter().any(|a| match a {
                    HirCallArgument::Positional(e) => depends_on_generic_param(e, ctx),
                    HirCallArgument::Named(n) => depends_on_generic_param(&n.value, ctx),
                })
                || c.type_args.as_ref().map_or(false, |targs| {
                    targs.iter().any(|t| match t {
                        HirType::Named(_, n) => {
                            ctx.current_generic_params_info.iter().any(|p| &p.name == n)
                        }
                        _ => false,
                    })
                })
        }
        HirExpression::Member(m) => depends_on_generic_param(&m.object, ctx),
        HirExpression::Index(i) => {
            depends_on_generic_param(&i.object, ctx) || depends_on_generic_param(&i.index, ctx)
        }
        HirExpression::Slice(s) => {
            depends_on_generic_param(&s.object, ctx)
                || depends_on_generic_param(&s.start, ctx)
                || depends_on_generic_param(&s.end, ctx)
        }
        HirExpression::Array(_, elements) | HirExpression::Tuple(_, elements) => {
            elements.iter().any(|e| depends_on_generic_param(e, ctx))
        }
        _ => false,
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
    if i.is_comptime {
        let cond_ty = check_expression(ctx, &mut i.condition, Some(&Type::Bool));
        if cond_ty != Type::Bool && cond_ty != Type::Error {
            ctx.error("comptime if condition must be Bool".to_string(), i.span);
        }

        if let Err((msg, span)) = semantic_check_comptime_expr(&mut i.condition, ctx) {
            ctx.error(msg, span);
            return Type::Error;
        }

        if depends_on_generic_param(&i.condition, ctx) {
            return expected.cloned().unwrap_or(Type::Unknown);
        }

        let val = match ctx.eval_comptime_expr(&mut i.condition) {
            Ok(v) => v,
            Err(e) => {
                ctx.error(
                    format!("if comptime condition must be known at compile time: {}", e),
                    i.condition.span(),
                );
                return Type::Error;
            }
        };

        if let tempra_interpreter::value::Value::Bool(b) = val {
            let result = if b {
                ctx.scopes.push_scope();
                let refinement = build_refinement_from_condition(ctx, &mut i.condition);
                refinement.apply(ctx);
                let then_ty = crate::check_stmt::check_block(ctx, &mut i.then_block, expected);
                ctx.scopes.pop_scope();

                // Prune the else branch if the then branch is selected
                i.else_branch = None;
                // Rewrite condition to literal true
                i.condition = Box::new(HirExpression::Bool(i.condition.span(), true));

                then_ty
            } else {
                let else_ty = if let Some(ref mut else_branch) = i.else_branch {
                    check_expression(ctx, else_branch, expected)
                } else {
                    Type::Unit
                };

                // Prune the then branch if the else branch is selected
                i.then_block.statements.clear();
                // Rewrite condition to literal false
                i.condition = Box::new(HirExpression::Bool(i.condition.span(), false));

                else_ty
            };

            return result;
        } else {
            if cond_ty != Type::Error {
                ctx.error("if comptime condition must be Bool".to_string(), i.span);
            }
            return Type::Error;
        }
    }

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
    if m.is_comptime {
        let val_ty = check_expression(ctx, &mut m.value, None);
        if val_ty == Type::Error {
            return Type::Error;
        }

        if let Err((msg, span)) = semantic_check_comptime_expr(&mut m.value, ctx) {
            ctx.error(msg, span);
            return Type::Error;
        }

        if depends_on_generic_param(&m.value, ctx) {
            return expected.cloned().unwrap_or(Type::Unknown);
        }

        let val = match ctx.eval_comptime_expr(&mut m.value) {
            Ok(v) => v,
            Err(e) => {
                ctx.error(
                    format!(
                        "comptime match scrutinee must be known at compile time: {}",
                        e
                    ),
                    m.value.span(),
                );
                return Type::Error;
            }
        };

        let mut selected_idx = None;
        for (i, arm) in m.arms.iter_mut().enumerate() {
            if arm.is_else {
                selected_idx = Some(i);
                break;
            }
            if arm.guard.is_some() {
                ctx.error(
                    "unsupported pattern in comptime match (guards not allowed)".to_string(),
                    arm.span,
                );
                continue;
            }

            let matches = match &mut arm.pattern {
                HirPattern::Literal(_, expr) => match ctx.eval_comptime_expr(expr) {
                    Ok(pat_val) => pat_val == val,
                    Err(e) => {
                        ctx.error(
                            format!(
                                "comptime match pattern must be known at compile time: {}",
                                e
                            ),
                            arm.pattern.span(),
                        );
                        false
                    }
                },
                HirPattern::Identifier(span, name) => {
                    let ty = ctx.resolve_type_name(name);
                    if ty != Type::Unknown {
                        if let tempra_interpreter::value::Value::Type(scrutinee_ty) = &val {
                            scrutinee_ty == &ty
                        } else {
                            false
                        }
                    } else {
                        ctx.error("unsupported pattern in comptime match".to_string(), *span);
                        false
                    }
                }
                HirPattern::EnumCase(span, name, args) => {
                    let ty = ctx.resolve_type_name(name);
                    if ty != Type::Unknown {
                        if !args.is_empty() {
                            ctx.error("unsupported pattern in comptime match (complex enum patterns not allowed)".to_string(), *span);
                            false
                        } else if let tempra_interpreter::value::Value::Type(scrutinee_ty) = &val {
                            scrutinee_ty == &ty
                        } else {
                            false
                        }
                    } else {
                        if !args.is_empty() {
                            ctx.error("unsupported pattern in comptime match (complex enum patterns not allowed)".to_string(), *span);
                        } else {
                            ctx.error("unsupported pattern in comptime match".to_string(), *span);
                        }
                        false
                    }
                }
                _ => {
                    ctx.error(
                        "unsupported pattern in comptime match".to_string(),
                        arm.pattern.span(),
                    );
                    false
                }
            };

            if matches {
                selected_idx = Some(i);
                break;
            }
        }

        if let Some(idx) = selected_idx {
            // Keep ONLY the selected arm
            let mut arm = m.arms.remove(idx);
            let arm_ty = check_expression(ctx, &mut arm.value, expected);
            m.arms = vec![arm];

            if let Some(expected_ty) = expected {
                if !types_compatible(expected_ty, &arm_ty) {
                    ctx.error(
                        format!(
                            "comptime match arm has incompatible type: expected {}, found {}",
                            expected_ty, arm_ty
                        ),
                        m.arms[0].value.span(),
                    );
                    return Type::Error;
                }
            }
            return arm_ty;
        }

        ctx.error("no matching arm in comptime match".to_string(), m.span);
        return Type::Error;
    }

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
                        crate::pattern::Domain::Enum(enum_name) => {
                            Some(crate::pattern::MatchCoverageKey::Variant {
                                enum_name: enum_name.clone(),
                                variant: name.clone(),
                            })
                        }
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
                ctx.scopes.insert(name, ty, ctx.is_comptime);
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
        Type::Struct(_, _) | Type::Enum(_, _) => {
            ctx.error(
                format!("structs and enums are not iterable, found {}", iter_ty),
                f.span,
            );
            Type::Error
        }
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
    ctx.scopes
        .insert(f.binding.clone(), elem_ty, ctx.is_comptime);

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
    let types: Vec<Type> = elems
        .iter_mut()
        .map(|e| check_expression(ctx, e, None))
        .collect();
    Type::Tuple(types)
}

fn check_array(
    ctx: &mut TypeContext,
    elems: &mut [HirExpression],
    span: Span,
    expected: Option<&Type>,
) -> Type {
    let item_expected = match expected {
        Some(Type::Array(inner)) => Some(&**inner),
        _ => None,
    };

    if elems.is_empty() {
        if let Some(inner) = item_expected {
            return Type::Array(Box::new(inner.clone()));
        } else {
            ctx.error("cannot infer element type for empty array literal", span);
            return Type::Array(Box::new(Type::Error));
        }
    }

    let mut common_ty = item_expected.cloned();

    for elem in elems.iter_mut() {
        let ty = check_expression(ctx, elem, item_expected);
        if ty == Type::Error {
            continue;
        }

        if let Some(ref current) = common_ty {
            if !types_compatible(current, &ty) {
                // If the current common type is Never, it can be promoted to any other type.
                if *current == Type::Never {
                    common_ty = Some(ty);
                } else if ty == Type::Never {
                    // Never is compatible with anything, so we keep the current type.
                } else {
                    ctx.error(
                        format!(
                            "array literal elements must have the same type: expected {}, found {}",
                            current, ty
                        ),
                        span,
                    );
                }
            }
        } else {
            common_ty = Some(ty);
        }
    }

    let result_item_ty = common_ty.unwrap_or(Type::Error);

    if result_item_ty == Type::Never && item_expected.is_none() {
        ctx.error(
            "cannot infer element type for array literal with only Never elements",
            span,
        );
        return Type::Array(Box::new(Type::Error));
    }

    if result_item_ty == Type::Error {
        return Type::Array(Box::new(Type::Error));
    }

    Type::Array(Box::new(result_item_ty))
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
            ctx.error(format!("Regex has no method '{}'", name), span);
            Type::Error
        }
    }
}

fn check_struct_literal(ctx: &mut TypeContext, s: &mut HirStructLiteral) -> Type {
    let ty = ctx.resolve_hir_type(&s.ty);
    let (name, args) = match &ty {
        Type::Struct(name, args) => (name.clone(), args.clone()),
        Type::Error => return Type::Error,
        _ => {
            ctx.error(format!("expected struct type, found {}", ty), s.span);
            return Type::Error;
        }
    };

    let info = match ctx.structs.get(&name) {
        Some(info) => info.clone(),
        None => {
            ctx.error(format!("Unknown struct '{}'", name), s.span);
            return Type::Error;
        }
    };

    // Prepare substitution map if generic
    let mut sub_map = std::collections::HashMap::new();
    if info.generic_params.len() != args.len() {
        ctx.error(
            format!(
                "generic struct '{}' requires {} type argument{}",
                name,
                info.generic_params.len(),
                if info.generic_params.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            s.span,
        );
    } else {
        for (param, arg) in info.generic_params.iter().zip(args.iter()) {
            sub_map.insert(param.id, arg.clone());
        }
    }

    // Check each provided field
    let mut provided = std::collections::HashSet::new();
    for (field_name, field_expr) in s.fields.iter_mut() {
        if !provided.insert(field_name.as_str()) {
            ctx.error(
                format!(
                    "Duplicate field '{}' in struct literal '{}'",
                    field_name, name
                ),
                field_expr.span(),
            );
        }

        let field_info = info.fields.get(field_name.as_str());
        let mut expected_ty = None;
        if let Some(f) = field_info {
            expected_ty = Some(substitute_generics(ctx, &f.ty, &sub_map));
        }
        let actual_ty = check_expression(ctx, field_expr, expected_ty.as_ref());

        match field_info {
            Some(field_info) => {
                let expected = substitute_generics(ctx, &field_info.ty, &sub_map);
                if actual_ty != Type::Error && !types_compatible(&expected, &actual_ty) {
                    ctx.error(
                        format!(
                            "Field '{}' of '{}': expected {}, found {}",
                            field_name, name, expected, actual_ty
                        ),
                        field_expr.span(),
                    );
                }
            }
            None => {
                ctx.error(
                    format!("Struct '{}' has no field '{}'", name, field_name),
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
                field_name, name
            ),
            s.span,
        );
    }

    ty
}

fn check_closure(ctx: &mut TypeContext, c: &mut HirClosure, expected: Option<&Type>) -> Type {
    let mut expected_ret = None;
    let mut expected_params = Vec::new();

    if let Some(Type::Function(params, ret)) = expected {
        expected_ret = Some((**ret).clone());
        expected_params = params.clone();
    }

    // Resolve parameters
    let mut param_types = Vec::new();
    for (i, p) in c.parameters.iter().enumerate() {
        let ty = ctx.resolve_hir_type(&p.ty);
        if let Some(exp_p) = expected_params.get(i) {
            if !types_compatible(exp_p, &ty) {
                ctx.error(
                    format!("parameter type mismatch: expected {}, found {}", exp_p, ty),
                    p.span,
                );
            }
        }
        param_types.push(ty);
    }

    let return_type = if let Some(ret_ty_expr) = &c.return_type {
        ctx.resolve_hir_type(ret_ty_expr)
    } else {
        expected_ret.unwrap_or(Type::Unit)
    };

    let mut seen_captures = HashMap::new();
    let mut capture_info = Vec::new();
    let mut has_recursive_self = false;

    for cap in &c.captures {
        match cap {
            HirCapture::Named(named) => {
                if let Some(_prev_span) = seen_captures.insert(named.name.clone(), named.span) {
                    ctx.error(format!("duplicate capture: '{}'", named.name), named.span);
                }

                if let Some(info) = ctx.scopes.lookup_anywhere(&named.name).cloned() {
                    if info.is_consumed {
                        ctx.error(
                            format!("use of consumed value '{}'", named.name),
                            named.span,
                        );
                    }

                    if named.mode == HirCaptureMode::Consume {
                        ctx.scopes.mark_consumed(&named.name);
                    }

                    let is_mutable = match named.mode {
                        HirCaptureMode::Ref => false,
                        HirCaptureMode::Inout => {
                            if !info.is_mutable {
                                ctx.error(
                                    format!(
                                        "cannot capture immutable value '{}' as inout",
                                        named.name
                                    ),
                                    named.span,
                                );
                            }
                            true
                        }
                        HirCaptureMode::Consume => false,
                    };
                    capture_info.push((named.name.clone(), info.ty.clone(), is_mutable));
                } else {
                    ctx.error(
                        format!("cannot capture unknown value '{}'", named.name),
                        named.span,
                    );
                }
            }
            HirCapture::RecursiveSelf(span) => {
                if let Some(_prev_span) = seen_captures.insert("self".into(), span.clone()) {
                    ctx.error("duplicate capture: 'self'", span.clone());
                }
                has_recursive_self = true;
            }
        }
    }

    let closure_type = Type::Function(param_types.clone(), Box::new(return_type.clone()));

    // Push isolated scope
    ctx.scopes.push_isolated_scope();

    // Inject recursive self if present
    if has_recursive_self {
        ctx.scopes.insert_info(
            "self".into(),
            closure_type.clone(),
            false,
            true,
            false,
            true,
        );
    }

    // Insert other captures into isolated scope
    for (name, ty, is_mutable) in capture_info {
        ctx.scopes
            .insert_info(name, ty, is_mutable, true, false, false);
    }

    // Insert parameters into scope
    for (p, ty) in c.parameters.iter().zip(param_types.iter()) {
        if let Some(info) = ctx.scopes.lookup(&p.name) {
            if info.is_capture {
                ctx.error(format!("cannot shadow captured value '{}'", p.name), p.span);
            }
        }
        ctx.scopes
            .insert_info(p.name.clone(), ty.clone(), false, false, false, true);
    }

    // Check body
    let prev_ret = ctx.current_return_type.take();
    let prev_fallible = ctx.current_is_fallible;

    ctx.current_return_type = Some(return_type.clone());
    ctx.current_is_fallible = false;

    let body_ty = crate::check_stmt::check_block(ctx, &mut c.body, Some(&return_type));

    if return_type != Type::Unit && !types_compatible(&return_type, &body_ty) {
        ctx.error(
            format!(
                "closure return type mismatch: expected {}, found {}",
                return_type, body_ty
            ),
            c.body.span,
        );
    }

    ctx.current_return_type = prev_ret;
    ctx.current_is_fallible = prev_fallible;

    ctx.scopes.pop_isolated_scope();

    Type::Function(param_types, Box::new(return_type))
}

fn check_comptime_expr(
    ctx: &mut TypeContext,
    expr: &mut HirExpression,
    expected: Option<&Type>,
) -> Type {
    let mut comptime_node = if let HirExpression::Comptime(c) = expr {
        c.clone()
    } else {
        unreachable!()
    };

    let old_is_comptime = ctx.is_comptime;
    ctx.is_comptime = true;
    let ty = check_expression(ctx, &mut comptime_node.expression, expected);
    ctx.is_comptime = old_is_comptime;

    if ty == Type::Error {
        return Type::Error;
    }

    // Semantic check: ensure no runtime variables are used.
    if let Err((msg, span)) = semantic_check_comptime_expr(&comptime_node.expression, ctx) {
        ctx.error(msg, span);
        return Type::Error;
    }

    if let Some(interpreter) = &mut ctx.interpreter {
        let mut env = tempra_interpreter::env::RuntimeEnv::new();
        let old_mode = interpreter.mode;
        let old_fuel = interpreter.fuel;
        interpreter.mode = tempra_interpreter::interpreter::EvalMode::Comptime;
        interpreter.fuel = Some(1000000); // 1M steps

        match interpreter.eval_expression(&comptime_node.expression, &mut env) {
            Ok(tempra_interpreter::control_flow::ControlFlow::Value(v)) => {
                interpreter.mode = old_mode;
                interpreter.fuel = old_fuel;
                if let Some(lit) = materialize_primitive(&v, comptime_node.span) {
                    *expr = lit;
                    return ty;
                } else {
                    ctx.error(
                        format!(
                            "value of type {} cannot be materialized from comptime in v1",
                            v.type_name()
                        ),
                        comptime_node.span,
                    );
                    return Type::Error;
                }
            }
            Ok(cf) => {
                interpreter.mode = old_mode;
                interpreter.fuel = old_fuel;
                ctx.error(
                    format!(
                        "comptime evaluation resulted in unexpected control flow: {:?}",
                        cf
                    ),
                    comptime_node.span,
                );
                return Type::Error;
            }
            Err(e) => {
                interpreter.mode = old_mode;
                interpreter.fuel = old_fuel;
                if depends_on_generic_param(&comptime_node.expression, ctx) {
                    return ty;
                }
                ctx.error(
                    format!("comptime evaluation error: {}", e),
                    comptime_node.span,
                );
                return Type::Error;
            }
        }
    } else {
        ctx.error(
            "interpreter not available for comptime evaluation",
            comptime_node.span,
        );
        return Type::Error;
    }
}

fn materialize_primitive(
    val: &tempra_interpreter::value::Value,
    span: Span,
) -> Option<HirExpression> {
    use tempra_interpreter::value::Value;
    match val {
        Value::Int64(i) => Some(HirExpression::Integer(span, i.to_string())),
        Value::Float64(f) => Some(HirExpression::Float(span, f.to_string())),
        Value::Bool(b) => Some(HirExpression::Bool(span, *b)),
        Value::String(s) => Some(HirExpression::String(span, s.clone())),
        Value::Unit => Some(HirExpression::Tuple(span, vec![])),
        _ => None,
    }
}

fn semantic_check_comptime_expr(
    expr: &HirExpression,
    ctx: &TypeContext,
) -> Result<(), (String, Span)> {
    match expr {
        HirExpression::Name(span, name) => {
            if let Some(info) = ctx.scopes.lookup(name) {
                if !info.is_comptime {
                    return Err((
                        format!(
                            "runtime variable '{}' cannot be used in comptime context",
                            name
                        ),
                        *span,
                    ));
                }
            }
            Ok(())
        }
        HirExpression::Integer(_, _)
        | HirExpression::Float(_, _)
        | HirExpression::String(_, _)
        | HirExpression::Bool(_, _) => Ok(()),
        HirExpression::Unary(u) => semantic_check_comptime_expr(&u.expr, ctx),
        HirExpression::Binary(b) => {
            semantic_check_comptime_expr(&b.left, ctx)?;
            semantic_check_comptime_expr(&b.right, ctx)
        }
        HirExpression::Call(c) => {
            if let HirExpression::Member(member) = &*c.callee {
                if let HirExpression::Name(_, namespace) = &*member.object {
                    if matches!(namespace.as_str(), "fs" | "os" | "env") {
                        return Err((
                            format!(
                                "operation '{}.{}' is not allowed during comptime evaluation",
                                namespace, member.field
                            ),
                            member.span,
                        ));
                    }
                }
            }
            if let HirExpression::Name(span, name) = &*c.callee {
                if matches!(name.as_str(), "print" | "read_line" | "cmd" | "exit") {
                    return Err((
                        format!(
                            "operation '{}' is not allowed during comptime evaluation",
                            name
                        ),
                        *span,
                    ));
                }
            }
            semantic_check_comptime_expr(&c.callee, ctx)?;
            for arg in &c.args {
                match arg {
                    HirCallArgument::Positional(e) => semantic_check_comptime_expr(e, ctx)?,
                    HirCallArgument::Named(n) => semantic_check_comptime_expr(&n.value, ctx)?,
                }
            }
            Ok(())
        }
        HirExpression::Member(m) => semantic_check_comptime_expr(&m.object, ctx),
        HirExpression::Index(i) => {
            semantic_check_comptime_expr(&i.object, ctx)?;
            semantic_check_comptime_expr(&i.index, ctx)
        }
        HirExpression::Slice(s) => {
            semantic_check_comptime_expr(&s.object, ctx)?;
            semantic_check_comptime_expr(&s.start, ctx)?;
            semantic_check_comptime_expr(&s.end, ctx)
        }
        HirExpression::Block(b) => semantic_check_block(b, ctx),
        HirExpression::If(i) => {
            semantic_check_comptime_expr(&i.condition, ctx)?;
            semantic_check_block(&i.then_block, ctx)?;
            if let Some(eb) = &i.else_branch {
                semantic_check_comptime_expr(eb, ctx)?;
            }
            Ok(())
        }
        HirExpression::Match(m) => {
            semantic_check_comptime_expr(&m.value, ctx)?;
            for arm in &m.arms {
                if let Some(g) = &arm.guard {
                    semantic_check_comptime_expr(g, ctx)?;
                }
                semantic_check_comptime_expr(&arm.value, ctx)?;
            }
            Ok(())
        }
        HirExpression::Tuple(_, elems) => {
            for e in elems {
                semantic_check_comptime_expr(e, ctx)?;
            }
            Ok(())
        }
        HirExpression::Array(_, elems) => {
            for e in elems {
                semantic_check_comptime_expr(e, ctx)?;
            }
            Ok(())
        }
        HirExpression::StructLiteral(s) => {
            for (_, e) in &s.fields {
                semantic_check_comptime_expr(e, ctx)?;
            }
            Ok(())
        }
        HirExpression::Comptime(c) => semantic_check_comptime_expr(&c.expression, ctx),
        _ => Ok(()),
    }
}

fn semantic_check_block(block: &HirBlock, ctx: &TypeContext) -> Result<(), (String, Span)> {
    for stmt in &block.statements {
        match stmt {
            HirStatement::Let(l) => semantic_check_comptime_expr(&l.value, ctx)?,
            HirStatement::Var(v) => semantic_check_comptime_expr(&v.value, ctx)?,
            HirStatement::Expr(e) => semantic_check_comptime_expr(e, ctx)?,
            HirStatement::Return(opt_e) => {
                if let Some(e) = opt_e {
                    semantic_check_comptime_expr(e, ctx)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn satisfies_interface(ctx: &mut TypeContext, ty: &Type, interface_name: &str) -> bool {
    let interface = match ctx.interfaces.get(interface_name) {
        Some(i) => i.clone(),
        None => {
            ctx.error(
                format!("undefined interface '{}'", interface_name),
                tempra_span::Span::new(0, 0),
            );
            return false;
        }
    };

    use crate::types::substitute_self;

    let mut mappings = HashMap::new();
    let mut satisfied = true;
    for (method_name, req) in &interface.methods {
        let provided_sig = ctx.lookup_direct_method_signature(ty, method_name);
        match provided_sig {
            Some(sig) => {
                // Substitute Self in requirement with the concrete type
                let req_params: Vec<Type> =
                    req.params.iter().map(|p| substitute_self(p, ty)).collect();
                let req_ret = substitute_self(&req.return_type, ty);

                let mut method_match = true;
                if sig.params.len() != req_params.len() {
                    method_match = false;
                } else {
                    for (p_prov, p_req) in sig.params.iter().zip(req_params.iter()) {
                        if let Err(e) = unify_associated(p_req, &p_prov.ty, interface_name, &mut mappings) {
                            ctx.error(e, tempra_span::Span::new(0, 0));
                            method_match = false;
                            break;
                        }
                    }
                }

                if method_match {
                    if let Err(e) = unify_associated(&req_ret, &sig.return_type, interface_name, &mut mappings) {
                        ctx.error(e, tempra_span::Span::new(0, 0));
                        method_match = false;
                    }
                }

                if !method_match {
                    ctx.error(
                        format!("mismatched requirement for method '{}'", method_name),
                        tempra_span::Span::new(0, 0),
                    );
                    satisfied = false;
                }
            }
            None => {
                ctx.error(
                    format!("missing requirement '{}'", method_name),
                    tempra_span::Span::new(0, 0),
                );
                satisfied = false;
            }
        }
    }

    if satisfied {
        // Ensure all associated types were inferred
        for at_name in &interface.associated_types {
            if !mappings.contains_key(at_name) {
                ctx.error(
                    format!("could not infer associated type '{}'", at_name),
                    tempra_span::Span::new(0, 0),
                );
                satisfied = false;
            }
        }
    }

    satisfied
}

pub fn satisfies_interface_no_errors(ctx: &TypeContext, ty: &Type, interface_name: &str) -> bool {
    let interface = match ctx.interfaces.get(interface_name) {
        Some(i) => i.clone(),
        None => return false,
    };

    use crate::types::substitute_self;
    let mut mappings = HashMap::new();
    let mut satisfied = true;
    for (method_name, req) in &interface.methods {
        let provided_sig = ctx.lookup_direct_method_signature(ty, method_name);
        match provided_sig {
            Some(sig) => {
                let req_params: Vec<Type> =
                    req.params.iter().map(|p| substitute_self(p, ty)).collect();
                let req_ret = substitute_self(&req.return_type, ty);

                let mut method_match = true;
                if sig.params.len() != req_params.len() {
                    method_match = false;
                } else {
                    for (p_prov, p_req) in sig.params.iter().zip(req_params.iter()) {
                        if unify_associated(p_req, &p_prov.ty, interface_name, &mut mappings).is_err() {
                            method_match = false;
                            break;
                        }
                    }
                }
                if method_match && unify_associated(&req_ret, &sig.return_type, interface_name, &mut mappings).is_err() {
                    method_match = false;
                }
                if !method_match {
                    satisfied = false;
                    break;
                }
            }
            None => {
                satisfied = false;
                break;
            }
        }
    }
    if satisfied {
        for at_name in &interface.associated_types {
            if !mappings.contains_key(at_name) {
                satisfied = false;
                break;
            }
        }
    }
    satisfied
}

fn unify_associated(
    req: &Type,
    actual: &Type,
    interface_name: &str,
    mappings: &mut HashMap<String, Type>,
) -> Result<(), String> {
    match req {
        Type::Associated(itf, name) if itf == interface_name => {
            if let Some(existing) = mappings.get(name) {
                if existing == actual {
                    return Ok(());
                } else {
                    return Err(format!("conflicting associated type inference for {}", name));
                }
            }
            mappings.insert(name.clone(), actual.clone());
            Ok(())
        }
        Type::Array(inner_req) => {
            if let Type::Array(inner_act) = actual {
                unify_associated(inner_req, inner_act, interface_name, mappings)
            } else {
                Err("type mismatch".into())
            }
        }
        Type::Slice(inner_req) => {
            if let Type::Slice(inner_act) = actual {
                unify_associated(inner_req, inner_act, interface_name, mappings)
            } else {
                Err("type mismatch".into())
            }
        }
        Type::Option(inner_req) => {
            if let Type::Option(inner_act) = actual {
                unify_associated(inner_req, inner_act, interface_name, mappings)
            } else {
                Err("type mismatch".into())
            }
        }
        Type::Result(ok_req, err_req) => {
            if let Type::Result(ok_act, err_act) = actual {
                unify_associated(ok_req, ok_act, interface_name, mappings)?;
                unify_associated(err_req, err_act, interface_name, mappings)
            } else {
                Err("type mismatch".into())
            }
        }
        _ => {
            if crate::types::types_compatible(req, actual) {
                Ok(())
            } else {
                Err("type mismatch".into())
            }
        }
    }
}
