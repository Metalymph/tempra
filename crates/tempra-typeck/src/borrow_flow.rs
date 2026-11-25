//! Source-level, path-sensitive loan checks for identifiable local owners.
//!
//! The public memory contract is `docs/Tempra_Memory_Ownership_v0.4.md` §14.
//! This pass runs after structural type checking. It deliberately handles only
//! sources whose provenance is representable by a local binding identity: it
//! is not a replacement for interprocedural BorrowSummary inference.
//!
//! No runtime repair, copy insertion or implicit lifetime extension occurs.

use crate::context::{FunctionSignature, TypeContext};
use crate::error::TypeError;
use crate::facts::BorrowFact;
use crate::types::Type;
use std::collections::{BTreeSet, HashMap};
use tempra_hir::*;
use tempra_span::Span;

type BindingId = usize;
type NodeId = usize;

#[derive(Clone)]
struct Binding {
    name: String,
    span: Span,
    returnable_borrow_source: bool,
    loan: Option<Loan>,
}

#[derive(Clone)]
struct Loan {
    owners: BTreeSet<BindingId>,
    origin: Span,
    closure: bool,
    aggregate: bool,
    exclusive_owners: BTreeSet<BindingId>,
}

/// Access to an owner, optionally through an authorized borrowed binding.
struct Access {
    owner: BindingId,
    via: Option<BindingId>,
    exclusive: bool,
    span: Span,
}

#[derive(Default)]
struct Node {
    next: Vec<NodeId>,
    uses: BTreeSet<BindingId>,
    defines: BTreeSet<BindingId>,
    accesses: Vec<Access>,
}

#[derive(Default)]
struct LoopExit {
    header: NodeId,
    breaks: Vec<NodeId>,
}

struct Analyzer<'a> {
    ctx: &'a TypeContext,
    bindings: Vec<Binding>,
    scopes: Vec<HashMap<String, BindingId>>,
    nodes: Vec<Node>,
    loops: Vec<LoopExit>,
    expression_loans: HashMap<Span, Option<Loan>>,
    block_loans: HashMap<Span, Option<Loan>>,
    errors: Vec<TypeError>,
    facts: Vec<BorrowFact>,
    /// Parameter owner authorized by the inferred return contract, if any.
    return_source: Option<BindingId>,
    returns_slice: bool,
}

impl<'a> Analyzer<'a> {
    fn new(ctx: &'a TypeContext) -> Self {
        Self {
            ctx,
            bindings: Vec::new(),
            scopes: vec![HashMap::new()],
            nodes: Vec::new(),
            loops: Vec::new(),
            expression_loans: HashMap::new(),
            block_loans: HashMap::new(),
            errors: Vec::new(),
            facts: Vec::new(),
            return_source: None,
            returns_slice: false,
        }
    }

    fn error(&mut self, message: String, span: Span) {
        let mut err = TypeError::new(message, span);
        if !self.ctx.current_module.is_empty() {
            err.module_name = Some(self.ctx.current_module.clone());
        }
        self.errors.push(err);
    }

    fn bind(&mut self, name: &str, span: Span, loan: Option<Loan>) -> BindingId {
        let id = self.bindings.len();
        if let Some(loan) = &loan {
            for &owner in &loan.owners {
                self.facts.push(BorrowFact {
                    borrower: span,
                    owner: self.bindings[owner].span,
                    origin: loan.origin,
                });
            }
        }
        self.bindings.push(Binding {
            name: name.to_owned(),
            span,
            returnable_borrow_source: false,
            loan,
        });
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(name.to_owned(), id);
        id
    }

    fn lookup(&self, name: &str) -> Option<BindingId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn connect(&mut self, from: &[NodeId], to: NodeId) {
        for &id in from {
            if !self.nodes[id].next.contains(&to) {
                self.nodes[id].next.push(to);
            }
        }
    }

    fn node(&mut self, previous: Vec<NodeId>, node: Node) -> Vec<NodeId> {
        let id = self.nodes.len();
        self.nodes.push(node);
        self.connect(&previous, id);
        vec![id]
    }

    fn noop(&mut self, previous: Vec<NodeId>) -> Vec<NodeId> {
        self.node(previous, Node::default())
    }

    fn use_name(&mut self, name: &str, span: Span, previous: Vec<NodeId>) -> Vec<NodeId> {
        let Some(id) = self.lookup(name) else {
            return previous;
        };
        let mut node = Node::default();
        node.uses.insert(id);
        // A read of the owner is incompatible with a live exclusive loan.
        node.accesses.push(Access {
            owner: id,
            via: Some(id),
            exclusive: false,
            span,
        });
        if let Some(loan) = &self.bindings[id].loan {
            for &owner in &loan.owners {
                if owner != id {
                    node.accesses.push(Access {
                        owner,
                        via: Some(id),
                        exclusive: false,
                        span,
                    });
                }
            }
        }
        self.node(previous, node)
    }

    fn place_binding(&self, expr: &HirExpression) -> Option<BindingId> {
        match expr {
            HirExpression::Name(_, name) => self.lookup(name),
            HirExpression::Slice(slice) => self.place_binding(&slice.object),
            HirExpression::Member(member) => self.place_binding(&member.object),
            HirExpression::Index(index) => self.place_binding(&index.object),
            _ => None,
        }
    }

    fn source_owners(&self, expr: &HirExpression) -> BTreeSet<BindingId> {
        self.place_binding(expr).map_or_else(BTreeSet::new, |id| {
            self.bindings[id].loan.as_ref().map_or_else(
                || BTreeSet::from([id]),
                |loan| {
                    let mut owners = loan.owners.clone();
                    if loan.aggregate {
                        owners.insert(id);
                    }
                    owners
                },
            )
        })
    }

    // Projections of definitely owned values do not retain a loan. Missing
    // type facts are intentionally conservative, not proof of ownership.
    fn plain_type(ty: &Type) -> bool {
        match ty {
            Type::Unit | Type::Bool | Type::String | Type::Never => true,
            Type::Array(t) | Type::Option(t) => Self::plain_type(t),
            Type::Tuple(ts) => ts.iter().all(Self::plain_type),
            Type::Result(a, b) => Self::plain_type(a) && Self::plain_type(b),
            _ => ty.is_numeric(),
        }
    }

