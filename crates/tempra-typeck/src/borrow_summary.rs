//! Conservative, inferred borrowed-return contracts (Memory & Ownership v0.4 §14.8–14.9).
//!
//! Only identifiable, single-source read-only Slice returns are summarized.
//! Structured branch exits and immutable aliases are considered; unsupported
//! control flow and unresolved cycles remain without a contract. Summaries
//! describe provenance, not execution support or lifetime annotations.

use crate::facts::TypecheckFacts;
use std::collections::{BTreeMap, HashMap};
use tempra_hir::{
    HirBinaryOp, HirBlock, HirCallArgument, HirExpression, HirFile, HirFunction, HirItem,
    HirPattern, HirStatement, HirType, ParameterKind,
};

/// Access mode required at a function boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowAccess {
    /// The argument is observed without exclusive access.
    Read,
    /// The argument is borrowed exclusively during the call.
    Exclusive,
    /// The argument is not a borrowed slice parameter.
    Value,
}

/// A borrowed return whose lifetime is derived from a single parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnBorrow {
    /// Zero-based position in the complete function parameter list.
    pub parameter_index: usize,
    /// The returned access is currently always read-only.
    pub mode: BorrowAccess,
}

/// Statically certified function borrow information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowSummary {
    /// Parameter accesses, ordered exactly as declared.
    pub parameter_accesses: Vec<BorrowAccess>,
    /// The unique parameter supplying a read-only returned slice.
    pub return_borrow: ReturnBorrow,
}

/// Determine whether a HIR type is a slice without resolving unrelated types.
pub(crate) fn is_slice(ty: &HirType) -> bool {
    matches!(ty, HirType::Apply(_, base, _) if matches!(&**base, HirType::Named(_, name) if name == "Slice"))
}

fn qualify(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else {
        format!("{module}.{name}")
    }
}

/// Resolve a direct function name only within an unambiguous namespace.
/// No dynamic-call target guessing is permitted.
pub(crate) fn resolve_direct<'a, T>(
    table: &'a BTreeMap<String, T>,
    module: &str,
    name: &str,
) -> Option<(&'a str, &'a T)> {
    let qualified = qualify(module, name);
    if let Some((key, value)) = table.get_key_value(&qualified) {
        return Some((key.as_str(), value));
    }
    table
        .get_key_value(name)
        .map(|(key, value)| (key.as_str(), value))
}

// Detect returns in expressions outside the supported structured statement
// traversal. Never ignore an exit just because its value is not a tail.
fn has_explicit_return(block: &HirBlock) -> bool {
    block.statements.iter().any(|stmt| match stmt {
        HirStatement::Return(_) => true,
        HirStatement::For(loop_) => {
            expr_has_return(&loop_.iterable)
                || loop_.filter.as_ref().is_some_and(expr_has_return)
                || has_explicit_return(&loop_.body)
        }
        HirStatement::Let(l) => expr_has_return(&l.value),
        HirStatement::Var(v) => expr_has_return(&v.value),
        HirStatement::Expr(expr) => expr_has_return(expr),
        HirStatement::Break(_) | HirStatement::Continue(_) => false,
    })
}

