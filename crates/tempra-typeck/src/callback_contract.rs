//! Statically inferred non-escaping callback contracts (Memory & Ownership v0.4 §16.5).
//!
//! A callback parameter is certified only if every source-level use is either
//! an immediate invocation or a forward to another already certified parameter.
//! No untrusted extern, dynamic callee, storage, closure capture, or return is
//! accepted as proof. The fixed-point computation is monotone and deterministic.

use std::collections::{BTreeMap, BTreeSet};
use tempra_hir::{
    HirBlock, HirCall, HirCallArgument, HirExpression, HirFile, HirFunction, HirItem, HirPattern,
    HirStatement, HirType, ParameterKind,
};

use crate::borrow_summary::resolve_direct;

/// Indices of function-typed parameters proved to remain inside a call frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallbackSummary {
    /// Declared parameter positions, including any comptime/type parameters.
    pub non_escaping_parameters: BTreeSet<usize>,
}

fn is_callback(ty: &HirType) -> bool {
    matches!(ty, HirType::Function(_, _, _))
}

fn pattern_shadows(pattern: &HirPattern, name: &str) -> bool {
    match pattern {
        HirPattern::Identifier(_, found) => found == name,
        HirPattern::Tuple(_, values)
        | HirPattern::Or(_, values)
        | HirPattern::EnumCase(_, _, values) => values.iter().any(|p| pattern_shadows(p, name)),
        HirPattern::EnumConstructor(variant) => {
            variant.payloads.iter().any(|p| pattern_shadows(p, name))
        }
        HirPattern::Struct(_, _, fields) => fields
            .iter()
            .any(|(_, p)| p.as_ref().is_some_and(|p| pattern_shadows(p, name))),
        HirPattern::Literal(_, expr) => {
            matches!(&**expr, HirExpression::Name(_, found) if found == name)
        }
        HirPattern::Wildcard(_) => false,
    }
}

struct Proof<'a> {
    name: &'a str,
    module: &'a str,
    definitions: &'a BTreeMap<String, &'a HirFunction>,
    known: &'a BTreeMap<String, CallbackSummary>,
    /// Any local/parameter shadowing the name of a potential direct callee.
    local_names: BTreeSet<String>,
}

