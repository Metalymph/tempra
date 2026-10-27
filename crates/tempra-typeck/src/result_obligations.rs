//! Static Result provenance and obligation tracking.
//!
//! This module does not implement ownership or move semantics.
//! Bindings may freely share an origin while retaining value semantics.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResultOriginId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObligationId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BindingId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObligationState {
    Pending,
    Handled,
    Propagated,
}

#[derive(Debug, Clone)]
pub struct ResultObligation {
    pub id: ObligationId,
    pub origin: ResultOriginId,
    pub state: ObligationState,
}

#[derive(Debug, Default)]
pub struct ResultObligationTracker {
    obligations: Vec<ResultObligation>,
    bindings: HashMap<BindingId, ResultOriginId>,
}

impl ResultObligationTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn produce(&mut self) -> ResultOriginId {
        let index = self.obligations.len();

        let origin = ResultOriginId(index);
        let obligation = ObligationId(index);

        self.obligations.push(ResultObligation {
            id: obligation,
            origin,
            state: ObligationState::Pending,
        });

        origin
    }

    pub fn bind(&mut self, binding: BindingId, origin: ResultOriginId) {
        self.bindings.insert(binding, origin);
    }

    pub fn origin_of(&self, binding: BindingId) -> Option<ResultOriginId> {
        self.bindings.get(&binding).copied()
    }

    pub fn copy(&mut self, source: BindingId, destination: BindingId) -> bool {
        let Some(origin) = self.origin_of(source) else {
            return false;
        };

        self.bind(destination, origin);
        true
    }

    pub fn overwrite(&mut self, binding: BindingId, origin: ResultOriginId) {
        self.bind(binding, origin);
    }

    pub fn handle(&mut self, origin: ResultOriginId) {
        self.transition(origin, ObligationState::Handled);
    }

    pub fn propagate(&mut self, origin: ResultOriginId) {
        self.transition(origin, ObligationState::Propagated);
    }

    fn transition(&mut self, origin: ResultOriginId, state: ObligationState) {
        if let Some(obligation) = self.obligations.get_mut(origin.0) {
            if obligation.state == ObligationState::Pending {
                obligation.state = state;
            }
        }
    }

    pub fn pending(&self) -> impl Iterator<Item = &ResultObligation> {
        self.obligations
            .iter()
            .filter(|obligation| obligation.state == ObligationState::Pending)
    }
}

/// Lexical environment for Result provenance.
///
/// Binding identities are independent of variable names.
/// Shadowing creates a new identity rather than replacing the old one.
#[derive(Debug, Default)]
pub struct ResultBindingEnvironment {
    scopes: Vec<HashMap<String, BindingId>>,
    next_binding: usize,
}

impl ResultBindingEnvironment {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_binding: 0,
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        assert!(
            self.scopes.len() > 1,
            "cannot exit the root provenance scope"
        );
        self.scopes.pop();
    }

    pub fn declare(&mut self, name: &str) -> BindingId {
        let id = BindingId(self.next_binding);
        self.next_binding += 1;

        self.scopes
            .last_mut()
            .expect("provenance environment has a root scope")
            .insert(name.to_string(), id);

        id
    }

    pub fn resolve(&self, name: &str) -> Option<BindingId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

#[cfg(test)]
mod binding_environment_tests {
    use super::*;

    #[test]
    fn shadowing_creates_distinct_binding_identity() {
        let mut env = ResultBindingEnvironment::new();

        let outer = env.declare("result");

        env.enter_scope();
        let inner = env.declare("result");

        assert_ne!(outer, inner);
        assert_eq!(env.resolve("result"), Some(inner));

        env.exit_scope();

        assert_eq!(env.resolve("result"), Some(outer));
    }

    #[test]
    fn nested_scope_resolves_outer_binding() {
        let mut env = ResultBindingEnvironment::new();

        let original = env.declare("value");

        env.enter_scope();

        assert_eq!(env.resolve("value"), Some(original));

        env.exit_scope();
    }

    #[test]
    fn independent_declarations_have_distinct_identities() {
        let mut env = ResultBindingEnvironment::new();

        let first = env.declare("first");
        let second = env.declare("second");

        assert_ne!(first, second);
    }
}

/// Diagnostic emitted for a locally produced Result that remains unhandled.
pub const UNHANDLED_RESULT_DIAGNOSTIC: &str = "Result is not handled or propagated";

use tempra_hir::{
    HirBinaryOp, HirBlock, HirCallArgument, HirExpression, HirFile, HirItem, HirStatement,
};

