use crate::context::TypeContext;
use crate::types::Type;
use tempra_hir::*;

/// Explicit model for branch-local narrowing and bindings.
pub struct Refinement {
    pub bindings: Vec<(String, Type)>,
    pub narrowed_value: Option<(String, Type)>,
}

impl Refinement {
    pub fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            narrowed_value: None,
        }
    }

    pub fn apply(&self, ctx: &mut TypeContext) {
        for (name, ty) in &self.bindings {
            ctx.scopes.insert(name.clone(), ty.clone(), ctx.is_comptime);
        }
        if let Some((name, ty)) = &self.narrowed_value {
            ctx.scopes.insert(name.clone(), ty.clone(), ctx.is_comptime);
        }
    }
}

pub fn refinement_from_is_type(
    ctx: &mut TypeContext,
    value: &HirExpression,
    ty_node: &HirType,
) -> Refinement {
    let mut narrowed_value = None;
    if let HirExpression::Name(_, var_name) = value {
        let narrowed_ty = ctx.resolve_hir_type(ty_node);
        if narrowed_ty != Type::Unknown {
            // Check if variable is in scope to avoid narrowing non-variables
            if ctx.scopes.lookup(var_name).is_some() {
                narrowed_value = Some((var_name.clone(), narrowed_ty));
            }
        }
    }
    Refinement {
        bindings: Vec::new(),
        narrowed_value,
    }
}

pub fn refinement_from_is_pattern(
    ctx: &mut TypeContext,
    value: &HirExpression,
    pattern: &mut HirPattern,
    val_ty: Type,
) -> Refinement {
    // 0. Validate pattern is allowed in 'is'
    if !pattern_allowed_in_is(ctx, pattern) {
        return Refinement::empty();
    }

    let mut bindings = Vec::new();
    let mut narrowed_value = None;

    // 1. Extract pattern bindings (e.g. 'v' in 'Some(v)')
    extract_pattern_bindings(ctx, &val_ty, pattern, &mut bindings);

    // 2. Narrow the tested value itself if it's a Name
    if let HirExpression::Name(_, var_name) = value {
        if ctx.scopes.lookup(var_name).is_some() {
            // Re-bind with the current type (which is 'refined' by the pattern match)
            narrowed_value = Some((var_name.clone(), val_ty));
        }
    }

    Refinement {
        bindings,
        narrowed_value,
    }
}