fn expr_has_return(expr: &HirExpression) -> bool {
    match expr {
        HirExpression::Block(block) => has_explicit_return(block),
        HirExpression::If(branch) => {
            expr_has_return(&branch.condition)
                || has_explicit_return(&branch.then_block)
                || branch
                    .else_branch
                    .as_ref()
                    .is_some_and(|other| expr_has_return(other))
        }
        HirExpression::Match(m) => {
            expr_has_return(&m.value)
                || m.arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_has_return) || expr_has_return(&arm.value)
                })
        }
        HirExpression::For(loop_) => {
            expr_has_return(&loop_.iterable)
                || loop_
                    .filter
                    .as_ref()
                    .is_some_and(|filter| expr_has_return(filter))
                || has_explicit_return(&loop_.body)
        }
        HirExpression::Call(call) => {
            expr_has_return(&call.callee)
                || call.args.iter().any(|arg| match arg {
                    HirCallArgument::Positional(value) => expr_has_return(value),
                    HirCallArgument::Named(named) => expr_has_return(&named.value),
                })
        }
        HirExpression::Slice(slice) => {
            expr_has_return(&slice.object)
                || expr_has_return(&slice.start)
                || expr_has_return(&slice.end)
        }
        HirExpression::Index(index) => {
            expr_has_return(&index.object) || expr_has_return(&index.index)
        }
        HirExpression::Unary(unary) => expr_has_return(&unary.expr),
        HirExpression::Binary(binary) => {
            expr_has_return(&binary.left) || expr_has_return(&binary.right)
        }
        HirExpression::Member(member) => expr_has_return(&member.object),
        HirExpression::IsType(e) => expr_has_return(&e.value),
        HirExpression::IsPattern(e) => expr_has_return(&e.value),
        HirExpression::AsOptional(e) => expr_has_return(&e.expr),
        HirExpression::AsForced(e) => expr_has_return(&e.expr),
        HirExpression::Try(e) => expr_has_return(&e.expr),
        HirExpression::Unwrap(e) => expr_has_return(&e.expr),
        HirExpression::Tuple(_, values) | HirExpression::Array(_, values) => {
            values.iter().any(expr_has_return)
        }
        HirExpression::StructLiteral(s) => s.fields.iter().any(|(_, value)| expr_has_return(value)),
        HirExpression::EnumConstructor(e) => e.payloads.iter().any(expr_has_return),
        HirExpression::Comptime(e) => expr_has_return(&e.expression),
        // A nested closure has its own return context.
        HirExpression::Closure(_)
        | HirExpression::Name(_, _)
        | HirExpression::Integer(_, _)
        | HirExpression::Float(_, _)
        | HirExpression::String(_, _)
        | HirExpression::Bool(_, _) => false,
    }
}

// A present entry with no origin shadows an outer declaration. Only immutable
// aliases and Slice parameters may carry a proven source.
type Origins = HashMap<String, Option<usize>>;

fn shadow_pattern(pattern: &HirPattern, locals: &mut Origins) {
    match pattern {
        HirPattern::Identifier(_, name) => {
            locals.insert(name.clone(), None);
        }
        HirPattern::Tuple(_, patterns)
        | HirPattern::Or(_, patterns)
        | HirPattern::EnumCase(_, _, patterns) => {
            for pattern in patterns {
                shadow_pattern(pattern, locals);
            }
        }
        HirPattern::EnumConstructor(value) => {
            for pattern in &value.payloads {
                shadow_pattern(pattern, locals);
            }
        }
        HirPattern::Struct(_, _, fields) => {
            for (name, pattern) in fields {
                if let Some(pattern) = pattern {
                    shadow_pattern(pattern, locals);
                } else {
                    locals.insert(name.clone(), None);
                }
            }
        }
        HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
    }
}

fn shadow_condition(condition: &HirExpression, locals: &mut Origins) {
    match condition {
        HirExpression::IsPattern(value) => shadow_pattern(&value.pattern, locals),
        HirExpression::Binary(value) if value.op == HirBinaryOp::And => {
            shadow_condition(&value.left, locals);
            shadow_condition(&value.right, locals);
        }
        _ => {}
    }
}

struct Inference<'a> {
    facts: &'a TypecheckFacts,
    known: &'a BTreeMap<String, BorrowSummary>,
}