#[derive(Debug, Clone)]
enum ResultReturnProvenance {
    ForwardParameter { index: usize, name: String },
}

fn local_result_forwarding_contracts(
    file: &HirFile,
    facts: &crate::facts::TypecheckFacts,
    function_spans: &HashMap<String, tempra_span::Span>,
    current_module: &str,
) -> HashMap<tempra_span::Span, ResultReturnProvenance> {
    use std::collections::HashSet;

    let mut contracts = HashMap::new();
    let mut seen = HashSet::new();
    let mut ambiguous = HashSet::new();

    for item in &file.items {
        if let HirItem::Function(function) = item {
            if !seen.insert(function.name.clone()) {
                ambiguous.insert(function.name.clone());
            }
        }
    }

    // Seed: functions returning exactly one of their parameters.
    for item in &file.items {
        let HirItem::Function(function) = item else {
            continue;
        };

        if ambiguous.contains(&function.name) {
            continue;
        }

        let Some(body) = &function.body else {
            continue;
        };

        let [HirStatement::Return(Some(HirExpression::Name(_, returned_name)))] =
            body.statements.as_slice()
        else {
            continue;
        };

        let Some(index) = function
            .parameters
            .iter()
            .filter(|parameter| !matches!(parameter.kind, tempra_ast::ParameterKind::Type))
            .position(|parameter| parameter.name == *returned_name)
        else {
            continue;
        };

        let qualified_name = if current_module.is_empty() {
            function.name.clone()
        } else {
            format!("{}.{}", current_module, function.name)
        };

        let Some(&definition_span) = function_spans.get(&qualified_name) else {
            continue;
        };

        contracts.insert(
            definition_span,
            ResultReturnProvenance::ForwardParameter {
                index,
                name: returned_name.clone(),
            },
        );
    }

    // Fixed point: discover wrappers of already-proven forwarding functions.
    // A cycle without a direct forwarding seed cannot introduce a contract.
    loop {
        let mut changed = false;

        for item in &file.items {
            let HirItem::Function(function) = item else {
                continue;
            };

            if ambiguous.contains(&function.name) {
                continue;
            }

            let qualified_name = if current_module.is_empty() {
                function.name.clone()
            } else {
                format!("{}.{}", current_module, function.name)
            };

            let Some(&definition_span) = function_spans.get(&qualified_name) else {
                continue;
            };

            if contracts.contains_key(&definition_span) {
                continue;
            }

            let Some(body) = &function.body else {
                continue;
            };

            let [HirStatement::Return(Some(HirExpression::Call(call)))] =
                body.statements.as_slice()
            else {
                continue;
            };

            let HirExpression::Name(callee_span, _) = call.callee.as_ref() else {
                continue;
            };

            let Some(callee_definition) = facts.resolved_symbols.get(callee_span) else {
                continue;
            };

            let Some(ResultReturnProvenance::ForwardParameter {
                index: callee_index,
                name: callee_name,
            }) = contracts.get(callee_definition)
            else {
                continue;
            };

            let forwarded_arg =
                if let Some(HirCallArgument::Positional(expr)) = call.args.get(*callee_index) {
                    Some(expr)
                } else {
                    call.args.iter().find_map(|arg| match arg {
                        HirCallArgument::Named(named) if named.name == *callee_name => {
                            Some(&named.value)
                        }
                        _ => None,
                    })
                };

            let Some(HirExpression::Name(_, argument_name)) = forwarded_arg else {
                continue;
            };

            let Some((index, parameter)) = function
                .parameters
                .iter()
                .filter(|parameter| !matches!(parameter.kind, tempra_ast::ParameterKind::Type))
                .enumerate()
                .find(|(_, parameter)| parameter.name == *argument_name)
            else {
                continue;
            };

            contracts.insert(
                definition_span,
                ResultReturnProvenance::ForwardParameter {
                    index,
                    name: parameter.name.clone(),
                },
            );
            changed = true;
        }

        if !changed {
            break;
        }
    }

    contracts
}

struct ResultProvenanceAnalyzer<'a> {
    facts: &'a crate::facts::TypecheckFacts,
    forwarding_contracts: &'a HashMap<tempra_span::Span, ResultReturnProvenance>,
    tracker: ResultObligationTracker,
    bindings: ResultBindingEnvironment,
    origin_spans: Vec<tempra_span::Span>,
}

impl<'a> ResultProvenanceAnalyzer<'a> {
    fn new(
        facts: &'a crate::facts::TypecheckFacts,
        forwarding_contracts: &'a HashMap<tempra_span::Span, ResultReturnProvenance>,
    ) -> Self {
        Self {
            facts,
            forwarding_contracts,
            tracker: ResultObligationTracker::new(),
            bindings: ResultBindingEnvironment::new(),
            origin_spans: Vec::new(),
        }
    }