pub fn validate_pattern_against_type(
    ctx: &mut TypeContext,
    val_ty: &Type,
    pattern: &mut HirPattern,
) -> Option<Type> {
    match pattern {
        HirPattern::Wildcard(_) | HirPattern::Identifier(_, _) => None,
        HirPattern::EnumCase(span, name, pat_args) => {
            let span_val = *span;
            let name_val = name.clone();

            // 1. Centralized Resolution: Check if the name refers to a Type (Built-in, Struct, or Enum)
            let resolved_ty = ctx.resolve_type_name(&name_val);
            if resolved_ty != Type::Unknown && pat_args.is_empty() {
                if !crate::types::types_compatible(&resolved_ty, val_ty) && *val_ty != Type::Error {
                    // No error for 'is' tests usually, but Tempra's pattern matching can be strict.
                }

                // If it resolved to an Enum, we still need to check if the user meant
                // a unit variant of THAT enum vs the enum type itself.
                if let Type::Enum(enum_name, _) = &resolved_ty {
                    if let Some(info) = ctx.enums.get(enum_name) {
                        if info.variants.contains_key(&name_val) {
                            // Ambiguity: Name is both the Enum type and a Variant name.
                            // Tempra convention: Variant takes priority if it's a unit variant.
                            // But 'if x is EnumName' is a common type test.
                        }
                    }
                }

                // If it's a Type Test, we're done here.
                // But we should probably rewrite it to a specific TypeTest pattern
                // or keep it as is if the evaluator understands it.
                // Actually, if enum_name == name_val, it's explicitly allowed.
                if let Type::Enum(enum_name, _) = val_ty {
                    if enum_name == &name_val {
                        return Some(resolved_ty);
                    }
                }
                // For built-ins and structs:
                if resolved_ty == *val_ty {
                    return Some(resolved_ty);
                }

                // If it resolves to a type but val_ty is different,
                // we still treat it as a Type Test (which might evaluate to false).
                // However, to keep it simple, only return Some if it's likely a type test.
                // Part 2.1 requires 'is Int' to work.
                if matches!(
                    resolved_ty,
                    Type::Int
                        | Type::String
                        | Type::Bool
                        | Type::Struct(_, _)
                        | Type::Enum(_, _)
                        | Type::Never
                ) {
                    return Some(resolved_ty);
                }

                // If it's an Enum type but we have arguments, or it didn't match the bare name test,
                // fall through to variant matching.
            }

            // 2. Resolve against user-defined Enum context
            if let Type::Enum(enum_name, args) = val_ty {
                // Extension: Allow 'val is EnumName' as a type test even if parsed as a pattern
                if pat_args.is_empty() && enum_name == name {
                    return Some(Type::Enum(enum_name.clone(), vec![]));
                }

                let info = ctx.enums.get(enum_name).cloned();
                if let Some(info) = info {
                    if let Some(payload_tys) = info.variants.get(name) {
                        let mut sub_map = std::collections::HashMap::new();
                        if info.generic_params.len() == args.len() {
                            for (param, type_arg) in info.generic_params.iter().zip(args.iter()) {
                                sub_map.insert(param.id, type_arg.clone());
                            }
                        }

                        let mut substituted_payloads = Vec::new();
                        for t in payload_tys {
                            substituted_payloads
                                .push(crate::check_expr::substitute_generics(ctx, t, &sub_map));
                        }
                        // Validate arity
                        if pat_args.len() != payload_tys.len() {
                            ctx.error(
                                format!(
                                    "variant '{}' of enum '{}' expects {} arguments, found {}",
                                    name_val,
                                    enum_name,
                                    payload_tys.len(),
                                    pat_args.len()
                                ),
                                span_val,
                            );
                            return None;
                        }
                        // Validate sub-patterns
                        for (i, arg_pat) in pat_args.iter_mut().enumerate() {
                            validate_pattern_against_type(ctx, &substituted_payloads[i], arg_pat);
                        }

                        // REWRITE: Transform EnumCase -> EnumConstructor
                        // Move args out of current variant
                        let mut moved_args = Vec::new();
                        if let HirPattern::EnumCase(_, _, ref mut original_args) = pattern {
                            moved_args = std::mem::take(original_args);
                        }

                        *pattern = HirPattern::EnumConstructor(HirEnumConstructorPattern {
                            span: span_val,
                            enum_name: enum_name.clone(),
                            variant_name: name_val,
                            payloads: moved_args,
                        });
                        return None;
                    }
                    let owners = ctx.variant_owners(&name_val);
                    let hint = if !owners.is_empty() {
                        format!(" (it belongs to: {})", owners.join(", "))
                    } else {
                        "".to_string()
                    };
                    ctx.error(
                        format!(
                            "constructor '{}' does not belong to enum '{}'{}",
                            name_val, enum_name, hint
                        ),
                        span_val,
                    );
                    return None;
                }
            }

            // 2. Generic Built-ins
            match name_val.as_str() {
                "Some" => {
                    if let Type::Option(inner) = val_ty {
                        if pat_args.len() == 1 {
                            validate_pattern_against_type(ctx, inner, &mut pat_args[0]);
                        } else if pat_args.len() > 1 {
                            ctx.error("some pattern expects at most 1 argument", span_val);
                        }
                    } else if *val_ty != Type::Error {
                        ctx.error(
                            format!(
                                "Cannot match {} with 'Some' pattern (expected Option)",
                                val_ty
                            ),
                            span_val,
                        );
                    }
                }
                "None" => {
                    if !matches!(val_ty, Type::Option(_)) && *val_ty != Type::Error {
                        ctx.error(
                            format!(
                                "Cannot match {} with 'None' pattern (expected Option)",
                                val_ty
                            ),
                            span_val,
                        );
                    }
                    if !pat_args.is_empty() {
                        ctx.error("none pattern expects 0 arguments", span_val);
                    }
                }
                "Ok" => {
                    if let Type::Result(ok_ty, _) = val_ty {
                        if pat_args.len() == 1 {
                            validate_pattern_against_type(ctx, ok_ty, &mut pat_args[0]);
                        } else if pat_args.len() > 1 {
                            ctx.error("ok pattern expects at most 1 argument", span_val);
                        }
                    } else if *val_ty != Type::Error {
                        ctx.error(
                            format!(
                                "Cannot match {} with 'Ok' pattern (expected Result)",
                                val_ty
                            ),
                            span_val,
                        );
                    }
                }
                "Err" => {
                    if let Type::Result(_, err_ty) = val_ty {
                        if pat_args.len() == 1 {
                            validate_pattern_against_type(ctx, err_ty, &mut pat_args[0]);
                        } else if pat_args.len() > 1 {
                            ctx.error("err pattern expects at most 1 argument", span_val);
                        }
                    } else if *val_ty != Type::Error {
                        ctx.error(
                            format!(
                                "Cannot match {} with 'Err' pattern (expected Result)",
                                val_ty
                            ),
                            span_val,
                        );
                    }
                }
                _ => {
                    if ctx.variant_to_enums.contains_key(name_val.as_str()) {
                        // The name is an enum constructor, but for the wrong type
                        let owners = ctx.variant_owners(&name_val);
                        ctx.error(
                            format!(
                                "constructor '{}' does not belong to type {} (it belongs to: {})",
                                name_val,
                                val_ty,
                                owners.join(", ")
                            ),
                            span_val,
                        );
                    } else {
                        ctx.error(
                            format!(
                                "unknown constructor '{}' in pattern for type {}",
                                name_val, val_ty
                            ),
                            span_val,
                        );
                    }
                }
            }
            None
        }
        HirPattern::Literal(span, lit_expr) => {
            // Pattern literals are immutable here as they are simple constants.
            let lit_ty = match lit_expr.as_ref() {
                HirExpression::Integer(_, _) => Type::Int,
                HirExpression::Float(_, _) => Type::Float64,
                HirExpression::String(_, _) => Type::String,
                HirExpression::Bool(_, _) => Type::Bool,
                _ => Type::Unknown,
            };
            if lit_ty != Type::Unknown && *val_ty != Type::Error && *val_ty != lit_ty {
                ctx.error(
                    format!(
                        "Literal pattern of type {} cannot match scrutinee of type {}",
                        lit_ty, val_ty
                    ),
                    *span,
                );
            }
            None
        }
        HirPattern::Struct(span, name, fields) => {
            if let Type::Struct(struct_name, args) = val_ty {
                if struct_name == name {
                    let def = ctx.structs.get(name).cloned().unwrap();
                    let mut sub_map = std::collections::HashMap::new();
                    for (param, arg) in def.generic_params.iter().zip(args.iter()) {
                        sub_map.insert(param.id, arg.clone());
                    }

                    let mut seen_fields = std::collections::HashSet::new();
                    for (field_name, field_pat) in fields.iter_mut() {
                        if !seen_fields.insert(field_name.clone()) {
                            ctx.error(
                                format!("duplicate field '{}' in struct pattern", field_name),
                                *span,
                            );
                        }
                        if let Some(field_def) = def.fields.get(field_name) {
                            if let Some(pat) = field_pat {
                                let expected = crate::check_expr::substitute_generics(
                                    ctx,
                                    &field_def.ty,
                                    &sub_map,
                                );
                                validate_pattern_against_type(ctx, &expected, pat);
                            }
                        } else {
                            ctx.error(
                                format!(
                                    "struct '{}' does not have a field named '{}'",
                                    name, field_name
                                ),
                                *span,
                            );
                        }
                    }
                } else {
                    ctx.error(
                        format!("cannot match {} with struct pattern for {}", val_ty, name),
                        *span,
                    );
                }
            } else {
                ctx.error(
                    format!("cannot match {} with struct pattern for {}", val_ty, name),
                    *span,
                );
            }
            None
        }
        HirPattern::Tuple(span, items) => {
            if let Type::Tuple(element_types) = val_ty {
                if element_types.len() != items.len() {
                    ctx.error(
                        format!(
                            "tuple pattern arity mismatch: expected {}, found {}",
                            element_types.len(),
                            items.len()
                        ),
                        *span,
                    );
                } else {
                    for (item_pat, item_ty) in items.iter_mut().zip(element_types.iter()) {
                        validate_pattern_against_type(ctx, item_ty, item_pat);
                    }
                }
            } else if *val_ty != Type::Error {
                ctx.error(
                    format!("cannot match non-tuple type {} with tuple pattern", val_ty),
                    *span,
                );
            }
            None
        }
        HirPattern::Or(_span, alternatives) => {
            if alternatives.is_empty() {
                return None;
            }

            // Rule 4: No Wildcards/Identifiers at top level
            for alt in alternatives.iter() {
                if matches!(alt, HirPattern::Wildcard(_) | HirPattern::Identifier(_, _)) {
                    ctx.error(
                        "wildcards and bindings are forbidden as direct alternatives in OR-patterns",
                        alt.span(),
                    );
                }
            }

            // Rule 3: No duplicate alternatives
            for i in 0..alternatives.len() {
                for j in i + 1..alternatives.len() {
                    if patterns_structurally_identical(&alternatives[i], &alternatives[j]) {
                        ctx.error("duplicate pattern in OR-pattern", alternatives[j].span());
                    }
                }
            }

            let mut all_bindings = Vec::new();
            for alt in alternatives.iter_mut() {
                validate_pattern_against_type(ctx, val_ty, alt);

                let mut bindings = Vec::new();
                extract_pattern_bindings(ctx, val_ty, alt, &mut bindings);
                all_bindings.push(bindings);
            }

            // Rule 2: Binding consistency
            if !all_bindings.is_empty() {
                let first = &all_bindings[0];
                for (i, other) in all_bindings.iter().enumerate().skip(1) {
                    if !compare_bindings(first, other) {
                        ctx.error(
                            "alternatives in OR-pattern must bind the same variables with the same types",
                            alternatives[i].span(),
                        );
                    }
                }
            }

            None
        }
        HirPattern::EnumConstructor(_) => None,
    }
}