impl Inference<'_> {
    // Expressions here cannot contain an explicit return. Their value must
    // come from the same parameter on every possible value-producing path.
    fn origin_of(&self, expr: &HirExpression, locals: &Origins) -> Option<usize> {
        if expr_has_return(expr) {
            return None;
        }
        match expr {
            HirExpression::Name(_, name) => locals.get(name).copied().flatten(),
            HirExpression::Slice(slice) => self.origin_of(&slice.object, locals),
            HirExpression::Block(block) => self.block_origin(block, locals),
            HirExpression::If(branch) => {
                let mut then_locals = locals.clone();
                shadow_condition(&branch.condition, &mut then_locals);
                let then_src = self.block_origin(&branch.then_block, &then_locals)?;
                let else_src = self.origin_of(branch.else_branch.as_deref()?, locals)?;
                (then_src == else_src).then_some(then_src)
            }
            HirExpression::Match(value) => {
                let mut source = None;
                for arm in &value.arms {
                    let mut arm_locals = locals.clone();
                    shadow_pattern(&arm.pattern, &mut arm_locals);
                    record_origin(&mut source, self.origin_of(&arm.value, &arm_locals)?)?;
                }
                source
            }
            HirExpression::Call(call) => {
                // Use the actual target and argument mapping established by
                // typeck, including named arguments and imported functions.
                let (key, indices) = self.facts.direct_call_parameters.get(&call.span)?;
                let summary = self.known.get(key)?;
                let index = indices
                    .iter()
                    .position(|index| *index == Some(summary.return_borrow.parameter_index))?;
                let argument = match call.args.get(index)? {
                    HirCallArgument::Positional(value) => value,
                    HirCallArgument::Named(named) => &named.value,
                };
                self.origin_of(argument, locals)
            }
            _ => None,
        }
    }

    fn block_origin(&self, block: &HirBlock, outside: &Origins) -> Option<usize> {
        let mut source = None;
        if self.scan_block(block, outside, true, &mut source)? {
            return None;
        }
        source
    }

    // Return true if execution can fall through this block. Every explicit
    // return (and the implicit tail, when requested) contributes to `source`.
    // Unknown exits fail closed; the independent borrow-flow pass still
    // validates lifetime, access, escape and mutation on the actual CFG.
    fn scan_block(
        &self,
        block: &HirBlock,
        outside: &Origins,
        tail_returns: bool,
        source: &mut Option<usize>,
    ) -> Option<bool> {
        let mut locals = outside.clone();
        for (index, stmt) in block.statements.iter().enumerate() {
            let tail = tail_returns && index + 1 == block.statements.len();
            match stmt {
                HirStatement::Let(value) if !expr_has_return(&value.value) => {
                    let origin = self.origin_of(&value.value, &locals);
                    locals.insert(value.name.clone(), origin);
                }
                HirStatement::Var(value) if !expr_has_return(&value.value) => {
                    // Mutable aliases have no proven stable origin here.
                    locals.insert(value.name.clone(), None);
                }
                HirStatement::Return(Some(value)) => {
                    record_origin(source, self.origin_of(value, &locals)?)?;
                    return Some(false);
                }
                HirStatement::Expr(value) => {
                    if !self.scan_expr(value, &locals, tail, source)? {
                        return Some(false);
                    }
                }
                HirStatement::For(loop_)
                    if !expr_has_return(&loop_.iterable)
                        && !loop_.filter.as_ref().is_some_and(expr_has_return)
                        && !has_explicit_return(&loop_.body) => {}
                // Do not guess about return paths in loops, initializers,
                // matches with early exits, or nested arbitrary expressions.
                _ => return None,
            }
        }
        Some(true)
    }

    fn scan_expr(
        &self,
        value: &HirExpression,
        locals: &Origins,
        tail_returns: bool,
        source: &mut Option<usize>,
    ) -> Option<bool> {
        if !expr_has_return(value) {
            if tail_returns {
                record_origin(source, self.origin_of(value, locals)?)?;
                return Some(false);
            }
            return Some(true);
        }
        match value {
            HirExpression::Block(block) => self.scan_block(block, locals, tail_returns, source),
            HirExpression::If(branch) if !expr_has_return(&branch.condition) => {
                let mut then_locals = locals.clone();
                shadow_condition(&branch.condition, &mut then_locals);
                let then_falls =
                    self.scan_block(&branch.then_block, &then_locals, tail_returns, source)?;
                let else_falls = match &branch.else_branch {
                    Some(other) => self.scan_expr(other, locals, tail_returns, source)?,
                    None => true,
                };
                Some(then_falls || else_falls)
            }
            _ => None,
        }
    }
}