    fn is_result(&self, expr: &HirExpression) -> bool {
        matches!(
            self.facts.expr_types.get(&expr.span()),
            Some(crate::types::Type::Result(_, _))
        )
    }

    fn produce(&mut self, expr: &HirExpression) -> ResultOriginId {
        let origin = self.tracker.produce();
        debug_assert_eq!(origin.0, self.origin_spans.len());
        self.origin_spans.push(expr.span());
        origin
    }

    fn analyze_block(&mut self, block: &HirBlock, is_function_body: bool) {
        self.bindings.enter_scope();

        let last_index = block.statements.len().checked_sub(1);

        for (index, statement) in block.statements.iter().enumerate() {
            if is_function_body && Some(index) == last_index {
                if let HirStatement::Expr(expr) = statement {
                    if let Some(origin) = self.analyze_expression(expr) {
                        self.tracker.propagate(origin);
                    }
                    continue;
                }
            }

            self.analyze_statement(statement);
        }

        self.bindings.exit_scope();
    }

    fn analyze_statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::Let(binding) => {
                let origin = self.analyze_expression(&binding.value);
                let id = self.bindings.declare(&binding.name);

                if let Some(origin) = origin {
                    self.tracker.bind(id, origin);
                }
            }

            HirStatement::Var(binding) => {
                let origin = self.analyze_expression(&binding.value);
                let id = self.bindings.declare(&binding.name);

                if let Some(origin) = origin {
                    self.tracker.bind(id, origin);
                }
            }

            HirStatement::Return(Some(expr)) => {
                if let Some(origin) = self.analyze_expression(expr) {
                    self.tracker.propagate(origin);
                }
            }

            HirStatement::Return(None) => {}

            HirStatement::Expr(expr) => {
                self.analyze_expression(expr);
            }

            // Control-flow analysis belongs to R3.
            HirStatement::For(_) | HirStatement::Break(_) | HirStatement::Continue(_) => {}
        }
    }

    fn analyze_expression(&mut self, expr: &HirExpression) -> Option<ResultOriginId> {
        match expr {
            HirExpression::Name(_, name) => self
                .bindings
                .resolve(name)
                .and_then(|id| self.tracker.origin_of(id)),

            HirExpression::Call(call) => {
                let mut argument_origins = Vec::with_capacity(call.args.len());

                for arg in &call.args {
                    let origin = match arg {
                        HirCallArgument::Positional(value) => self.analyze_expression(value),
                        HirCallArgument::Named(named) => self.analyze_expression(&named.value),
                    };

                    argument_origins.push(origin);
                }

                if !self.is_result(expr) {
                    return None;
                }

                // R2.4a.7: map the forwarded formal parameter to its
                // positional or named call argument.
                if let HirExpression::Name(callee_span, _) = call.callee.as_ref() {
                    if let Some(definition_span) = self.facts.resolved_symbols.get(callee_span) {
                        if let Some(ResultReturnProvenance::ForwardParameter { index, name }) =
                            self.forwarding_contracts.get(definition_span)
                        {
                            // Match the type checker's get_call_arg_mut rule.
                            let argument_index = if matches!(
                                call.args.get(*index),
                                Some(HirCallArgument::Positional(_))
                            ) {
                                Some(*index)
                            } else {
                                call.args.iter().position(|arg| {
                                    matches!(
                                        arg,
                                        HirCallArgument::Named(named)
                                            if named.name == *name
                                    )
                                })
                            };

                            if let Some(Some(origin)) =
                                argument_index.and_then(|i| argument_origins.get(i))
                            {
                                return Some(*origin);
                            }
                        }
                    }
                }

                Some(self.produce(expr))
            }

            HirExpression::Try(try_expr) => {
                if let Some(origin) = self.analyze_expression(&try_expr.expr) {
                    self.tracker.propagate(origin);
                }

                None
            }

            HirExpression::Match(match_expr) => {
                if let Some(origin) = self.analyze_expression(&match_expr.value) {
                    self.tracker.handle(origin);
                }

                // R2: analyze arm expressions for independent obligations.
                // Full path-sensitive provenance merging is deferred to R3.
                for arm in &match_expr.arms {
                    if let Some(guard) = &arm.guard {
                        self.analyze_expression(guard);
                    }

                    self.analyze_expression(&arm.value);
                }

                None
            }

            HirExpression::Binary(binary) if binary.op == HirBinaryOp::Assign => {
                let new_origin = self.analyze_expression(&binary.right);

                if let HirExpression::Name(_, name) = binary.left.as_ref() {
                    if let Some(binding) = self.bindings.resolve(name) {
                        if let Some(origin) = new_origin {
                            self.tracker.overwrite(binding, origin);
                        }
                    }
                }

                // Assignment is not a Result-valued expression.
                // The produced obligation remains tracked independently.
                None
            }

            HirExpression::Block(block) => {
                self.analyze_block(block, false);
                None
            }

            HirExpression::Tuple(_, values) | HirExpression::Array(_, values) => {
                for value in values {
                    self.analyze_expression(value);
                }
                None
            }

            HirExpression::Binary(binary) => {
                self.analyze_expression(&binary.left);
                self.analyze_expression(&binary.right);
                None
            }

            HirExpression::Integer(_, _)
            | HirExpression::Float(_, _)
            | HirExpression::String(_, _)
            | HirExpression::Bool(_, _) => None,

            // Remaining expression families are handled incrementally.
            _ => None,
        }
    }

    fn diagnostics(&self) -> Vec<crate::error::TypeError> {
        self.tracker
            .pending()
            .map(|obligation| {
                crate::error::TypeError::new(
                    UNHANDLED_RESULT_DIAGNOSTIC,
                    self.origin_spans[obligation.origin.0],
                )
            })
            .collect()
    }
}