pub fn extract_pattern_bindings(
    ctx: &mut TypeContext,
    val_ty: &Type,
    pattern: &mut HirPattern,
    out: &mut Vec<(String, Type)>,
) {
    match pattern {
        HirPattern::Identifier(_, name) => {
            out.push((name.clone(), val_ty.clone()));
        }
        HirPattern::EnumConstructor(e) => {
            if let Type::Enum(enum_name, args) = val_ty {
                if enum_name == &e.enum_name {
                    if let Some(info) = ctx.enums.get(enum_name) {
                        if let Some(payload_tys) = info.variants.get(&e.variant_name).cloned() {
                            let mut sub_map = std::collections::HashMap::new();
                            if info.generic_params.len() == args.len() {
                                for (param, arg) in info.generic_params.iter().zip(args.iter()) {
                                    sub_map.insert(param.id, arg.clone());
                                }
                            }
                            for (arg_pat, arg_ty) in e.payloads.iter_mut().zip(payload_tys.iter()) {
                                let substituted =
                                    crate::check_expr::substitute_generics(ctx, arg_ty, &sub_map);
                                extract_pattern_bindings(ctx, &substituted, arg_pat, out);
                            }
                        }
                    }
                }
            }
        }
        HirPattern::EnumCase(_, name, args) => {
            // Built-ins (Some, None, Ok, Err) - still using EnumCase for now
            match name.as_str() {
                "Some" => {
                    if let Type::Option(inner) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(ctx, inner, &mut args[0], out);
                        }
                    }
                }
                "Ok" => {
                    if let Type::Result(ok_ty, _) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(ctx, ok_ty, &mut args[0], out);
                        }
                    }
                }
                "Err" => {
                    if let Type::Result(_, err_ty) = val_ty {
                        if args.len() == 1 {
                            extract_pattern_bindings(ctx, err_ty, &mut args[0], out);
                        }
                    }
                }
                _ => {}
            }
        }
        HirPattern::Struct(_, name, fields) => {
            if let Type::Struct(struct_name, args) = val_ty {
                if struct_name == name {
                    let def = ctx.structs.get(name).cloned().unwrap();
                    let mut sub_map = std::collections::HashMap::new();
                    for (param, arg) in def.generic_params.iter().zip(args.iter()) {
                        sub_map.insert(param.id, arg.clone());
                    }
                    let mut field_types = Vec::new();
                    for (name, _) in fields.iter() {
                        field_types.push(
                            def.fields
                                .get(name)
                                .map(|f| f.ty.clone())
                                .unwrap_or(Type::Unknown),
                        );
                    }
                    for ((field_name, field_pat), field_ty) in fields.iter_mut().zip(field_types) {
                        if field_ty != Type::Unknown {
                            let substituted =
                                crate::check_expr::substitute_generics(ctx, &field_ty, &sub_map);
                            if let Some(pat) = field_pat {
                                extract_pattern_bindings(ctx, &substituted, pat, out);
                            } else {
                                // Shorthand binding
                                out.push((field_name.clone(), substituted));
                            }
                        }
                    }
                }
            }
        }
        HirPattern::Tuple(_, items) => {
            if let Type::Tuple(element_types) = val_ty {
                for (item_pat, item_ty) in items.iter_mut().zip(element_types.iter()) {
                    extract_pattern_bindings(ctx, item_ty, item_pat, out);
                }
            }
        }
        HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
        HirPattern::Or(_, alternatives) => {
            if let Some(first) = alternatives.get_mut(0) {
                extract_pattern_bindings(ctx, val_ty, first, out);
            }
        }
    }
}

