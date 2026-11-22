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
}

#[derive(Default)]
struct Node {
    next: Vec<NodeId>,
    uses: BTreeSet<BindingId>,
    defines: BTreeSet<BindingId>,
    mutation: Option<(BindingId, Span)>,
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

    fn use_name(&mut self, name: &str, previous: Vec<NodeId>) -> Vec<NodeId> {
        let Some(id) = self.lookup(name) else {
            return previous;
        };
        let mut node = Node::default();
        node.uses.insert(id);
        self.node(previous, node)
    }

    fn source_owner(&self, expr: &HirExpression) -> Option<BindingId> {
        match expr {
            HirExpression::Name(_, name) => self.lookup(name).map(|id| {
                self.bindings[id]
                    .loan
                    .as_ref()
                    .and_then(|loan| loan.owners.iter().next().copied())
                    .unwrap_or(id)
            }),
            HirExpression::Slice(slice) => self.source_owner(&slice.object),
            HirExpression::Member(member) => self.source_owner(&member.object),
            _ => None,
        }
    }

    // Only direct, statically identifiable views/closures are classified here.
    // Unknown call results must be handled by a future BorrowSummary contract.
    fn loan_of(&mut self, expr: &HirExpression) -> Option<Loan> {
        match expr {
            HirExpression::Name(span, name) => self.lookup(name).and_then(|id| {
                self.bindings[id].loan.clone().or_else(|| {
                    self.bindings[id].returnable_borrow_source.then(|| Loan {
                        owners: BTreeSet::from([id]),
                        origin: *span,
                        closure: false,
                    })
                })
            }),
            HirExpression::Slice(slice) => {
                let owner = self.source_owner(&slice.object)?;
                Some(Loan {
                    owners: BTreeSet::from([owner]),
                    origin: slice.span,
                    closure: false,
                })
            }
            HirExpression::Call(call) => self.loan_of_call(call),
            HirExpression::Closure(closure) => {
                let owners = closure
                    .captures
                    .iter()
                    .filter_map(|capture| {
                        if let HirCapture::Named(named) = capture {
                            if matches!(named.mode, HirCaptureMode::Ref | HirCaptureMode::Inout) {
                                return self.lookup(&named.name);
                            }
                        }
                        None
                    })
                    .flat_map(|id| {
                        self.bindings[id]
                            .loan
                            .as_ref()
                            .map(|loan| loan.owners.iter().copied().collect::<Vec<_>>())
                            .unwrap_or_else(|| vec![id])
                    })
                    .collect::<BTreeSet<_>>();
                if owners.is_empty() {
                    None
                } else {
                    Some(Loan {
                        owners,
                        origin: closure.span,
                        closure: true,
                    })
                }
            }
            HirExpression::If(branch) => {
                let then_loan = branch.then_block.statements.last().and_then(|stmt| {
                    if let HirStatement::Expr(expr) = stmt {
                        self.loan_of(expr)
                    } else {
                        None
                    }
                });
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
            HirExpression::Block(b) => b.statements.last().and_then(|stmt| {
                if let HirStatement::Expr(expr) = stmt {
                    self.loan_of(expr)
                } else {
                    None
                }
            }),
            _ => None,
        }
    }

    fn loan_of_call(&mut self, call: &HirCall) -> Option<Loan> {
        let HirExpression::Name(_, name) = &*call.callee else {
            return None;
        };
        let module = self.ctx.current_module.as_str();
        let qualified = if module.is_empty() {
            name.clone()
        } else {
            format!("{module}.{name}")
        };
        let key = if self.ctx.borrow_summaries.contains_key(&qualified) {
            qualified
        } else {
            name.clone()
        };
        let summary = self.ctx.borrow_summaries.get(&key)?;
        let sig = self.ctx.functions.get(&key)?;
        let names = sig
            .params
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        let kinds = sig.params.iter().map(|p| p.kind).collect::<Vec<_>>();
        let argument = crate::borrow_summary::call_argument(
            call,
            summary.return_borrow.parameter_index,
            &names,
            &kinds,
        )?;
        let mut loan = self.loan_of(argument)?;
        loan.origin = call.span;
        Some(loan)
    }

    fn merge_loans(&mut self, left: Option<Loan>, right: Option<Loan>, span: Span) -> Option<Loan> {
        match (left, right) {
            (Some(a), Some(b)) if a.owners == b.owners => Some(a),
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
                if loan.closure || !self.bindings[owner].returnable_borrow_source {
                    self.error(format!("borrow escape: cannot return a view or closure borrowed from local owner '{name}'"), expr.span());
                } else if self.return_source != Some(owner) {
                    // A return is valid only when *every* supported return
                    // path was proven to derive from this exact parameter.
                    self.error(format!("borrow return contract unavailable: returning a view of parameter '{name}' requires a propagated BorrowSummary"), expr.span());
                }
            }
        }
    }

