//! Conservative, inferred borrowed-return contracts (Memory & Ownership v0.4 §14.8–14.9).
//!
//! Only direct, identifiable, single-source read-only Slice returns are
//! summarized. Unknown, recursive and control-flow-dependent returns are not
//! promoted to borrow-safe contracts. The summaries are compiler metadata,
//! not lifetime annotations in the Tempra surface language.

use std::collections::{BTreeMap, HashMap};
use tempra_hir::{
    HirBlock, HirCall, HirCallArgument, HirExpression, HirFile, HirFunction, HirItem, HirStatement,
    HirType, ParameterKind,
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

/// Find the expression corresponding to a parameter, honoring named arguments
/// and source-order positional arguments with type parameters skipped.
pub(crate) fn call_argument<'a>(
    call: &'a HirCall,
    parameter_index: usize,
    names: &[String],
    kinds: &[ParameterKind],
) -> Option<&'a HirExpression> {
    let mut position = 0;
    for arg in &call.args {
        match arg {
            HirCallArgument::Named(named) => {
                if names
                    .get(parameter_index)
                    .is_some_and(|name| name == &named.name)
                {
                    return Some(&named.value);
                }
            }
            HirCallArgument::Positional(value) => {
                while kinds
                    .get(position)
                    .is_some_and(|kind| *kind == ParameterKind::Type)
                {
                    position += 1;
                }
                if position == parameter_index {
                    return Some(value);
                }
                position += 1;
            }
        }
    }
    None
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

// Deliberately conservative: a body containing any explicit return (including
// nested conditional/loop/block returns) requires a later path-sensitive
// contract inference pass. Never infer from only the last expression if an
// earlier path might escape a different owner.
fn has_explicit_return(block: &HirBlock) -> bool {
    block.statements.iter().any(|stmt| match stmt {
        HirStatement::Return(_) => true,
        HirStatement::For(loop_) => has_explicit_return(&loop_.body),
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

fn origin_of(
    expr: &HirExpression,
    locals: &HashMap<String, usize>,
    module: &str,
    definitions: &BTreeMap<String, &HirFunction>,
    known: &BTreeMap<String, BorrowSummary>,
) -> Option<usize> {
    match expr {
        HirExpression::Name(_, name) => locals.get(name).copied(),
        HirExpression::Slice(slice) => origin_of(&slice.object, locals, module, definitions, known),
        HirExpression::Block(block) => block_origin(block, locals, module, definitions, known),
        HirExpression::If(branch) => {
            let then_src = block_origin(&branch.then_block, locals, module, definitions, known)?;
            let else_src = origin_of(
                branch.else_branch.as_deref()?,
                locals,
                module,
                definitions,
                known,
            )?;
            (then_src == else_src).then_some(then_src)
        }
        HirExpression::Match(m) => {
            let mut arms = m.arms.iter();
            let first = origin_of(&arms.next()?.value, locals, module, definitions, known)?;
            for arm in arms {
                if origin_of(&arm.value, locals, module, definitions, known)? != first {
                    return None;
                }
            }
            Some(first)
        }
        HirExpression::Call(call) => {
            let HirExpression::Name(_, callee) = &*call.callee else {
                return None;
            };
            let (key, def) = resolve_direct(definitions, module, callee)?;
            let summary = known.get(key)?;
            let names = def
                .parameters
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>();
            let kinds = def.parameters.iter().map(|p| p.kind).collect::<Vec<_>>();
            let argument =
                call_argument(call, summary.return_borrow.parameter_index, &names, &kinds)?;
            origin_of(argument, locals, module, definitions, known)
        }
        _ => None,
    }
}

fn block_origin(
    block: &HirBlock,
    outside: &HashMap<String, usize>,
    module: &str,
    definitions: &BTreeMap<String, &HirFunction>,
    known: &BTreeMap<String, BorrowSummary>,
) -> Option<usize> {
    let mut locals = outside.clone();
    for (index, stmt) in block.statements.iter().enumerate() {
        let last = index + 1 == block.statements.len();
        if !last && matches!(stmt, HirStatement::Return(_)) {
            return None;
        }
        match stmt {
            HirStatement::Let(l) if !expr_has_return(&l.value) => {
                let origin = origin_of(&l.value, &locals, module, definitions, known)?;
                locals.insert(l.name.clone(), origin);
            }
            HirStatement::Expr(value) if last && !expr_has_return(value) => {
                return origin_of(value, &locals, module, definitions, known);
            }
            HirStatement::Return(Some(value)) if last && !expr_has_return(value) => {
                return origin_of(value, &locals, module, definitions, known);
            }
            // Mutable aliases and intermediate statements need a more advanced
            // control-flow summary; do not guess their provenance.
            _ => return None,
        }
    }
    None
}

fn infer_one(
    function: &HirFunction,
    module: &str,
    definitions: &BTreeMap<String, &HirFunction>,
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
                local_origins.insert(param.name.clone(), index);
                if param.is_inout {
                    BorrowAccess::Exclusive
                } else {
                    BorrowAccess::Read
                }
            } else {
                BorrowAccess::Value
            }
        })
        .collect::<Vec<_>>();
    let parameter_index = block_origin(body, &local_origins, module, definitions, known)?;
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
/// concrete, directly callable functions. An unresolved cycle remains without
/// a summary, and is rejected rather than accepted speculatively.
pub fn infer(files: &[(&str, &HirFile)]) -> BTreeMap<String, BorrowSummary> {
    let mut definitions = BTreeMap::<String, &HirFunction>::new();
    for &(module, file) in files {
        for item in &file.items {
            if let HirItem::Function(function) = item {
                definitions.insert(qualify(module, &function.name), function);
            }
        }
    }
    let mut summaries = BTreeMap::new();
    for _ in 0..definitions.len() {
        let mut changed = false;
        for (key, function) in &definitions {
            if summaries.contains_key(key) {
                continue;
            }
            let module = key.rsplit_once('.').map(|(module, _)| module).unwrap_or("");
            if let Some(summary) = infer_one(function, module, &definitions, &summaries) {
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