    fn projection_loan(&mut self, object: &HirExpression, span: Span) -> Option<Loan> {
        let ty = self.ctx.facts.expr_types.get(&span).cloned();
        if ty.as_ref().is_some_and(Self::plain_type) {
            return None;
        }
        let mut loan = self.loan_of(object)?;
        // A projection conservatively inherits all possible owners. Only a
        // known Slice projection may use the existing single-source summary.
        if matches!(ty, Some(Type::Slice(_))) {
            loan.aggregate = false;
        }
        loan.origin = span;
        Some(loan)
    }

    fn aggregate_loan<'e>(
        &mut self,
        values: impl IntoIterator<Item = &'e HirExpression>,
        span: Span,
    ) -> Option<Loan> {
        let mut result = Loan {
            owners: BTreeSet::new(),
            origin: span,
            closure: false,
            aggregate: true,
            exclusive_owners: BTreeSet::new(),
        };
        for value in values {
            if let Some(loan) = self.loan_of(value) {
                result.owners.extend(loan.owners);
                result.exclusive_owners.extend(loan.exclusive_owners);
                result.closure |= loan.closure;
            }
        }
        (!result.owners.is_empty()).then_some(result)
    }

    // Cached expression results are computed in their lexical scope, before
    // branch/match/block bindings disappear. This is not a name-based join.
    fn loan_of(&mut self, expr: &HirExpression) -> Option<Loan> {
        if let Some(loan) = self.expression_loans.get(&expr.span()) {
            return loan.clone();
        }
        self.infer_loan(expr)
    }

    fn infer_loan(&mut self, expr: &HirExpression) -> Option<Loan> {
        match expr {
            HirExpression::Name(span, name) => self.lookup(name).and_then(|id| {
                if self
                    .ctx
                    .facts
                    .expr_types
                    .get(span)
                    .is_some_and(Self::plain_type)
                {
                    return None;
                }
                self.bindings[id]
                    .loan
                    .clone()
                    .map(|mut loan| {
                        if matches!(self.ctx.facts.expr_types.get(span), Some(Type::Slice(_))) {
                            loan.aggregate = false;
                        }
                        loan
                    })
                    .or_else(|| {
                        self.bindings[id].returnable_borrow_source.then(|| Loan {
                            owners: BTreeSet::from([id]),
                            origin: *span,
                            closure: false,
                            aggregate: false,
                            exclusive_owners: BTreeSet::new(),
                        })
                    })
            }),
            HirExpression::Slice(slice) => {
                // A user-defined slice hook can return an owned scalar.
                if self
                    .ctx
                    .facts
                    .expr_types
                    .get(&slice.span)
                    .is_some_and(Self::plain_type)
                {
                    return None;
                }
                let mut owners = self.source_owners(&slice.object);
                if owners.is_empty() {
                    if let Some(loan) = self.loan_of(&slice.object) {
                        owners = loan.owners;
                    }
                }
                if owners.is_empty() {
                    if matches!(
                        self.ctx.facts.expr_types.get(&slice.span),
                        Some(Type::Slice(_))
                    ) {
                        self.error(
                            "borrow provenance unavailable: slice requires an identifiable owner"
                                .into(),
                            slice.span,
                        );
                    }
                    return None;
                }
                Some(Loan {
                    owners,
                    origin: slice.span,
                    closure: false,
                    aggregate: false,
                    exclusive_owners: BTreeSet::new(),
                })
            }
            HirExpression::Call(call) => {
                // Option/Result constructors remain calls in HIR. User enum
                // constructors are rewritten to EnumConstructor by typeck.
                if matches!(&*call.callee, HirExpression::Name(_, name)
                    if matches!(name.as_str(), "Some" | "Ok" | "Err")
                        && self.lookup(name).is_none() && self.direct_callee(call).is_none())
                {
                    self.aggregate_loan(
                        call.args.iter().map(|arg| match arg {
                            HirCallArgument::Positional(value) => value,
                            HirCallArgument::Named(named) => &named.value,
                        }),
                        call.span,
                    )
                } else {
                    self.loan_of_call(call)
                }
            }
            HirExpression::Tuple(span, values) | HirExpression::Array(span, values) => {
                self.aggregate_loan(values, *span)
            }
            HirExpression::StructLiteral(value) => {
                self.aggregate_loan(value.fields.iter().map(|(_, value)| value), value.span)
            }
            HirExpression::EnumConstructor(value) => {
                self.aggregate_loan(&value.payloads, value.span)
            }
            HirExpression::Member(value) => self.projection_loan(&value.object, value.span),
            HirExpression::Index(value) => self.projection_loan(&value.object, value.span),
            HirExpression::Unwrap(value) => self.projection_loan(&value.expr, value.span),
            HirExpression::Try(value) => self.projection_loan(&value.expr, value.span),
            HirExpression::AsOptional(value) => self.projection_loan(&value.expr, value.span),
            HirExpression::AsForced(value) => self.projection_loan(&value.expr, value.span),
            HirExpression::Comptime(value) => self.loan_of(&value.expression),
            HirExpression::Closure(closure) => {
                let mut owners = BTreeSet::new();
                let mut exclusive_owners = BTreeSet::new();
                for capture in &closure.captures {
                    let HirCapture::Named(named) = capture else {
                        continue;
                    };
                    let Some(id) = self.lookup(&named.name) else {
                        continue;
                    };
                    if named.mode == HirCaptureMode::Consume
                        && self.bindings[id].loan.is_none()
                        && !self.bindings[id].returnable_borrow_source
                    {
                        continue;
                    }
                    let mut origins = self.bindings[id]
                        .loan
                        .as_ref()
                        .map_or_else(|| BTreeSet::from([id]), |loan| loan.owners.clone());
                    if named.mode != HirCaptureMode::Consume {
                        origins.insert(id);
                    }
                    if named.mode == HirCaptureMode::Inout {
                        exclusive_owners.extend(origins.iter().copied());
                    }
                    if let Some(loan) = &self.bindings[id].loan {
                        exclusive_owners.extend(loan.exclusive_owners.iter().copied());
                    }
                    owners.extend(origins);
                }
                if owners.is_empty() {
                    None
                } else {
                    Some(Loan {
                        owners,
                        exclusive_owners,
                        origin: closure.span,
                        closure: true,
                        aggregate: false,
                    })
                }
            }
            HirExpression::If(branch) => {
                let then_loan = self
                    .block_loans
                    .get(&branch.then_block.span)
                    .cloned()
                    .flatten();
                let else_loan = branch
                    .else_branch
                    .as_ref()
                    .and_then(|expr| self.loan_of(expr));
                self.merge_loans(then_loan, else_loan, branch.span)
            }
            HirExpression::Match(m) => {
                let mut arms = m.arms.iter();
                let mut origin = arms.next().and_then(|arm| self.loan_of(&arm.value));
                for arm in arms {
                    let candidate = self.loan_of(&arm.value);
                    origin = self.merge_loans(origin, candidate, m.span);
                }
                origin
            }
            HirExpression::Block(b) => self.block_loans.get(&b.span).cloned().flatten(),
            HirExpression::For(f) => {
                if self
                    .block_loans
                    .get(&f.body.span)
                    .is_some_and(Option::is_some)
                {
                    // The result collects values across iterations. Until a
                    // per-element origin contract is available, fail closed.
                    self.error("borrow provenance: borrowed for-expression results require a verified iteration contract".into(), f.span);
                }
                None
            }
            _ => None,
        }
    }

    fn loan_of_call(&mut self, call: &HirCall) -> Option<Loan> {
        let (key, _) = self.direct_callee(call)?;
        let parameter = self
            .ctx
            .borrow_summaries
            .get(&key)?
            .return_borrow
            .parameter_index;
        let indices = &self.ctx.facts.direct_call_parameters.get(&call.span)?.1;
        let position = indices.iter().position(|index| *index == Some(parameter))?;
        let argument = match call.args.get(position)? {
            HirCallArgument::Positional(value) => value,
            HirCallArgument::Named(named) => &named.value,
        };
        let mut loan = self.loan_of(argument)?;
        loan.origin = call.span;
        loan.exclusive_owners.clear();
        Some(loan)
    }

    fn merge_loans(&mut self, left: Option<Loan>, right: Option<Loan>, span: Span) -> Option<Loan> {
        match (left, right) {
            (Some(a), Some(b))
                if a.owners == b.owners
                    && a.exclusive_owners == b.exclusive_owners
                    && a.closure == b.closure
                    && a.aggregate == b.aggregate =>
            {
                Some(a)
            }
            (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {
                self.error(
                    "borrow provenance conflict: branch results have no common borrowed owner"
                        .into(),
                    span,
                );
                None
            }
            (None, None) => None,
        }
    }

    fn check_return_loan(&mut self, expr: &HirExpression) {
        let loan = self.loan_of(expr);
        if self.returns_slice && loan.is_none() {
            self.error("borrow return provenance unavailable: Slice return has no statically verified owner".into(), expr.span());
            return;
        }
        if let Some(loan) = loan {
            for owner in loan.owners.iter().copied() {
                let name = self.bindings[owner].name.clone();
                if loan.closure || loan.aggregate || !self.bindings[owner].returnable_borrow_source
                {
                    self.error(format!("borrow escape: cannot return a view or closure borrowed from local owner '{name}'"), expr.span());
                } else if self.return_source != Some(owner) {
                    // A return is valid only when *every* supported return
                    // path was proven to derive from this exact parameter.
                    self.error(format!("borrow return contract unavailable: returning a view of parameter '{name}' requires a propagated BorrowSummary"), expr.span());
                }
            }
        }
    }

    fn place_access(
        &mut self,
        target: &HirExpression,
        previous: Vec<NodeId>,
        span: Span,
        exclusive: bool,
    ) -> Vec<NodeId> {
        let Some(via) = self.place_binding(target) else {
            return previous;
        };
        let mut node = Node::default();
        // Mutation/reading through a borrowed binding is a use of that loan.
        if self.bindings[via].loan.is_some() {
            node.uses.insert(via);
        }
        let mut owners = self.source_owners(target);
        owners.insert(via);
        for owner in owners {
            node.accesses.push(Access {
                owner,
                via: Some(via),
                exclusive,
                span,
            });
        }
        self.node(previous, node)
    }

    fn mutation(
        &mut self,
        target: &HirExpression,
        previous: Vec<NodeId>,
        span: Span,
    ) -> Vec<NodeId> {
        self.place_access(target, previous, span, true)
    }

    // A temporary is not a lexical name. Its definition and final use make
    // the loan live during evaluation of later operands and the call itself.
    fn hold_loan(&mut self, loan: Loan, previous: Vec<NodeId>) -> (Vec<NodeId>, BindingId) {
        let id = self.bindings.len();
        self.bindings.push(Binding {
            name: "<temporary borrow>".into(),
            span: loan.origin,
            returnable_borrow_source: false,
            loan: Some(loan),
        });
        let mut node = Node::default();
        node.defines.insert(id);
        (self.node(previous, node), id)
    }

    fn finish_operands(&mut self, previous: Vec<NodeId>, held: Vec<BindingId>) -> Vec<NodeId> {
        if held.is_empty() {
            return previous;
        }
        let mut node = Node::default();
        node.uses.extend(held);
        self.node(previous, node)
    }

    fn eval_operands<'e>(
        &mut self,
        values: impl IntoIterator<Item = &'e HirExpression>,
        mut previous: Vec<NodeId>,
    ) -> Vec<NodeId> {
        let mut held = Vec::new();
        for value in values {
            previous = self.expr(value, previous);
            if let Some(loan) = self.loan_of(value) {
                if !loan.exclusive_owners.is_empty() {
                    self.error("borrow provenance: exclusive borrowed value cannot be copied into an aggregate".into(), value.span());
                }
                let (next, id) = self.hold_loan(loan, previous);
                previous = next;
                held.push(id);
            }
        }
        self.finish_operands(previous, held)
    }

    fn direct_callee(&self, call: &HirCall) -> Option<(String, FunctionSignature)> {
        // Use the actual name and argument mapping recorded by typeck. Never
        // guess a generic/module target or reinterpret a shadowed name.
        let (key, _) = self.ctx.facts.direct_call_parameters.get(&call.span)?;
        self.ctx
            .functions
            .get(key)
            .map(|sig| (key.clone(), sig.clone()))
    }

    fn callback_loan(&mut self, expr: &HirExpression) -> Option<Loan> {
        let loan = self.loan_of(expr)?;
        loan.closure.then_some(loan)
    }

    fn argument_loan(
        &mut self,
        expr: &HirExpression,
        exclusive: bool,
        slice: bool,
    ) -> Option<Loan> {
        let existing = self.loan_of(expr);
        if exclusive || slice {
            if existing
                .as_ref()
                .is_some_and(|loan| loan.aggregate || loan.closure)
            {
                return existing;
            }
            let owners = self.source_owners(expr);
            if !owners.is_empty() {
                return Some(Loan {
                    exclusive_owners: if exclusive {
                        owners.clone()
                    } else {
                        BTreeSet::new()
                    },
                    owners,
                    origin: expr.span(),
                    closure: false,
                    aggregate: false,
                });
            }
        }
        existing
    }

    fn acquire_argument(
        &mut self,
        value: &HirExpression,
        loan: Loan,
        previous: Vec<NodeId>,
        held: &mut Vec<BindingId>,
    ) -> Vec<NodeId> {
        let via = self.place_binding(value);
        // Check simultaneous loans explicitly, including a callback whose last
        // ordinary use occurs before the call node. Keep its loan to call end.
        for &id in held.iter() {
            if let Some(other) = &self.bindings[id].loan {
                let conflict = loan.owners.iter().any(|owner| {
                    other.owners.contains(owner)
                        && (loan.exclusive_owners.contains(owner)
                            || other.exclusive_owners.contains(owner))
                });
                if conflict {
                    self.error(
                        "borrow alias: conflicting simultaneous accesses in one call".into(),
                        value.span(),
                    );
                    break;
                }
            }
        }
        let mut node = Node::default();
        if let Some(id) = via {
            node.uses.insert(id);
        }
        for &owner in &loan.owners {
            node.accesses.push(Access {
                owner,
                via,
                exclusive: loan.exclusive_owners.contains(&owner),
                span: value.span(),
            });
        }
        let previous = self.node(previous, node);
        let (next, id) = self.hold_loan(loan, previous);
        held.push(id);
        next
    }

    fn expr(&mut self, expr: &HirExpression, previous: Vec<NodeId>) -> Vec<NodeId> {
        let next = self.expr_inner(expr, previous);
        let loan = self.infer_loan(expr);
        self.expression_loans.insert(expr.span(), loan);
        next
    }

    fn expr_inner(&mut self, expr: &HirExpression, mut previous: Vec<NodeId>) -> Vec<NodeId> {
        match expr {
            HirExpression::Name(span, name) => self.use_name(name, *span, previous),
            HirExpression::Unary(e) => self.expr(&e.expr, previous),
            HirExpression::Binary(e) if e.op == HirBinaryOp::Assign => {
                previous = self.expr(&e.right, previous);
                if let Some(loan) = self.loan_of(&e.right) {
                    // Builtin scalar range assignment copies elements, not the
                    // Slice descriptor. The RHS remains borrowed until its
                    // elements have been read, after evaluation of the bounds.
                    if let HirExpression::Slice(slice) = e.left.as_ref() {
                        let scalar_source = matches!(
                            self.ctx.facts.expr_types.get(&e.right.span()),
                            Some(Type::Slice(item)) if item.is_numeric()
                                || matches!(item.as_ref(), Type::Bool)
                        );
                        let scalar_target = matches!(
                            self.ctx.facts.expr_types.get(&slice.object.span()),
                            Some(Type::Array(item)) if item.is_numeric()
                                || matches!(item.as_ref(), Type::Bool)
                        );
                        if scalar_source
                            && scalar_target
                            && matches!(slice.object.as_ref(), HirExpression::Name(_, _))
                            && !loan.aggregate
                            && !loan.closure
                            && loan.exclusive_owners.is_empty()
                        {
                            let (next, held) = self.hold_loan(loan, previous);
                            previous = self.expr(&slice.start, next);
                            previous = self.expr(&slice.end, previous);
                            previous = self.finish_operands(previous, vec![held]);
                            return self.mutation(&slice.object, previous, e.span);
                        }
                    }
                    self.error("borrow provenance: storing a borrowed value by assignment requires a verified flow contract".into(), e.span);
                }
                match &*e.left {
                    HirExpression::Name(span, name) => {
                        if let Some(id) = self.lookup(name) {
                            if self.bindings[id].loan.is_some() {
                                self.error(format!("borrow provenance: reassignment of borrowed binding '{name}' requires a verified flow contract"), *span);
                            }
                        }
                        self.mutation(&e.left, previous, e.span)
                    }
                    HirExpression::Index(index) => {
                        previous = self.expr(&index.index, previous);
                        self.mutation(&index.object, previous, e.span)
                    }
                    HirExpression::Slice(slice) => {
                        previous = self.expr(&slice.start, previous);
                        previous = self.expr(&slice.end, previous);
                        self.mutation(&slice.object, previous, e.span)
                    }
                    HirExpression::Member(member) => {
                        self.mutation(&member.object, previous, e.span)
                    }
                    target => self.mutation(target, previous, e.span),
                }
            }
            HirExpression::Binary(e) => {
                previous = self.expr(&e.left, previous);
                self.expr(&e.right, previous)
            }
            HirExpression::Call(call) => {
                // A direct Slice-returning function needs a certified
                // provenance contract; local callable aliases are not direct.
                if let Some((key, signature)) = self.direct_callee(call) {
                    if matches!(&signature.return_type, Type::Slice(_)) {
                        if !self.ctx.borrow_summaries.contains_key(&key) {
                            let name = match &*call.callee {
                                HirExpression::Name(_, name) => name.as_str(),
                                _ => "<dynamic>",
                            };
                            self.error(format!("borrow return contract unavailable: function '{name}' returns a Slice without a proven single-owner BorrowSummary"), call.span);
                        }
                    }
                }
                previous = self.expr(&call.callee, previous);
                let direct = self.direct_callee(call);
                let mut held = Vec::new();
                let is_constructor = matches!(&*call.callee, HirExpression::Name(_, name)
                    if matches!(name.as_str(), "Some" | "Ok" | "Err")
                        && self.lookup(name).is_none() && direct.is_none());
                if let HirExpression::Member(member) = &*call.callee {
                    if self
                        .loan_of(&member.object)
                        .is_some_and(|loan| loan.aggregate)
                    {
                        self.error("borrow aggregate escape: receiver requires a verified non-escaping contract".into(), member.span);
                    }
                    let exclusive = self
                        .ctx
                        .facts
                        .receiver_accesses
                        .get(&call.span)
                        .copied()
                        .unwrap_or(false);
                    let slice = matches!(
                        self.ctx.facts.expr_types.get(&member.object.span()),
                        Some(Type::Slice(_))
                    );
                    if exclusive || slice {
                        if let Some(loan) = self.argument_loan(&member.object, exclusive, slice) {
                            previous =
                                self.acquire_argument(&member.object, loan, previous, &mut held);
                        } else if exclusive {
                            self.error("borrow provenance unavailable: exclusive receiver requires an identifiable owner".into(), member.span);
                        }
                    }
                } else if let Some(loan) = self.callback_loan(&call.callee) {
                    previous = self.acquire_argument(&call.callee, loan, previous, &mut held);
                }
                for (arg_index, arg) in call.args.iter().enumerate() {
                    let value = match arg {
                        HirCallArgument::Positional(value) => value,
                        HirCallArgument::Named(named) => &named.value,
                    };
                    let index = self
                        .ctx
                        .facts
                        .direct_call_parameters
                        .get(&call.span)
                        .and_then(|(_, indices)| indices.get(arg_index))
                        .copied()
                        .flatten();
                    previous = self.expr(value, previous);
                    let borrowed_callback = self.callback_loan(value).is_some();
                    if borrowed_callback && !is_constructor {
                        let proven = direct.as_ref().is_some_and(|(key, sig)| {
                            index.is_some_and(|i| {
                                sig.params
                                    .get(i)
                                    .is_some_and(|p| matches!(&p.ty, Type::Function(_, _)))
                                    && self.ctx.callback_summaries.get(key).is_some_and(|summary| {
                                        summary.non_escaping_parameters.contains(&i)
                                    })
                            })
                        });
                        if !proven {
                            self.error("borrow callback escape cannot be excluded without a non-escaping callee contract".into(), value.span());
                        }
                    }
                    let param = direct
                        .as_ref()
                        .and_then(|(_, sig)| index.and_then(|i| sig.params.get(i)));
                    let exclusive = call
                        .inout_arguments
                        .get(arg_index)
                        .copied()
                        .unwrap_or(false)
                        || param.is_some_and(|p| p.is_inout);
                    let slice = param.is_some_and(|p| matches!(&p.ty, Type::Slice(_)))
                        || index.is_some_and(|i| {
                            self.ctx.facts.slice_observer_parameters.get(&call.span) == Some(&i)
                        });
                    if let Some(loan) = self.argument_loan(value, exclusive, slice) {
                        if exclusive && (loan.aggregate || loan.closure) {
                            self.error("borrow provenance: inout of a borrowed aggregate or callback requires a verified transfer contract".into(), value.span());
                        }
                        if !exclusive
                            && !slice
                            && !loan.closure
                            && !is_constructor
                            && param.is_some()
                        {
                            self.error("borrow escape: borrowed argument requires a verified parameter access contract".into(), value.span());
                        }
                        if loan.aggregate && !is_constructor {
                            self.error("borrow aggregate escape: passing borrowed aggregates requires a verified non-escaping contract".into(), value.span());
                        }
                        if is_constructor && !loan.exclusive_owners.is_empty() {
                            self.error("borrow provenance: exclusive borrowed value cannot be copied into an aggregate".into(), value.span());
                        }
                        previous = self.acquire_argument(value, loan, previous, &mut held);
                    } else if exclusive {
                        self.error("borrow provenance unavailable: exclusive argument requires an identifiable owner".into(), value.span());
                    }
                }
                if direct.as_ref().is_some_and(|(key, signature)| {
                    matches!(&signature.return_type, Type::Slice(_))
                        && self.ctx.borrow_summaries.contains_key(key)
                }) && self.loan_of_call(call).is_none()
                {
                    self.error("borrow provenance unavailable: cannot identify the owner of returned Slice".into(), call.span);
                }
                self.finish_operands(previous, held)
            }
            HirExpression::Slice(s) => {
                previous = self.expr(&s.object, previous);
                let mut held = Vec::new();
                if let Some(loan) = self.loan_of(&s.object) {
                    let (next, id) = self.hold_loan(loan, previous);
                    previous = next;
                    held.push(id);
                }
                previous = self.expr(&s.start, previous);
                previous = self.expr(&s.end, previous);
                previous = self.finish_operands(previous, held);
                self.place_access(&s.object, previous, s.span, false)
            }
            HirExpression::Index(i) => {
                previous = self.expr(&i.object, previous);
                let mut held = Vec::new();
                if let Some(loan) = self.loan_of(&i.object) {
                    let (next, id) = self.hold_loan(loan, previous);
                    previous = next;
                    held.push(id);
                }
                previous = self.expr(&i.index, previous);
                self.finish_operands(previous, held)
            }
            HirExpression::Member(m) => self.expr(&m.object, previous),
            HirExpression::If(i) => {
                previous = self.expr(&i.condition, previous);
                let fork = self.noop(previous);
                self.scopes.push(HashMap::new());
                let first = self.bindings.len();
                self.bind_condition_pattern(&i.condition);
                let mut definition = Node::default();
                definition.defines.extend(first..self.bindings.len());
                let then_entry = self.node(fork.clone(), definition);
                let mut exits = self.block(&i.then_block, then_entry);
                if self
                    .block_loans
                    .get(&i.then_block.span)
                    .and_then(Option::as_ref)
                    .is_some_and(|loan| loan.owners.iter().any(|owner| *owner >= first))
                {
                    self.error(
                        "borrow escape: branch result refers to storage declared inside its branch"
                            .into(),
                        i.span,
                    );
                }
                self.scopes.pop();
                if let Some(else_branch) = &i.else_branch {
                    self.scopes.push(HashMap::new());
                    exits.extend(self.expr(else_branch, fork));
                    self.scopes.pop();
                } else {
                    exits.extend(fork);
                }
                exits
            }
            HirExpression::Match(m) => {
                previous = self.expr(&m.value, previous);
                let scrutinee_loan = self.loan_of(&m.value);
                let mut scrutinee_hold = Vec::new();
                if let Some(loan) = scrutinee_loan.clone() {
                    if !loan.exclusive_owners.is_empty() {
                        self.error("borrow provenance: matching an exclusive borrowed value requires a verified transfer contract".into(), m.span);
                    }
                    let (next, id) = self.hold_loan(loan, previous);
                    previous = next;
                    scrutinee_hold.push(id);
                }
                let mut candidates = self.noop(previous);
                let mut exits = Vec::new();
                for arm in &m.arms {
                    self.scopes.push(HashMap::new());
                    let first = self.bindings.len();
                    self.bind_pattern(&arm.pattern, scrutinee_loan.as_ref());
                    let mut definition = Node::default();
                    definition.defines.extend(first..self.bindings.len());
                    let mut path = self.node(candidates.clone(), definition);
                    if let Some(guard) = &arm.guard {
                        path = self.expr(guard, path);
                        // A failing guard may run later arms, not just the
                        // current arm. Preserve liveness on that edge.
                        candidates.extend(path.iter().copied());
                    }
                    path = self.finish_operands(path, scrutinee_hold.clone());
                    exits.extend(self.expr(&arm.value, path));
                    if self
                        .loan_of(&arm.value)
                        .is_some_and(|loan| loan.owners.iter().any(|owner| *owner >= first))
                    {
                        self.error(
                            "borrow escape: match result refers to storage declared inside its arm"
                                .into(),
                            arm.span,
                        );
                    }
                    self.scopes.pop();
                }
                if m.arms.is_empty() {
                    exits.extend(self.finish_operands(candidates, scrutinee_hold));
                }
                exits
            }
            HirExpression::Block(b) => self.block(b, previous),
            HirExpression::For(f) => self.loop_body(
                &f.binding,
                &f.iterable,
                f.filter.as_deref(),
                &f.body,
                previous,
            ),
            HirExpression::IsType(e) => self.expr(&e.value, previous),
            HirExpression::IsPattern(e) => self.expr(&e.value, previous),
            HirExpression::AsOptional(e) => self.expr(&e.expr, previous),
            HirExpression::AsForced(e) => self.expr(&e.expr, previous),
            HirExpression::Try(e) => self.expr(&e.expr, previous),
            HirExpression::Unwrap(e) => self.expr(&e.expr, previous),
            HirExpression::Tuple(_, values) | HirExpression::Array(_, values) => {
                self.eval_operands(values, previous)
            }
            HirExpression::StructLiteral(s) => {
                self.eval_operands(s.fields.iter().map(|(_, value)| value), previous)
            }
            HirExpression::EnumConstructor(e) => self.eval_operands(&e.payloads, previous),
            HirExpression::Closure(closure) => {
                for capture in &closure.captures {
                    if let HirCapture::Named(named) = capture {
                        let target = HirExpression::Name(named.span, named.name.clone());
                        previous = self.place_access(
                            &target,
                            previous,
                            named.span,
                            named.mode != HirCaptureMode::Ref,
                        );
                        if self.lookup(&named.name).is_some_and(|id| {
                            self.bindings[id]
                                .loan
                                .as_ref()
                                .is_some_and(|loan| !loan.exclusive_owners.is_empty())
                        }) {
                            self.error("borrow provenance: capturing an existing exclusive borrowed value requires a verified reborrow contract".into(), named.span);
                        }
                    }
                }
                self.check_closure_body(closure);
                previous
            }
            HirExpression::Comptime(e) => self.expr(&e.expression, previous),
            HirExpression::Integer(_, _)
            | HirExpression::Float(_, _)
            | HirExpression::String(_, _)
            | HirExpression::Bool(_, _) => previous,
        }
    }

    fn check_closure_body(&mut self, closure: &HirClosure) {
        let mut nested = Analyzer::new(self.ctx);
        for capture in &closure.captures {
            match capture {
                HirCapture::Named(named) => {
                    let Some(outer) = self.lookup(&named.name) else {
                        continue;
                    };
                    let source = &self.bindings[outer];
                    let id = nested.bind(&named.name, named.span, None);
                    if source.loan.is_some() || source.returnable_borrow_source {
                        nested.bindings[id].loan = Some(Loan {
                            owners: BTreeSet::from([id]),
                            origin: named.span,
                            closure: source.loan.as_ref().is_some_and(|loan| loan.closure),
                            aggregate: source.loan.as_ref().is_some_and(|loan| loan.aggregate),
                            exclusive_owners: BTreeSet::new(),
                        });
                    }
                }
                HirCapture::RecursiveSelf(span) => {
                    nested.bind("self", *span, None);
                }
            }
        }
        for p in &closure.parameters {
            let id = nested.bind(&p.name, p.span, None);
            nested.bindings[id].returnable_borrow_source = crate::borrow_summary::is_slice(&p.ty);
        }
        nested.returns_slice = closure
            .return_type
            .as_ref()
            .is_some_and(crate::borrow_summary::is_slice);
        let entry = nested.noop(Vec::new());
        nested.block_with_tail(&closure.body, entry, true);
        nested.solve();
        self.errors.extend(nested.errors);
        self.facts.extend(nested.facts);
    }

    fn bind_condition_pattern(&mut self, condition: &HirExpression) {
        match condition {
            HirExpression::IsPattern(pattern) => {
                let loan = self.loan_of(&pattern.value);
                if loan
                    .as_ref()
                    .is_some_and(|loan| !loan.exclusive_owners.is_empty())
                {
                    self.error("borrow provenance: pattern of exclusive borrowed value requires a verified transfer contract".into(), pattern.span);
                }
                self.bind_pattern(&pattern.pattern, loan.as_ref());
            }
            HirExpression::Binary(binary) if binary.op == HirBinaryOp::And => {
                // Binding refinements in a compound guard require separate
                // success/failure edges. Reject borrowed refinements until
                // that transfer can be represented precisely.
                if self.has_borrowed_pattern(&binary.left)
                    || self.has_borrowed_pattern(&binary.right)
                {
                    self.error("borrow provenance: compound borrowed pattern requires a verified flow contract".into(), binary.span);
                }
                self.bind_condition_pattern(&binary.left);
                self.bind_condition_pattern(&binary.right);
            }
            _ => {}
        }
    }

    fn has_borrowed_pattern(&mut self, expr: &HirExpression) -> bool {
        match expr {
            HirExpression::IsPattern(pattern) => self.loan_of(&pattern.value).is_some(),
            HirExpression::Binary(binary) => {
                self.has_borrowed_pattern(&binary.left) || self.has_borrowed_pattern(&binary.right)
            }
            _ => false,
        }
    }

    fn bind_pattern(&mut self, p: &HirPattern, loan: Option<&Loan>) {
        match p {
            HirPattern::Identifier(span, name) => {
                self.bind(name, *span, loan.cloned());
            }
            HirPattern::Tuple(_, patterns) | HirPattern::Or(_, patterns) => {
                for p in patterns {
                    self.bind_pattern(p, loan);
                }
            }
            HirPattern::EnumCase(_, _, patterns) => {
                for p in patterns {
                    self.bind_pattern(p, loan);
                }
            }
            HirPattern::EnumConstructor(e) => {
                for p in &e.payloads {
                    self.bind_pattern(p, loan);
                }
            }
            HirPattern::Struct(span, _, fields) => {
                for (name, pattern) in fields {
                    if let Some(p) = pattern {
                        self.bind_pattern(p, loan);
                    } else {
                        self.bind(name, *span, loan.cloned());
                    }
                }
            }
            HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
        }
    }

    fn loop_body(
        &mut self,
        binding: &str,
        iterable: &HirExpression,
        filter: Option<&HirExpression>,
        body: &HirBlock,
        previous: Vec<NodeId>,
    ) -> Vec<NodeId> {
        let mut previous = self.expr(iterable, previous);
        let iterable_loan = self.loan_of(iterable);
        let mut header_node = Node::default();
        if let Some(loan) = iterable_loan.clone() {
            let (next, id) = self.hold_loan(loan, previous);
            previous = next;
            // Keep a temporary (including an inline slice) live on backedges.
            header_node.uses.insert(id);
        }
        if let Some(id) = self.place_binding(iterable) {
            header_node.uses.insert(id);
        }
        let header = self.node(previous, header_node)[0];
        self.loops.push(LoopExit {
            header,
            breaks: Vec::new(),
        });
        self.scopes.push(HashMap::new());
        let borrowed_item = self
            .ctx
            .facts
            .expr_types
            .get(&iterable.span())
            .is_some_and(|ty| match ty {
                Type::Array(item) | Type::Slice(item) => !Self::plain_type(item),
                _ => true,
            });
        let item_loan = if borrowed_item { iterable_loan } else { None };
        let id = self.bind(binding, body.span, item_loan);
        let mut definition = Node::default();
        definition.defines.insert(id);
        let mut path = self.node(vec![header], definition);
        if let Some(filter) = filter {
            path = self.expr(filter, path);
            self.connect(&path, header);
        }
        let exits = self.block(body, path);
        self.scopes.pop();
        self.connect(&exits, header);
        let mut loop_exit = self.loops.pop().expect("loop scope");
        loop_exit.breaks.push(header);
        loop_exit.breaks
    }

    fn stmt(&mut self, stmt: &HirStatement, mut previous: Vec<NodeId>) -> Vec<NodeId> {
        match stmt {
            HirStatement::Let(l) => self.declaration(&l.name, l.span, &l.value, previous),
            HirStatement::Var(v) => self.declaration(&v.name, v.span, &v.value, previous),
            HirStatement::Return(value) => {
                if let Some(value) = value {
                    previous = self.expr(value, previous);
                    self.check_return_loan(value);
                }
                self.noop(previous);
                Vec::new()
            }
            HirStatement::Break(_) => {
                let exit = self.noop(previous);
                if let Some(loop_scope) = self.loops.last_mut() {
                    loop_scope.breaks.extend(exit);
                }
                Vec::new()
            }
            HirStatement::Continue(_) => {
                let exit = self.noop(previous);
                if let Some(header) = self.loops.last().map(|l| l.header) {
                    self.connect(&exit, header);
                }
                Vec::new()
            }
            HirStatement::For(f) => self.loop_body(
                &f.binding,
                &f.iterable,
                f.filter.as_ref(),
                &f.body,
                previous,
            ),
            HirStatement::Expr(e) => self.expr(e, previous),
        }
    }

    fn declaration(
        &mut self,
        name: &str,
        span: Span,
        value: &HirExpression,
        previous: Vec<NodeId>,
    ) -> Vec<NodeId> {
        let previous = self.expr(value, previous);
        let loan = self.loan_of(value);
        if loan
            .as_ref()
            .is_some_and(|loan| !loan.exclusive_owners.is_empty())
            && !matches!(value, HirExpression::Closure(_))
        {
            self.error("borrow provenance: copying an exclusive borrowed value requires a verified transfer contract".into(), span);
        }
        let id = self.bind(name, span, loan);
        let mut node = Node::default();
        node.defines.insert(id);
        self.node(previous, node)
    }

    fn block(&mut self, block: &HirBlock, previous: Vec<NodeId>) -> Vec<NodeId> {
        self.block_with_tail(block, previous, false)
    }

    fn block_with_tail(
        &mut self,
        block: &HirBlock,
        mut previous: Vec<NodeId>,
        return_tail: bool,
    ) -> Vec<NodeId> {
        let first_local = self.bindings.len();
        self.scopes.push(HashMap::new());
        for stmt in &block.statements {
            if previous.is_empty() {
                break;
            } // unreachable after return/break/continue
            previous = self.stmt(stmt, previous);
        }
        let loan = if previous.is_empty() {
            None
        } else {
            match block.statements.last() {
                Some(HirStatement::Expr(value)) => {
                    if return_tail {
                        self.check_return_loan(value);
                    }
                    self.loan_of(value)
                }
                _ => None,
            }
        };
        if !return_tail {
            if let Some(loan) = &loan {
                if loan.owners.iter().any(|owner| *owner >= first_local) {
                    self.error(
                        "borrow escape: block result refers to storage declared inside that block"
                            .into(),
                        block.span,
                    );
                }
            }
        }
        self.block_loans.insert(block.span, loan);
        self.scopes.pop();
        previous
    }

    fn solve(&mut self) {
        let len = self.nodes.len();
        let mut live_in = vec![BTreeSet::new(); len];
        let mut live_out = vec![BTreeSet::new(); len];
        loop {
            let mut changed = false;
            for id in (0..len).rev() {
                let mut out = BTreeSet::new();
                for &successor in &self.nodes[id].next {
                    out.extend(live_in[successor].iter().copied());
                }
                let mut input = self.nodes[id].uses.clone();
                input.extend(out.difference(&self.nodes[id].defines).copied());
                if live_out[id] != out || live_in[id] != input {
                    live_out[id] = out;
                    live_in[id] = input;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        for (id, node) in self.nodes.iter().enumerate() {
            for access in &node.accesses {
                for &borrower in &live_in[id] {
                    if access.via == Some(borrower) {
                        continue;
                    }
                    if self.bindings[borrower].loan.as_ref().is_some_and(|loan| {
                        loan.owners.contains(&access.owner)
                            && (access.exclusive || loan.exclusive_owners.contains(&access.owner))
                    }) {
                        let borrow = &self.bindings[borrower];
                        let action = if access.exclusive {
                            "mutate or exclusively borrow"
                        } else {
                            "read"
                        };
                        self.errors.push(TypeError {
                            message: format!("borrow conflict: cannot {action} '{}' while borrowed value '{}' remains live", self.bindings[access.owner].name, borrow.name),
                            span: access.span,
                            module_name: if self.ctx.current_module.is_empty() { None } else { Some(self.ctx.current_module.clone()) },
                        });
                        break;
                    }
                }
            }
        }
    }
}

/// Verify local borrowed views and closures in a typechecked HIR file.
/// Returns diagnostics and deterministic owner/borrow origin facts.
pub(crate) fn analyze(file: &HirFile, ctx: &TypeContext) -> (Vec<TypeError>, Vec<BorrowFact>) {
    let mut all_errors = Vec::new();
    let mut all_facts = Vec::new();
    for item in &file.items {
        let mut functions = Vec::new();
        match item {
            HirItem::Function(f) => functions.push(f),
            HirItem::Extend(e) => functions.extend(e.methods.iter()),
            _ => {}
        }
        for function in functions {
            let Some(body) = &function.body else { continue };
            let mut analyzer = Analyzer::new(ctx);
            for p in &function.parameters {
                // Parameter origins need interprocedural BorrowSummary
                // propagation; parameter bindings are still unique owners here.
                let id = analyzer.bind(&p.name, p.span, None);
                analyzer.bindings[id].returnable_borrow_source = matches!(
                    &p.ty,
                    HirType::Apply(_, callee, _) if matches!(&**callee, HirType::Named(_, name) if name == "Slice")
                );
            }
            let qualified = if ctx.current_module.is_empty() {
                function.name.clone()
            } else {
                format!("{}.{}", ctx.current_module, function.name)
            };
            analyzer.returns_slice = function
                .return_type
                .as_ref()
                .is_some_and(crate::borrow_summary::is_slice);
            if let Some(summary) = ctx.borrow_summaries.get(&qualified) {
                analyzer.return_source = analyzer.scopes[0]
                    .get(&function.parameters[summary.return_borrow.parameter_index].name)
                    .copied();
            }
            let entry = analyzer.noop(Vec::new());
            analyzer.block_with_tail(body, entry, true);
            analyzer.solve();
            all_errors.extend(analyzer.errors);
            all_facts.extend(analyzer.facts);
        }
    }
    if !file.top_level_statements.is_empty() {
        let mut analyzer = Analyzer::new(ctx);
        let entry = analyzer.noop(Vec::new());
        analyzer.scopes.push(HashMap::new());
        let mut previous = entry;
        for stmt in &file.top_level_statements {
            previous = match stmt {
                HirStatement::Let(l) => analyzer.declaration(&l.name, l.span, &l.value, previous),
                HirStatement::Var(v) => analyzer.declaration(&v.name, v.span, &v.value, previous),
                _ => analyzer.stmt(stmt, previous),
            };
        }
        analyzer.solve();
        all_errors.extend(analyzer.errors);
        all_facts.extend(analyzer.facts);
    }
    (all_errors, all_facts)
}