    fn mutation(
        &mut self,
        target: &HirExpression,
        previous: Vec<NodeId>,
        span: Span,
    ) -> Vec<NodeId> {
        let Some(owner) = self.source_owner(target) else {
            return previous;
        };
        let mut node = Node::default();
        node.mutation = Some((owner, span));
        self.node(previous, node)
    }

    fn call_accesses(&mut self, call: &HirCall) -> Vec<(BindingId, bool, Span)> {
        let HirExpression::Name(_, callee) = &*call.callee else {
            return Vec::new();
        };
        let qualified = format!("{}.{}", self.ctx.current_module, callee);
        let sig: FunctionSignature = match self
            .ctx
            .functions
            .get(&qualified)
            .or_else(|| self.ctx.functions.get(callee))
        {
            Some(sig) => sig.clone(),
            None => return Vec::new(),
        };
        let mut positional = 0usize;
        let mut accesses = Vec::<(BindingId, bool, Span)>::new();
        for arg in &call.args {
            let (index, expr) = match arg {
                HirCallArgument::Positional(expr) => {
                    while sig
                        .params
                        .get(positional)
                        .is_some_and(|p| p.kind == ParameterKind::Type)
                    {
                        positional += 1;
                    }
                    let index = positional;
                    positional += 1;
                    (index, expr)
                }
                HirCallArgument::Named(named) => {
                    let Some(index) = sig.params.iter().position(|p| p.name == named.name) else {
                        continue;
                    };
                    (index, &named.value)
                }
            };
            let Some(param) = sig.params.get(index) else {
                continue;
            };
            // Only borrowed Slice access participates in this owner-level check.
            if !matches!(&param.ty, Type::Slice(_)) {
                continue;
            }
            let Some(owner) = self.source_owner(expr) else {
                continue;
            };
            if accesses
                .iter()
                .any(|(other, exclusive, _)| *other == owner && (*exclusive || param.is_inout))
            {
                self.error(
                    format!(
                        "borrow alias: conflicting simultaneous accesses to '{}'",
                        self.bindings[owner].name
                    ),
                    expr.span(),
                );
            }
            accesses.push((owner, param.is_inout, expr.span()));
        }
        accesses
    }