impl Proof<'_> {
    fn call(&self, call: &HirCall) -> bool {
        if matches!(&*call.callee, HirExpression::Name(_, name) if name == self.name) {
            // The callback is invoked now, never retained.
            return call.args.iter().all(|arg| {
                self.expr(match arg {
                    HirCallArgument::Positional(value) => value,
                    HirCallArgument::Named(named) => &named.value,
                })
            });
        }
        let direct = match &*call.callee {
            HirExpression::Name(_, name) if !self.local_names.contains(name) => {
                resolve_direct(self.definitions, self.module, name)
            }
            _ => None,
        };
        let mut positional = 0usize;
        for arg in &call.args {
            let (value, param_index) = match arg {
                HirCallArgument::Positional(value) => {
                    let Some((_, callee)) = direct else {
                        if !self.expr(value) {
                            return false;
                        }
                        continue;
                    };
                    while callee
                        .parameters
                        .get(positional)
                        .is_some_and(|p| p.kind == ParameterKind::Type)
                    {
                        positional += 1;
                    }
                    let index = positional;
                    positional += 1;
                    (value, Some(index))
                }
                HirCallArgument::Named(named) => {
                    let index = direct.and_then(|(_, callee)| {
                        callee.parameters.iter().position(|p| p.name == named.name)
                    });
                    (&named.value, index)
                }
            };
            if matches!(value, HirExpression::Name(_, name) if name == self.name) {
                let Some((key, callee)) = direct else {
                    return false;
                };
                let Some(index) = param_index else {
                    return false;
                };
                if !callee
                    .parameters
                    .get(index)
                    .is_some_and(|p| is_callback(&p.ty))
                    || !self
                        .known
                        .get(key)
                        .is_some_and(|summary| summary.non_escaping_parameters.contains(&index))
                {
                    return false;
                }
            } else if !self.expr(value) {
                return false;
            }
        }
        self.expr(&call.callee)
    }

    fn block(&self, block: &HirBlock) -> bool {
        block.statements.iter().all(|stmt| self.stmt(stmt))
    }

    fn stmt(&self, stmt: &HirStatement) -> bool {
        match stmt {
            HirStatement::Let(l) => l.name != self.name && self.expr(&l.value),
            HirStatement::Var(v) => v.name != self.name && self.expr(&v.value),
            HirStatement::Return(Some(expr)) | HirStatement::Expr(expr) => self.expr(expr),
            HirStatement::Return(None) | HirStatement::Break(_) | HirStatement::Continue(_) => true,
            HirStatement::For(loop_) => {
                loop_.binding != self.name
                    && self.expr(&loop_.iterable)
                    && loop_.filter.as_ref().is_none_or(|e| self.expr(e))
                    && self.block(&loop_.body)
            }
        }
    }

    fn expr(&self, expr: &HirExpression) -> bool {
        match expr {
            HirExpression::Name(_, name) => name != self.name,
            HirExpression::Call(call) => self.call(call),
            HirExpression::Unary(e) => self.expr(&e.expr),
            HirExpression::Binary(e) => self.expr(&e.left) && self.expr(&e.right),
            HirExpression::IsType(e) => self.expr(&e.value),
            HirExpression::IsPattern(e) => self.expr(&e.value),
            HirExpression::AsOptional(e) => self.expr(&e.expr),
            HirExpression::AsForced(e) => self.expr(&e.expr),
            HirExpression::Try(e) => self.expr(&e.expr),
            HirExpression::Unwrap(e) => self.expr(&e.expr),
            HirExpression::Member(e) => self.expr(&e.object),
            HirExpression::Index(e) => self.expr(&e.object) && self.expr(&e.index),
            HirExpression::Slice(e) => {
                self.expr(&e.object) && self.expr(&e.start) && self.expr(&e.end)
            }
            HirExpression::Block(block) => self.block(block),
            HirExpression::If(e) => {
                self.expr(&e.condition)
                    && self.block(&e.then_block)
                    && e.else_branch.as_ref().is_none_or(|b| self.expr(b))
            }
            HirExpression::Match(e) => {
                self.expr(&e.value)
                    && e.arms.iter().all(|arm| {
                        !pattern_shadows(&arm.pattern, self.name)
                            && arm.guard.as_ref().is_none_or(|g| self.expr(g))
                            && self.expr(&arm.value)
                    })
            }
            HirExpression::For(e) => {
                e.binding != self.name
                    && self.expr(&e.iterable)
                    && e.filter.as_ref().is_none_or(|f| self.expr(f))
                    && self.block(&e.body)
            }
            HirExpression::Tuple(_, values) | HirExpression::Array(_, values) => {
                values.iter().all(|e| self.expr(e))
            }
            HirExpression::StructLiteral(e) => e.fields.iter().all(|(_, e)| self.expr(e)),
            HirExpression::EnumConstructor(e) => e.payloads.iter().all(|e| self.expr(e)),
            HirExpression::Closure(e) => {
                // A nested closure could outlive the call. Only prove the outer
                // parameter when neither the capture set nor nested body uses it.
                !e.parameters.iter().any(|p| p.name == self.name)
                    && !e.captures.iter().any(|capture| match capture {
                        tempra_hir::HirCapture::Named(named) => named.name == self.name,
                        tempra_hir::HirCapture::RecursiveSelf(_) => false,
                    })
                    && self.block(&e.body)
            }
            HirExpression::Comptime(e) => self.expr(&e.expression),
            HirExpression::Integer(_, _)
            | HirExpression::Float(_, _)
            | HirExpression::String(_, _)
            | HirExpression::Bool(_, _) => true,
        }
    }
}

fn collect_pattern_names(pattern: &HirPattern, names: &mut BTreeSet<String>) {
    match pattern {
        HirPattern::Identifier(_, name) => {
            names.insert(name.clone());
        }
        HirPattern::Tuple(_, patterns)
        | HirPattern::Or(_, patterns)
        | HirPattern::EnumCase(_, _, patterns) => {
            for p in patterns {
                collect_pattern_names(p, names);
            }
        }
        HirPattern::EnumConstructor(e) => {
            for p in &e.payloads {
                collect_pattern_names(p, names);
            }
        }
        HirPattern::Struct(_, _, fields) => {
            for (_, p) in fields {
                if let Some(p) = p {
                    collect_pattern_names(p, names);
                }
            }
        }
        HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
    }
}