pub fn compare_bindings(b1: &[(String, Type)], b2: &[(String, Type)]) -> bool {
    if b1.len() != b2.len() {
        return false;
    }
    let mut b1_sorted = b1.to_vec();
    let mut b2_sorted = b2.to_vec();
    b1_sorted.sort_by(|a, b| a.0.cmp(&b.0));
    b2_sorted.sort_by(|a, b| a.0.cmp(&b.0));

    for (i, (n1, t1)) in b1_sorted.iter().enumerate() {
        let (n2, t2) = &b2_sorted[i];
        if n1 != n2 || t1 != t2 {
            return false;
        }
    }
    true
}

pub fn patterns_structurally_identical(p1: &HirPattern, p2: &HirPattern) -> bool {
    match (p1, p2) {
        (HirPattern::Wildcard(_), HirPattern::Wildcard(_)) => true,
        (HirPattern::Identifier(_, n1), HirPattern::Identifier(_, n2)) => n1 == n2,
        (HirPattern::Literal(_, e1), HirPattern::Literal(_, e2)) => {
            // Minimal structural check for literals using their debug representation
            // Pattern literals in Tempra v1 are only Int, String, Bool.
            format!("{:?}", e1) == format!("{:?}", e2)
        }
        (HirPattern::EnumCase(_, n1, a1), HirPattern::EnumCase(_, n2, a2)) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(p1, p2)| patterns_structurally_identical(p1, p2))
        }
        (HirPattern::EnumConstructor(c1), HirPattern::EnumConstructor(c2)) => {
            c1.enum_name == c2.enum_name
                && c1.variant_name == c2.variant_name
                && c1.payloads.len() == c2.payloads.len()
                && c1
                    .payloads
                    .iter()
                    .zip(c2.payloads.iter())
                    .all(|(p1, p2)| patterns_structurally_identical(p1, p2))
        }
        (HirPattern::Struct(_, n1, f1), HirPattern::Struct(_, n2, f2)) => {
            if n1 != n2 || f1.len() != f2.len() {
                return false;
            }
            for (fn1, fp1) in f1 {
                if let Some((_, fp2)) = f2.iter().find(|(fn2, _)| fn1 == fn2) {
                    match (fp1, fp2) {
                        (Some(p1), Some(p2)) => {
                            if !patterns_structurally_identical(p1, p2) {
                                return false;
                            }
                        }
                        (None, None) => {}
                        _ => return false,
                    }
                } else {
                    return false;
                }
            }
            true
        }
        (HirPattern::Tuple(_, i1), HirPattern::Tuple(_, i2)) => {
            i1.len() == i2.len()
                && i1
                    .iter()
                    .zip(i2.iter())
                    .all(|(p1, p2)| patterns_structurally_identical(p1, p2))
        }
        (HirPattern::Or(_, a1), HirPattern::Or(_, a2)) => {
            a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(p1, p2)| patterns_structurally_identical(p1, p2))
        }
        _ => false,
    }
}