pub fn analyze_linear_result_obligations(
    file: &HirFile,
    facts: &crate::facts::TypecheckFacts,
    function_spans: &HashMap<String, tempra_span::Span>,
    current_module: &str,
) -> Vec<crate::error::TypeError> {
    let mut diagnostics = Vec::new();
    let forwarding_contracts =
        local_result_forwarding_contracts(file, facts, function_spans, current_module);

    for item in &file.items {
        if let HirItem::Function(function) = item {
            if let Some(body) = &function.body {
                let mut analyzer = ResultProvenanceAnalyzer::new(facts, &forwarding_contracts);
                analyzer.analyze_block(body, true);
                diagnostics.extend(analyzer.diagnostics());
            }
        }
    }

    if !file.top_level_statements.is_empty() {
        let mut analyzer = ResultProvenanceAnalyzer::new(facts, &forwarding_contracts);

        for statement in &file.top_level_statements {
            analyzer.analyze_statement(statement);
        }

        diagnostics.extend(analyzer.diagnostics());
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_preserves_origin_without_consuming_source() {
        let mut tracker = ResultObligationTracker::new();

        let origin = tracker.produce();

        tracker.bind(BindingId(0), origin);

        assert!(tracker.copy(BindingId(0), BindingId(1)));

        assert_eq!(tracker.origin_of(BindingId(0)), Some(origin));
        assert_eq!(tracker.origin_of(BindingId(1)), Some(origin));

        assert_eq!(tracker.pending().count(), 1);
    }

    #[test]
    fn handling_copy_satisfies_shared_obligation() {
        let mut tracker = ResultObligationTracker::new();

        let origin = tracker.produce();

        tracker.bind(BindingId(0), origin);
        tracker.copy(BindingId(0), BindingId(1));

        tracker.handle(tracker.origin_of(BindingId(1)).unwrap());

        assert_eq!(tracker.pending().count(), 0);

        // Both values remain available.
        assert_eq!(tracker.origin_of(BindingId(0)), Some(origin));
        assert_eq!(tracker.origin_of(BindingId(1)), Some(origin));
    }

    #[test]
    fn independent_productions_create_independent_obligations() {
        let mut tracker = ResultObligationTracker::new();

        let first = tracker.produce();
        let second = tracker.produce();

        tracker.handle(first);

        let pending: Vec<_> = tracker.pending().collect();

        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].origin, second);
    }

    #[test]
    fn overwrite_does_not_erase_previous_obligation() {
        let mut tracker = ResultObligationTracker::new();

        let first = tracker.produce();
        tracker.bind(BindingId(0), first);

        let second = tracker.produce();
        tracker.overwrite(BindingId(0), second);

        assert_eq!(tracker.origin_of(BindingId(0)), Some(second));
        assert_eq!(tracker.pending().count(), 2);
    }

    #[test]
    fn propagation_does_not_consume_binding() {
        let mut tracker = ResultObligationTracker::new();

        let origin = tracker.produce();
        tracker.bind(BindingId(0), origin);

        tracker.propagate(origin);

        assert_eq!(tracker.pending().count(), 0);
        assert_eq!(tracker.origin_of(BindingId(0)), Some(origin));
    }
}