fn collect_expr_names(expr: &HirExpression, names: &mut BTreeSet<String>) {
    match expr {
        HirExpression::Call(e) => {
            collect_expr_names(&e.callee, names);
            for arg in &e.args {
                collect_expr_names(
                    match arg {
                        HirCallArgument::Positional(value) => value,
                        HirCallArgument::Named(named) => &named.value,
                    },
                    names,
                );
            }
        }
        HirExpression::Block(b) => collect_locals(b, names),
        HirExpression::If(e) => {
            collect_expr_names(&e.condition, names);
            collect_locals(&e.then_block, names);
            if let Some(other) = &e.else_branch {
                collect_expr_names(other, names);
            }
        }
        HirExpression::Match(e) => {
            collect_expr_names(&e.value, names);
            for arm in &e.arms {
                collect_pattern_names(&arm.pattern, names);
                if let Some(guard) = &arm.guard {
                    collect_expr_names(guard, names);
                }
                collect_expr_names(&arm.value, names);
            }
        }
        HirExpression::For(e) => {
            names.insert(e.binding.clone());
            collect_expr_names(&e.iterable, names);
            if let Some(filter) = &e.filter {
                collect_expr_names(filter, names);
            }
            collect_locals(&e.body, names);
        }
        HirExpression::Closure(e) => {
            names.extend(e.parameters.iter().map(|p| p.name.clone()));
            collect_locals(&e.body, names);
        }
        HirExpression::Unary(e) => collect_expr_names(&e.expr, names),
        HirExpression::Binary(e) => {
            collect_expr_names(&e.left, names);
            collect_expr_names(&e.right, names);
        }
        HirExpression::IsType(e) => collect_expr_names(&e.value, names),
        HirExpression::IsPattern(e) => collect_expr_names(&e.value, names),
        HirExpression::AsOptional(e) => collect_expr_names(&e.expr, names),
        HirExpression::AsForced(e) => collect_expr_names(&e.expr, names),
        HirExpression::Try(e) => collect_expr_names(&e.expr, names),
        HirExpression::Unwrap(e) => collect_expr_names(&e.expr, names),
        HirExpression::Member(e) => collect_expr_names(&e.object, names),
        HirExpression::Index(e) => {
            collect_expr_names(&e.object, names);
            collect_expr_names(&e.index, names);
        }
        HirExpression::Slice(e) => {
            collect_expr_names(&e.object, names);
            collect_expr_names(&e.start, names);
            collect_expr_names(&e.end, names);
        }
        HirExpression::Tuple(_, values) | HirExpression::Array(_, values) => {
            for value in values {
                collect_expr_names(value, names);
            }
        }
        HirExpression::StructLiteral(e) => {
            for (_, value) in &e.fields {
                collect_expr_names(value, names);
            }
        }
        HirExpression::EnumConstructor(e) => {
            for value in &e.payloads {
                collect_expr_names(value, names);
            }
        }
        HirExpression::Comptime(e) => collect_expr_names(&e.expression, names),
        HirExpression::Name(_, _)
        | HirExpression::Integer(_, _)
        | HirExpression::Float(_, _)
        | HirExpression::String(_, _)
        | HirExpression::Bool(_, _) => {}
    }
}

fn collect_locals(block: &HirBlock, names: &mut BTreeSet<String>) {
    for stmt in &block.statements {
        match stmt {
            HirStatement::Let(l) => {
                names.insert(l.name.clone());
                collect_expr_names(&l.value, names);
            }
            HirStatement::Var(v) => {
                names.insert(v.name.clone());
                collect_expr_names(&v.value, names);
            }
            HirStatement::For(loop_) => {
                names.insert(loop_.binding.clone());
                collect_expr_names(&loop_.iterable, names);
                if let Some(filter) = &loop_.filter {
                    collect_expr_names(filter, names);
                }
                collect_locals(&loop_.body, names);
            }
            HirStatement::Return(Some(e)) | HirStatement::Expr(e) => collect_expr_names(e, names),
            HirStatement::Return(None) | HirStatement::Break(_) | HirStatement::Continue(_) => {}
        }
    }
}

/// Infer contracts on function-typed parameters only; never certify extern
/// or unresolved targets. Forwarding contracts require a proven callee contract.
/// The resulting map is stable under source and hash-map iteration order.
#[must_use]
pub fn infer(files: &[(&str, &HirFile)]) -> BTreeMap<String, CallbackSummary> {
    let mut definitions = BTreeMap::<String, &HirFunction>::new();
    for &(module, file) in files {
        for item in &file.items {
            if let HirItem::Function(function) = item {
                let key = if module.is_empty() {
                    function.name.clone()
                } else {
                    format!("{module}.{}", function.name)
                };
                definitions.insert(key, function);
            }
        }
    }
    let mut known = BTreeMap::<String, CallbackSummary>::new();
    for _ in 0..=definitions.len() {
        let mut changed = false;
        for (key, function) in &definitions {
            let Some(body) = &function.body else { continue };
            if function.is_extern {
                continue;
            }
            let module = key.rsplit_once('.').map(|(m, _)| m).unwrap_or("");
            let mut local_names = function
                .parameters
                .iter()
                .map(|p| p.name.clone())
                .collect::<BTreeSet<_>>();
            collect_locals(body, &mut local_names);
            let mut new = known.get(key).cloned().unwrap_or_default();
            for (index, param) in function.parameters.iter().enumerate() {
                if !is_callback(&param.ty) || new.non_escaping_parameters.contains(&index) {
                    continue;
                }
                let proof = Proof {
                    name: &param.name,
                    module,
                    definitions: &definitions,
                    known: &known,
                    local_names: local_names.clone(),
                };
                if proof.block(body) {
                    new.non_escaping_parameters.insert(index);
                    changed = true;
                }
            }
            if !new.non_escaping_parameters.is_empty() {
                known.insert(key.clone(), new);
            }
        }
        if !changed {
            break;
        }
    }
    known
}