fn record_origin(source: &mut Option<usize>, candidate: usize) -> Option<()> {
    if source.is_some_and(|existing| existing != candidate) {
        return None;
    }
    *source = Some(candidate);
    Some(())
}

/// Recognize a deliberately small class of generic observer bodies.
/// The callsite must separately prove the concrete type and builtin dispatch.
pub(crate) fn slice_observer_candidate(function: &HirFunction) -> Option<(usize, String)> {
    if function.is_extern || function.is_fallible {
        return None;
    }
    let [statement] = function.body.as_ref()?.statements.as_slice() else {
        return None;
    };
    let expression = match statement {
        HirStatement::Expr(value) | HirStatement::Return(Some(value)) => value,
        _ => return None,
    };
    let HirExpression::Call(call) = expression else {
        return None;
    };
    if !call.args.is_empty() || call.type_args.is_some() || !call.inout_arguments.is_empty() {
        return None;
    }
    let HirExpression::Member(member) = call.callee.as_ref() else {
        return None;
    };
    if !matches!(member.field.as_str(), "len" | "is_empty") {
        return None;
    }
    let HirExpression::Name(_, name) = member.object.as_ref() else {
        return None;
    };
    // All other parameters must be compile-time type parameters. This rules
    // out forwarding, secondary runtime values, and extra effects in the body.
    let mut runtime = function
        .parameters
        .iter()
        .enumerate()
        .filter(|(_, p)| p.kind != ParameterKind::Type);
    let (index, parameter) = runtime.next()?;
    if runtime.next().is_some()
        || parameter.kind != ParameterKind::Runtime
        || parameter.is_inout
        || &parameter.name != name
    {
        return None;
    }
    Some((index, member.field.clone()))
}

fn infer_one(
    function: &HirFunction,
    facts: &TypecheckFacts,
    known: &BTreeMap<String, BorrowSummary>,
) -> Option<BorrowSummary> {
    if !is_slice(function.return_type.as_ref()?) || function.is_extern {
        return None;
    }
    let body = function.body.as_ref()?;
    let mut local_origins = HashMap::new();
    let accesses = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, param)| {
            if is_slice(&param.ty) {
                local_origins.insert(param.name.clone(), Some(index));
                if param.is_inout {
                    BorrowAccess::Exclusive
                } else {
                    BorrowAccess::Read
                }
            } else {
                local_origins.insert(param.name.clone(), None);
                BorrowAccess::Value
            }
        })
        .collect::<Vec<_>>();
    let parameter_index = Inference { facts, known }.block_origin(body, &local_origins)?;
    if !is_slice(&function.parameters.get(parameter_index)?.ty) {
        return None;
    }
    Some(BorrowSummary {
        parameter_accesses: accesses,
        return_borrow: ReturnBorrow {
            parameter_index,
            mode: BorrowAccess::Read,
        },
    })
}

/// Infer modular, order-independent contracts for a conservative set of
/// concrete, directly callable functions using the corresponding module's
/// structural typecheck facts. An unresolved cycle remains without a summary,
/// and is rejected rather than accepted speculatively.
pub fn infer(files: &[(&str, &HirFile, &TypecheckFacts)]) -> BTreeMap<String, BorrowSummary> {
    let mut definitions = BTreeMap::<String, (&HirFunction, &TypecheckFacts)>::new();
    for &(module, file, facts) in files {
        for item in &file.items {
            if let HirItem::Function(function) = item {
                definitions.insert(qualify(module, &function.name), (function, facts));
            }
        }
    }
    let mut summaries = BTreeMap::new();
    for _ in 0..definitions.len() {
        let mut changed = false;
        for (key, (function, facts)) in &definitions {
            if summaries.contains_key(key) {
                continue;
            }
            if let Some(summary) = infer_one(function, facts, &summaries) {
                summaries.insert(key.clone(), summary);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    summaries
}