    fn expr(&mut self, expr: &HirExpression, mut previous: Vec<NodeId>) -> Vec<NodeId> {
        match expr {
            HirExpression::Name(_, name) => self.use_name(name, previous),
            HirExpression::Unary(e) => self.expr(&e.expr, previous),
            HirExpression::Binary(e) if e.op == HirBinaryOp::Assign => {
                previous = self.expr(&e.right, previous);
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
                // A user-defined function returning Slice must publish a
                // certified return owner before its result can be used.
                // Unknown/dynamic targets are not silently promoted.
                if let HirExpression::Name(_, name) = &*call.callee {
                    let qualified = if self.ctx.current_module.is_empty() {
                        name.clone()
                    } else {
                        format!("{}.{}", self.ctx.current_module, name)
                    };
                    let key = if self.ctx.functions.contains_key(&qualified) {
                        &qualified
                    } else {
                        name
                    };
                    if self
                        .ctx
                        .functions
                        .get(key)
                        .is_some_and(|signature| matches!(&signature.return_type, Type::Slice(_)))
                    {
                        if !self.ctx.borrow_summaries.contains_key(key) {
                            self.error(format!("borrow return contract unavailable: function '{name}' returns a Slice without a proven single-owner BorrowSummary"), call.span);
                        } else if self.loan_of_call(call).is_none() {
                            self.error(format!("borrow provenance unavailable: cannot identify the owner of returned Slice from '{name}'"), call.span);
                        }
                    }
                }
                previous = self.expr(&call.callee, previous);
                for arg in &call.args {
                    let value = match arg {
                        HirCallArgument::Positional(value) => value,
                        HirCallArgument::Named(named) => &named.value,
                    };
                    // In the absence of proven non-escape metadata, a
                    // borrowed closure cannot be forwarded to a callee.
                    let borrowed_callback = match value {
                        HirExpression::Name(_, name) => self.lookup(name)
                            .and_then(|id| self.bindings[id].loan.as_ref())
                            .is_some_and(|loan| loan.closure),
                        HirExpression::Closure(closure) => closure.captures.iter().any(|c| {
                            matches!(c, HirCapture::Named(n) if matches!(n.mode, HirCaptureMode::Ref | HirCaptureMode::Inout))
                        }),
                        _ => false,
                    };
                    if borrowed_callback {
                        self.error("borrow callback escape cannot be excluded without a non-escaping callee contract".into(), value.span());
                    }
                    previous = self.expr(value, previous);
                }
                for (owner, exclusive, span) in self.call_accesses(call) {
                    if exclusive {
                        let mut node = Node::default();
                        node.mutation = Some((owner, span));
                        previous = self.node(previous, node);
                    }
                }
                previous
            }
            HirExpression::Slice(s) => {
                previous = self.expr(&s.object, previous);
                previous = self.expr(&s.start, previous);
                self.expr(&s.end, previous)
            }
            HirExpression::Index(i) => {
                previous = self.expr(&i.object, previous);
                self.expr(&i.index, previous)
            }
            HirExpression::Member(m) => self.expr(&m.object, previous),
            HirExpression::If(i) => {
                previous = self.expr(&i.condition, previous);
                let fork = self.noop(previous);
                self.scopes.push(HashMap::new());
                let mut exits = self.block(&i.then_block, fork.clone());
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
                let fork = self.noop(previous);
                let mut exits = Vec::new();
                for arm in &m.arms {
                    self.scopes.push(HashMap::new());
                    self.bind_pattern(&arm.pattern);
                    let mut path = fork.clone();
                    if let Some(guard) = &arm.guard {
                        path = self.expr(guard, path);
                    }
                    exits.extend(self.expr(&arm.value, path));
                    self.scopes.pop();
                }
                if m.arms.is_empty() {
                    exits.extend(fork);
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
                for value in values {
                    previous = self.expr(value, previous);
                }
                previous
            }
            HirExpression::StructLiteral(s) => {
                for (_, value) in &s.fields {
                    previous = self.expr(value, previous);
                }
                previous
            }
            HirExpression::EnumConstructor(e) => {
                for value in &e.payloads {
                    previous = self.expr(value, previous);
                }
                previous
            }
            HirExpression::Closure(closure) => {
                for capture in &closure.captures {
                    if let HirCapture::Named(named) = capture {
                        if named.mode == HirCaptureMode::Consume {
                            previous = self.mutation(
                                &HirExpression::Name(named.span, named.name.clone()),
                                previous,
                                named.span,
                            );
                        }
                    }
                }
                previous
            }
            HirExpression::Comptime(e) => self.expr(&e.expression, previous),
            HirExpression::Integer(_, _)
            | HirExpression::Float(_, _)
            | HirExpression::String(_, _)
            | HirExpression::Bool(_, _) => previous,
        }
    }

    fn bind_pattern(&mut self, p: &HirPattern) {
        match p {
            HirPattern::Identifier(span, name) => {
                self.bind(name, *span, None);
            }
            HirPattern::Tuple(_, patterns) | HirPattern::Or(_, patterns) => {
                for p in patterns {
                    self.bind_pattern(p);
                }
            }
            HirPattern::EnumCase(_, _, patterns) => {
                for p in patterns {
                    self.bind_pattern(p);
                }
            }
            HirPattern::EnumConstructor(e) => {
                for p in &e.payloads {
                    self.bind_pattern(p);
                }
            }
            HirPattern::Struct(_, _, fields) => {
                for (_, pattern) in fields {
                    if let Some(p) = pattern {
                        self.bind_pattern(p);
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
        let previous = self.expr(iterable, previous);
        let header = self.noop(previous)[0];
        // Iteration keeps a borrowed iterable alive until the loop exits.
        if let HirExpression::Name(_, name) = iterable {
            if let Some(id) = self.lookup(name) {
                self.nodes[header].uses.insert(id);
            }
        }
        self.loops.push(LoopExit {
            header,
            breaks: Vec::new(),
        });
        self.scopes.push(HashMap::new());
        self.bind(binding, body.span, None);
        let mut path = vec![header];
        if let Some(filter) = filter {
            path = self.expr(filter, path);
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
        self.scopes.push(HashMap::new());
        for stmt in &block.statements {
            if previous.is_empty() {
                break;
            } // unreachable after return/break/continue
            previous = self.stmt(stmt, previous);
        }
        if return_tail {
            if let Some(HirStatement::Expr(value)) = block.statements.last() {
                self.check_return_loan(value);
            }
        }
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
            let Some((owner, span)) = node.mutation else {
                continue;
            };
            for &borrower in &live_out[id] {
                if self.bindings[borrower]
                    .loan
                    .as_ref()
                    .is_some_and(|loan| loan.owners.contains(&owner))
                {
                    let borrow = &self.bindings[borrower];
                    self.errors.push(TypeError {
                        message: format!("borrow conflict: cannot mutate '{}' while borrowed value '{}' remains live", self.bindings[owner].name, borrow.name),
                        span,
                        module_name: if self.ctx.current_module.is_empty() { None } else { Some(self.ctx.current_module.clone()) },
                    });
                    break;
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