// ─── Exhaustiveness Groundwork v3 ─────────────────────────────────────────────
//
// A pattern space foundation for match exhaustiveness.
// This model distinguishes between structural space (for reachability)
// and coverage space (for exhaustiveness).
//
// Space::Constructor(String) always represents the top-level case only.
// Payload subpatterns are ignored at this stage.

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchCoverageKey {
    Bool(bool),
    Variant { enum_name: String, variant: String },
}

impl MatchCoverageKey {
    pub fn display(&self) -> String {
        match self {
            MatchCoverageKey::Bool(b) => b.to_string(),
            MatchCoverageKey::Variant { variant, .. } => variant.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Space {
    Constructor(String),
    Wildcard,
    Unknown,
}

#[derive(Debug, Clone)]
pub enum Domain {
    Bool,
    Enum(String),
    Tuple(usize),
    Struct(String),
    Unknown,
}

pub fn pattern_structural_space(pat: &HirPattern) -> Space {
    match pat {
        HirPattern::Wildcard(_) | HirPattern::Identifier(_, _) => Space::Wildcard,
        HirPattern::Literal(_, expr) => {
            if let HirExpression::Bool(_, b) = expr.as_ref() {
                Space::Constructor(b.to_string())
            } else {
                Space::Unknown
            }
        }
        HirPattern::EnumCase(_, name, args) => {
            // For reachability, only return Constructor if it's "universal" for that variant (no sub-patterns or all wildcards)
            let all_wildcards = args
                .iter()
                .all(|p| matches!(p, HirPattern::Wildcard(_) | HirPattern::Identifier(_, _)));
            if all_wildcards {
                Space::Constructor(name.clone())
            } else {
                Space::Unknown
            }
        }
        HirPattern::EnumConstructor(e) => {
            // For reachability, only return Constructor if it's "universal" for that variant
            let all_wildcards = e
                .payloads
                .iter()
                .all(|p| matches!(p, HirPattern::Wildcard(_) | HirPattern::Identifier(_, _)));
            if all_wildcards {
                Space::Constructor(e.variant_name.clone())
            } else {
                Space::Unknown
            }
        }
        _ => Space::Unknown,
    }
}

pub fn pattern_coverage_space(pat: &HirPattern, has_guard: bool) -> Space {
    if has_guard {
        return Space::Unknown;
    }

    // Based on Tempra v3 groundwork: top-level pattern counts as top-level coverage regardless of inner payload shape.
    match pat {
        HirPattern::EnumCase(_, name, _) => Space::Constructor(name.clone()),
        HirPattern::EnumConstructor(e) => Space::Constructor(e.variant_name.clone()),
        HirPattern::Literal(_, expr) => {
            if let HirExpression::Bool(_, b) = expr.as_ref() {
                Space::Constructor(b.to_string())
            } else {
                Space::Unknown
            }
        }
        HirPattern::Wildcard(_) | HirPattern::Identifier(_, _) => Space::Wildcard,
        _ => pattern_structural_space(pat),
    }
}

pub fn domain_of_type(ty: &Type) -> Domain {
    match ty {
        Type::Bool => Domain::Bool,
        Type::Option(_) => Domain::Enum("Option".to_string()),
        Type::Result(_, _) => Domain::Enum("Result".to_string()),
        Type::Enum(name, _) => Domain::Enum(name.clone()),
        Type::Tuple(ts) => Domain::Tuple(ts.len()),
        Type::Struct(name, _) => Domain::Struct(name.clone()),
        _ => Domain::Unknown,
    }
}

pub fn cases_of_domain(domain: &Domain, ctx: &TypeContext) -> Option<Vec<MatchCoverageKey>> {
    match domain {
        Domain::Bool => Some(vec![
            MatchCoverageKey::Bool(false),
            MatchCoverageKey::Bool(true),
        ]),
        Domain::Enum(name) => match name.as_str() {
            "Option" => Some(vec![
                MatchCoverageKey::Variant {
                    enum_name: "Option".to_string(),
                    variant: "Some".to_string(),
                },
                MatchCoverageKey::Variant {
                    enum_name: "Option".to_string(),
                    variant: "None".to_string(),
                },
            ]),
            "Result" => Some(vec![
                MatchCoverageKey::Variant {
                    enum_name: "Result".to_string(),
                    variant: "Ok".to_string(),
                },
                MatchCoverageKey::Variant {
                    enum_name: "Result".to_string(),
                    variant: "Err".to_string(),
                },
            ]),
            _ => ctx.enums.get(name).map(|info| {
                let mut variants: Vec<_> = info
                    .variants
                    .keys()
                    .map(|k| MatchCoverageKey::Variant {
                        enum_name: name.clone(),
                        variant: k.clone(),
                    })
                    .collect();
                variants.sort_by_key(|v| v.display());
                variants
            }),
        },
        Domain::Tuple(_) | Domain::Struct(_) | Domain::Unknown => None,
    }
}

pub fn pattern_allowed_in_is(ctx: &mut TypeContext, pattern: &HirPattern) -> bool {
    match pattern {
        HirPattern::Or(span, _) => {
            ctx.error("or-patterns are not allowed in is", *span);
            false
        }
        HirPattern::Wildcard(span) => {
            ctx.error("wildcard patterns are not allowed in is", *span);
            false
        }
        HirPattern::Identifier(span, _) => {
            ctx.error("naked binding patterns are not allowed after 'is'", *span);
            false
        }
        _ => true,
    }
}
