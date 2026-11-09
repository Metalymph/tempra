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
    NotProduced,
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
#[derive(Debug, Default, Clone)]
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
    HirBinaryOp, HirBlock, HirCallArgument, HirExpression, HirFile, HirItem, HirPattern,
    HirStatement, HirType,
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

#[derive(Debug, Clone)]
struct ResultConsumedParameter {
    index: usize,
    name: String,
}

fn result_pattern_variant(pattern: &HirPattern) -> Option<&str> {
    match pattern {
        HirPattern::EnumCase(_, name, _) => Some(name.as_str()),
        HirPattern::EnumConstructor(pattern) => Some(pattern.variant_name.as_str()),
        _ => None,
    }
}

/// R2.5a: infer consumption only for a direct, unconditional,
/// exhaustive Ok/Err match on a Result parameter.
///
/// More complex control flow and transitive contracts belong to later phases.

// R2.5c: multiparameter consumption contracts.
//
// Only unconditional linear sequences of direct calls and exhaustive
// Result matches are supported. Branch-sensitive analysis belongs to R3.

fn result_parameter(
    function: &tempra_hir::HirFunction,
    name: &str,
) -> Option<ResultConsumedParameter> {
    function
        .parameters
        .iter()
        .filter(|parameter| !matches!(parameter.kind, tempra_ast::ParameterKind::Type))
        .enumerate()
        .find_map(|(index, parameter)| {
            (parameter.name == name && matches!(&parameter.ty, HirType::Result(..))).then(|| {
                ResultConsumedParameter {
                    index,
                    name: parameter.name.clone(),
                }
            })
        })
}

/// A simple expression cannot contain `?`, an embedded return,
/// a conditional or another control-flow expression.
fn simple_contract_expression(expr: &HirExpression) -> bool {
    matches!(
        expr,
        HirExpression::Name(..)
            | HirExpression::Integer(..)
            | HirExpression::Float(..)
            | HirExpression::String(..)
            | HirExpression::Bool(..)
    )
}

fn simple_contract_call(call: &tempra_hir::HirCall) -> bool {
    if !matches!(call.callee.as_ref(), HirExpression::Name(..)) {
        return false;
    }

    call.args.iter().all(|arg| {
        let value = match arg {
            HirCallArgument::Positional(value) => value,
            HirCallArgument::Named(named) => &named.value,
        };

        simple_contract_expression(value)
    })
}

fn exhaustive_result_match(match_expr: &tempra_hir::HirMatch) -> bool {
    if match_expr.arms.len() != 2 {
        return false;
    }

    if match_expr.arms.iter().any(|arm| {
        arm.guard.is_some()
            || arm.is_else
            || !matches!(
                &arm.value,
                HirExpression::Call(call)
                    if simple_contract_call(call)
            )
    }) {
        return false;
    }

    match_expr
        .arms
        .iter()
        .any(|arm| result_pattern_variant(&arm.pattern) == Some("Ok"))
        && match_expr
            .arms
            .iter()
            .any(|arm| result_pattern_variant(&arm.pattern) == Some("Err"))
}

fn local_result_consuming_contracts(
    file: &HirFile,
    facts: &crate::facts::TypecheckFacts,
    function_spans: &HashMap<String, tempra_span::Span>,
    current_module: &str,
) -> HashMap<tempra_span::Span, Vec<ResultConsumedParameter>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut ambiguous = HashSet::new();

    for item in &file.items {
        if let HirItem::Function(function) = item {
            if !seen.insert(function.name.clone()) {
                ambiguous.insert(function.name.clone());
            }
        }
    }

    let mut contracts: HashMap<tempra_span::Span, Vec<ResultConsumedParameter>> = HashMap::new();

    // Least fixed point: never infer a contract from an
    // unproven recursive cycle.
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

            let Some(body) = &function.body else {
                continue;
            };

            if body.statements.is_empty() {
                continue;
            }

            let mut proven = Vec::new();
            let mut supported = true;

            for statement in &body.statements {
                match statement {
                    HirStatement::Expr(HirExpression::Match(match_expr))
                        if exhaustive_result_match(match_expr) =>
                    {
                        let HirExpression::Name(_, name) = match_expr.value.as_ref() else {
                            supported = false;
                            break;
                        };

                        let Some(parameter) = result_parameter(function, name) else {
                            supported = false;
                            break;
                        };

                        proven.push(parameter);
                    }

                    HirStatement::Expr(HirExpression::Call(call)) if simple_contract_call(call) => {
                        let HirExpression::Name(callee_span, _) = call.callee.as_ref() else {
                            supported = false;
                            break;
                        };

                        // Calls without a proven consumption contract
                        // do not establish handling. They also do not
                        // invalidate handling already proved by a
                        // separate unconditional match.
                        let Some(callee_definition) = facts.resolved_symbols.get(callee_span)
                        else {
                            continue;
                        };

                        let Some(callee_parameters) = contracts.get(callee_definition) else {
                            continue;
                        };

                        for callee_parameter in callee_parameters {
                            let Some(argument_index) = result_call_argument_index(
                                call,
                                callee_parameter.index,
                                &callee_parameter.name,
                            ) else {
                                supported = false;
                                break;
                            };

                            let argument = match &call.args[argument_index] {
                                HirCallArgument::Positional(value) => value,
                                HirCallArgument::Named(named) => &named.value,
                            };

                            let HirExpression::Name(_, argument_name) = argument else {
                                supported = false;
                                break;
                            };

                            let Some(parameter) = result_parameter(function, argument_name) else {
                                supported = false;
                                break;
                            };

                            proven.push(parameter);
                        }

                        if !supported {
                            break;
                        }
                    }

                    // Return, branching, loops and arbitrary nested
                    // expressions cannot prove linear consumption.
                    _ => {
                        supported = false;
                        break;
                    }
                }
            }

            if !supported || proven.is_empty() {
                continue;
            }

            // Canonical parameter order, independent of call order
            // and HashMap iteration.
            proven.sort_by_key(|parameter| parameter.index);
            proven.dedup_by_key(|parameter| parameter.index);

            let unchanged = contracts.get(&definition_span).is_some_and(|previous| {
                previous.len() == proven.len()
                    && previous
                        .iter()
                        .zip(&proven)
                        .all(|(a, b)| a.index == b.index)
            });

            if unchanged {
                continue;
            }

            contracts.insert(definition_span, proven);
            changed = true;
        }

        if !changed {
            break;
        }
    }

    contracts
}

/// Resolve a value parameter to its positional or named call argument.
fn result_call_argument_index(
    call: &tempra_hir::HirCall,
    index: usize,
    name: &str,
) -> Option<usize> {
    if matches!(call.args.get(index), Some(HirCallArgument::Positional(_))) {
        Some(index)
    } else {
        call.args.iter().position(|arg| {
            matches!(
                arg,
                HirCallArgument::Named(named) if named.name == name
            )
        })
    }
}

// R3.1a: branch-local control-flow snapshots.
//
// Origin identities remain globally allocated. Snapshots record
// only the abstract state visible along an execution path.
// R3.1c: conditional binding provenance.
//
// A conditional binding can discharge an origin only when
// it covers every reachable path where that origin is pending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JoinedOrigin {
    origin: ResultOriginId,
    covers_pending_paths: bool,
}

#[derive(Clone)]
struct ResultFlowSnapshot {
    states: Vec<ObligationState>,
    bound_origins: HashMap<BindingId, ResultOriginId>,
    joined_bindings: HashMap<BindingId, Vec<JoinedOrigin>>,
    environment: ResultBindingEnvironment,
    reachable: bool,
}

// R3.1b: match arms and bounded loop flow.
//
// Loop analysis currently models zero iterations and one
// abstract iteration. Full back-edge fixed-point analysis
// remains a separate requirement.

#[derive(Default)]
struct LoopFlowExits {
    breaks: Vec<ResultFlowSnapshot>,
    continues: Vec<ResultFlowSnapshot>,
}

fn match_consumes_result(match_expr: &tempra_hir::HirMatch) -> bool {
    if match_expr.arms.len() != 2 {
        return false;
    }

    if match_expr
        .arms
        .iter()
        .any(|arm| arm.guard.is_some() || arm.is_else)
    {
        return false;
    }

    let has_ok = match_expr
        .arms
        .iter()
        .any(|arm| result_pattern_variant(&arm.pattern) == Some("Ok"));

    let has_err = match_expr
        .arms
        .iter()
        .any(|arm| result_pattern_variant(&arm.pattern) == Some("Err"));

    has_ok && has_err
}

struct ResultProvenanceAnalyzer<'a> {
    facts: &'a crate::facts::TypecheckFacts,
    forwarding_contracts: &'a HashMap<tempra_span::Span, ResultReturnProvenance>,
    consuming_contracts: &'a HashMap<tempra_span::Span, Vec<ResultConsumedParameter>>,
    tracker: ResultObligationTracker,
    bindings: ResultBindingEnvironment,
    origin_spans: Vec<tempra_span::Span>,
    origin_by_site: HashMap<tempra_span::Span, ResultOriginId>,
    reproduced_pending: Vec<ResultOriginId>,
    reachable: bool,
    exit_pending: Vec<ResultOriginId>,
    loop_exits: Vec<LoopFlowExits>,
    joined_bindings: HashMap<BindingId, Vec<JoinedOrigin>>,
}

impl<'a> ResultProvenanceAnalyzer<'a> {
    fn new(
        facts: &'a crate::facts::TypecheckFacts,
        forwarding_contracts: &'a HashMap<tempra_span::Span, ResultReturnProvenance>,
        consuming_contracts: &'a HashMap<tempra_span::Span, Vec<ResultConsumedParameter>>,
    ) -> Self {
        Self {
            facts,
            forwarding_contracts,
            consuming_contracts,
            tracker: ResultObligationTracker::new(),
            bindings: ResultBindingEnvironment::new(),
            origin_spans: Vec::new(),
            origin_by_site: HashMap::new(),
            reproduced_pending: Vec::new(),
            reachable: true,
            exit_pending: Vec::new(),
            loop_exits: Vec::new(),
            joined_bindings: HashMap::new(),
        }
    }

    fn is_result(&self, expr: &HirExpression) -> bool {
        matches!(
            self.facts.expr_types.get(&expr.span()),
            Some(crate::types::Type::Result(_, _))
        )
    }

    // R3.2b: static-site generations and loop-head fixed point.
    //
    // Each source site owns one abstract origin identity.
    // Reproducing the same site while its previous instance is
    // pending records sticky evidence of an unhandled instance.
    fn produce(&mut self, expr: &HirExpression) -> ResultOriginId {
        let span = expr.span();

        if let Some(&origin) = self.origin_by_site.get(&span) {
            let obligation = &mut self.tracker.obligations[origin.0];

            if obligation.state == ObligationState::Pending
                && !self.reproduced_pending.contains(&origin)
            {
                self.reproduced_pending.push(origin);
            }

            // The new dynamic instance starts pending even if a
            // previous instance from this site was handled.
            obligation.state = ObligationState::Pending;
            return origin;
        }

        let origin = self.tracker.produce();
        debug_assert_eq!(origin.0, self.origin_spans.len());

        self.origin_by_site.insert(span, origin);
        self.origin_spans.push(span);

        origin
    }

    fn flow_snapshot(&self) -> ResultFlowSnapshot {
        ResultFlowSnapshot {
            states: self
                .tracker
                .obligations
                .iter()
                .map(|obligation| obligation.state)
                .collect(),
            bound_origins: self.tracker.bindings.clone(),
            joined_bindings: self.joined_bindings.clone(),
            environment: self.bindings.clone(),
            reachable: self.reachable,
        }
    }

    fn restore_flow(&mut self, snapshot: &ResultFlowSnapshot) {
        // Do not truncate the global origin registry.
        // Origins produced by other branches become NotProduced
        // on the restored path, preserving unique IDs and spans.
        for (index, obligation) in self.tracker.obligations.iter_mut().enumerate() {
            obligation.state = snapshot
                .states
                .get(index)
                .copied()
                .unwrap_or(ObligationState::NotProduced);
        }

        self.tracker.bindings = snapshot.bound_origins.clone();
        self.joined_bindings = snapshot.joined_bindings.clone();
        self.bindings = snapshot.environment.clone();
        self.reachable = snapshot.reachable;
    }

    fn merge_flows(&mut self, before: &ResultFlowSnapshot, branches: &[ResultFlowSnapshot]) {
        // Returned paths are not successors of the conditional.
        // Their pending obligations were recorded at the exit.
        let live: Vec<_> = branches.iter().filter(|branch| branch.reachable).collect();

        if live.is_empty() {
            self.restore_flow(before);
            self.reachable = false;
            return;
        }

        // A Result remains pending if any reachable path has
        // an outstanding obligation for that origin.
        for (index, obligation) in self.tracker.obligations.iter_mut().enumerate() {
            let state_on = |branch: &&ResultFlowSnapshot| {
                branch
                    .states
                    .get(index)
                    .copied()
                    .unwrap_or(ObligationState::NotProduced)
            };

            obligation.state = if live
                .iter()
                .any(|branch| state_on(branch) == ObligationState::Pending)
            {
                ObligationState::Pending
            } else if live
                .iter()
                .any(|branch| state_on(branch) == ObligationState::Handled)
            {
                ObligationState::Handled
            } else if live
                .iter()
                .any(|branch| state_on(branch) == ObligationState::Propagated)
            {
                ObligationState::Propagated
            } else {
                ObligationState::NotProduced
            };
        }

        // Preserve conditional candidates without assuming
        // that a may-alias is a guaranteed binding.
        let mut merged_bindings = HashMap::new();
        let mut joined_bindings = HashMap::new();

        let mut binding_ids: Vec<_> = before
            .bound_origins
            .keys()
            .chain(before.joined_bindings.keys())
            .copied()
            .collect();

        binding_ids.sort_by_key(|binding| binding.0);
        binding_ids.dedup();

        for binding in binding_ids {
            let first = live[0].bound_origins.get(&binding).copied();

            if let Some(origin) = first {
                if live
                    .iter()
                    .all(|branch| branch.bound_origins.get(&binding) == Some(&origin))
                {
                    merged_bindings.insert(binding, origin);
                    continue;
                }
            }

            let mut origins = Vec::new();

            for branch in &live {
                if let Some(origin) = branch.bound_origins.get(&binding).copied() {
                    origins.push(origin);
                }

                if let Some(candidates) = branch.joined_bindings.get(&binding) {
                    origins.extend(candidates.iter().map(|candidate| candidate.origin));
                }
            }

            origins.sort_by_key(|origin| origin.0);
            origins.dedup();

            let mut candidates = Vec::with_capacity(origins.len());

            for origin in origins {
                let covers_pending_paths = live.iter().all(|branch| {
                    let state = branch
                        .states
                        .get(origin.0)
                        .copied()
                        .unwrap_or(ObligationState::NotProduced);

                    if state != ObligationState::Pending {
                        return true;
                    }

                    if branch.bound_origins.get(&binding) == Some(&origin) {
                        return true;
                    }

                    branch.joined_bindings.get(&binding).is_some_and(|joined| {
                        joined.iter().any(|candidate| {
                            candidate.origin == origin && candidate.covers_pending_paths
                        })
                    })
                });

                candidates.push(JoinedOrigin {
                    origin,
                    covers_pending_paths,
                });
            }

            if !candidates.is_empty() {
                joined_bindings.insert(binding, candidates);
            }
        }

        self.tracker.bindings = merged_bindings;
        self.joined_bindings = joined_bindings;
        self.bindings = before.environment.clone();
        self.reachable = true;
    }

    fn record_return_exit(&mut self) {
        self.exit_pending
            .extend(self.tracker.pending().map(|obligation| obligation.origin));
        self.reachable = false;
    }

    fn record_loop_jump(&mut self, is_break: bool) {
        let snapshot = self.flow_snapshot();

        if let Some(exits) = self.loop_exits.last_mut() {
            if is_break {
                exits.breaks.push(snapshot);
            } else {
                exits.continues.push(snapshot);
            }

            self.reachable = false;
        }
    }

    fn declare_pattern_bindings(&mut self, pattern: &tempra_hir::HirPattern) {
        use tempra_hir::HirPattern;

        match pattern {
            HirPattern::Identifier(_, name) => {
                self.bindings.declare(name);
            }

            HirPattern::EnumCase(_, _, patterns) | HirPattern::Tuple(_, patterns) => {
                for pattern in patterns {
                    self.declare_pattern_bindings(pattern);
                }
            }

            HirPattern::EnumConstructor(constructor) => {
                for pattern in &constructor.payloads {
                    self.declare_pattern_bindings(pattern);
                }
            }

            HirPattern::Struct(_, _, fields) => {
                for (name, pattern) in fields {
                    if let Some(pattern) = pattern {
                        self.declare_pattern_bindings(pattern);
                    } else {
                        self.bindings.declare(name);
                    }
                }
            }

            HirPattern::Or(_, alternatives) => {
                // Type checking already requires equivalent bindings
                // across OR alternatives. Declare them only once.
                if let Some(first) = alternatives.first() {
                    self.declare_pattern_bindings(first);
                }
            }

            HirPattern::Wildcard(_) | HirPattern::Literal(_, _) => {}
        }
    }

    fn equivalent_loop_flows(left: &ResultFlowSnapshot, right: &ResultFlowSnapshot) -> bool {
        left.reachable == right.reachable
            && left.states == right.states
            && left.bound_origins == right.bound_origins
            && left.joined_bindings == right.joined_bindings
    }

    fn analyze_for_loop(
        &mut self,
        binding: &str,
        iterable: &HirExpression,
        filter: Option<&HirExpression>,
        body: &HirBlock,
        produces_elements: bool,
    ) {
        // The iterable is evaluated once before entering the loop.
        self.analyze_expression(iterable);

        let entry = self.flow_snapshot();
        let mut head = entry.clone();

        // Break paths are genuine exits, never back-edges.
        let mut break_exits = Vec::new();

        loop {
            self.restore_flow(&head);

            self.loop_exits.push(LoopFlowExits::default());

            self.bindings.enter_scope();
            self.bindings.declare(binding);

            if let Some(filter) = filter {
                self.analyze_expression(filter);
            }

            let tail_origin = if self.reachable {
                self.analyze_block(body, false)
            } else {
                None
            };

            if produces_elements && self.reachable {
                if let Some(origin) = tail_origin {
                    // Array-comprehension elements are transferred
                    // into the collection, not simply discarded.
                    self.tracker.propagate(origin);
                }
            }

            self.bindings.exit_scope();

            let body_exit = self.flow_snapshot();

            let exits = self
                .loop_exits
                .pop()
                .expect("loop flow stack must be balanced");

            break_exits.extend(exits.breaks);

            // A normal completed iteration and a continue both
            // return control to the loop head.
            let mut back_edges = Vec::new();

            if body_exit.reachable {
                back_edges.push(body_exit);
            }

            back_edges.extend(exits.continues);

            self.restore_flow(&entry);

            // Monotone accumulation of possible loop-head states.
            // The zero-iteration path is always included.
            let mut incoming = vec![entry.clone(), head.clone()];
            incoming.extend(back_edges);

            self.merge_flows(&entry, &incoming);

            let next_head = self.flow_snapshot();

            if Self::equivalent_loop_flows(&head, &next_head) {
                head = next_head;
                break;
            }

            head = next_head;
        }

        // The loop may terminate after zero or more iterations.
        // Break exits bypass the back-edge and must be preserved.
        self.restore_flow(&entry);

        let mut exit_paths = vec![entry.clone(), head];
        exit_paths.extend(break_exits);

        self.merge_flows(&entry, &exit_paths);
    }

    fn analyze_block(
        &mut self,
        block: &HirBlock,
        is_function_body: bool,
    ) -> Option<ResultOriginId> {
        self.bindings.enter_scope();

        let last_index = block.statements.len().checked_sub(1);
        let mut tail_origin = None;

        for (index, statement) in block.statements.iter().enumerate() {
            if !self.reachable {
                break;
            }

            if Some(index) == last_index {
                if let HirStatement::Expr(expr) = statement {
                    let origin = self.analyze_expression(expr);

                    if is_function_body {
                        if let Some(origin) = origin {
                            self.tracker.propagate(origin);
                        }
                    } else {
                        tail_origin = origin;
                    }

                    continue;
                }
            }

            self.analyze_statement(statement);
        }

        self.bindings.exit_scope();
        tail_origin
    }

    /// Transfer nested Result obligations through a directly returned
    /// Result constructor. The container itself must be returned;
    /// ordinary construction never discharges payload obligations.
    fn propagate_returned_result_payloads(&mut self, expr: &HirExpression) {
        if !self.is_result(expr) {
            return;
        }

        match expr {
            HirExpression::Call(call)
                if matches!(
                    call.callee.as_ref(),
                    HirExpression::Name(_, name)
                        if name == "Ok" || name == "Err"
                ) =>
            {
                for argument in &call.args {
                    let payload = match argument {
                        HirCallArgument::Positional(value) => value,
                        HirCallArgument::Named(named) => &named.value,
                    };
                    self.transfer_returned_payload(payload);
                }
            }

            HirExpression::EnumConstructor(constructor)
                if constructor.variant_name == "Ok" || constructor.variant_name == "Err" =>
            {
                for payload in &constructor.payloads {
                    self.transfer_returned_payload(payload);
                }
            }

            _ => {}
        }
    }

    fn transfer_returned_payload(&mut self, payload: &HirExpression) {
        if self.is_result(payload) {
            let origin = match payload {
                HirExpression::Name(_, name) => self
                    .bindings
                    .resolve(name)
                    .and_then(|id| self.tracker.origin_of(id)),

                _ => self.origin_by_site.get(&payload.span()).copied(),
            };

            if let Some(origin) = origin {
                self.tracker.propagate(origin);
            }
        }

        self.propagate_returned_result_payloads(payload);
    }

    fn analyze_statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::Let(binding) => {
                let joined_source = if let HirExpression::Name(_, name) = &binding.value {
                    self.bindings
                        .resolve(name)
                        .and_then(|id| self.joined_bindings.get(&id))
                        .cloned()
                } else {
                    None
                };

                let origin = self.analyze_expression(&binding.value);
                let id = self.bindings.declare(&binding.name);

                if let Some(origin) = origin {
                    self.tracker.bind(id, origin);
                } else if let Some(candidates) = joined_source {
                    self.joined_bindings.insert(id, candidates);
                }
            }

            HirStatement::Var(binding) => {
                let joined_source = if let HirExpression::Name(_, name) = &binding.value {
                    self.bindings
                        .resolve(name)
                        .and_then(|id| self.joined_bindings.get(&id))
                        .cloned()
                } else {
                    None
                };

                let origin = self.analyze_expression(&binding.value);
                let id = self.bindings.declare(&binding.name);

                if let Some(origin) = origin {
                    self.tracker.bind(id, origin);
                } else if let Some(candidates) = joined_source {
                    self.joined_bindings.insert(id, candidates);
                }
            }

            HirStatement::Return(Some(expr)) => {
                if let Some(origin) = self.analyze_expression(expr) {
                    self.tracker.propagate(origin);
                }

                self.propagate_returned_result_payloads(expr);
                self.record_return_exit();
            }

            HirStatement::Return(None) => {
                self.record_return_exit();
            }

            HirStatement::Expr(expr) => {
                self.analyze_expression(expr);
            }

            HirStatement::For(for_stmt) => {
                self.analyze_for_loop(
                    &for_stmt.binding,
                    &for_stmt.iterable,
                    for_stmt.filter.as_ref(),
                    &for_stmt.body,
                    false,
                );
            }

            HirStatement::Break(_) => {
                self.record_loop_jump(true);
            }

            HirStatement::Continue(_) => {
                self.record_loop_jump(false);
            }
        }
    }

    fn analyze_expression(&mut self, expr: &HirExpression) -> Option<ResultOriginId> {
        match expr {
            HirExpression::Name(_, name) => self.bindings.resolve(name).and_then(|id| {
                self.tracker.origin_of(id).or_else(|| {
                    self.joined_bindings.get(&id).and_then(|candidates| {
                        if candidates.len() == 1 && candidates[0].covers_pending_paths {
                            Some(candidates[0].origin)
                        } else {
                            None
                        }
                    })
                })
            }),

            HirExpression::Call(call) => {
                let mut argument_origins = Vec::with_capacity(call.args.len());

                for arg in &call.args {
                    let origin = match arg {
                        HirCallArgument::Positional(value) => self.analyze_expression(value),
                        HirCallArgument::Named(named) => self.analyze_expression(&named.value),
                    };

                    argument_origins.push(origin);
                }

                // R2.5a: a proven consumer handles its Result argument.
                // Resolve by definition span to avoid shadowing mistakes.
                if let HirExpression::Name(callee_span, _) = call.callee.as_ref() {
                    if let Some(definition_span) = self.facts.resolved_symbols.get(callee_span) {
                        if let Some(parameters) = self.consuming_contracts.get(definition_span) {
                            for parameter in parameters {
                                let argument_index = result_call_argument_index(
                                    call,
                                    parameter.index,
                                    &parameter.name,
                                );

                                if let Some(Some(origin)) =
                                    argument_index.and_then(|i| argument_origins.get(i))
                                {
                                    self.tracker.handle(*origin);
                                }
                            }
                        }
                    }
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

            HirExpression::If(if_expr) => {
                self.analyze_expression(&if_expr.condition);

                let produces_result = self.is_result(expr);
                let before = self.flow_snapshot();

                // True branch.
                let then_origin = self.analyze_block(&if_expr.then_block, false);

                // A Result returned by an if expression transfers
                // its branch origin to the enclosing expression.
                if produces_result && self.reachable {
                    if let Some(origin) = then_origin {
                        self.tracker.propagate(origin);
                    }
                }

                let then_state = self.flow_snapshot();

                // False branch starts from the same pre-if state.
                self.restore_flow(&before);

                if let Some(else_branch) = &if_expr.else_branch {
                    let else_origin = self.analyze_expression(else_branch);

                    if produces_result && self.reachable {
                        if let Some(origin) = else_origin {
                            self.tracker.propagate(origin);
                        }
                    }
                }

                let else_state = self.flow_snapshot();

                self.merge_flows(&before, &[then_state, else_state]);

                // The enclosing Result has its own obligation.
                // This is a conservative R3.1a value merge;
                // full provenance unions belong to R3.1c.
                if produces_result && self.reachable {
                    Some(self.produce(expr))
                } else {
                    None
                }
            }

            HirExpression::Match(match_expr) => {
                let scrutinee_origin = self.analyze_expression(&match_expr.value);

                // A wildcard match is not proof that both
                // success and failure have been acknowledged.
                if match_consumes_result(match_expr) {
                    if let Some(origin) = scrutinee_origin {
                        self.tracker.handle(origin);
                    }

                    if let HirExpression::Name(_, name) = match_expr.value.as_ref() {
                        if let Some(binding) = self.bindings.resolve(name) {
                            if let Some(candidates) = self.joined_bindings.get(&binding) {
                                for candidate in candidates {
                                    if candidate.covers_pending_paths {
                                        self.tracker.handle(candidate.origin);
                                    }
                                }
                            }
                        }
                    }
                }

                let before = self.flow_snapshot();
                let produces_result = self.is_result(expr);
                let mut paths = Vec::with_capacity(match_expr.arms.len());

                for arm in &match_expr.arms {
                    self.restore_flow(&before);

                    self.bindings.enter_scope();
                    self.declare_pattern_bindings(&arm.pattern);

                    if let Some(guard) = &arm.guard {
                        self.analyze_expression(guard);
                    }

                    let arm_origin = if self.reachable {
                        self.analyze_expression(&arm.value)
                    } else {
                        None
                    };

                    // Result-valued match arms transfer their
                    // values into the enclosing expression.
                    if produces_result && self.reachable {
                        if let Some(origin) = arm_origin {
                            self.tracker.propagate(origin);
                        }
                    }

                    self.bindings.exit_scope();
                    paths.push(self.flow_snapshot());
                }

                self.merge_flows(&before, &paths);

                if produces_result && self.reachable {
                    Some(self.produce(expr))
                } else {
                    None
                }
            }

            HirExpression::For(for_expr) => {
                self.analyze_for_loop(
                    &for_expr.binding,
                    &for_expr.iterable,
                    for_expr.filter.as_deref(),
                    &for_expr.body,
                    true,
                );

                None
            }

            HirExpression::Binary(binary) if binary.op == HirBinaryOp::Assign => {
                let joined_source = if let HirExpression::Name(_, name) = binary.right.as_ref() {
                    self.bindings
                        .resolve(name)
                        .and_then(|id| self.joined_bindings.get(&id))
                        .cloned()
                } else {
                    None
                };

                let new_origin = self.analyze_expression(&binary.right);

                if let HirExpression::Name(_, name) = binary.left.as_ref() {
                    if let Some(binding) = self.bindings.resolve(name) {
                        match (new_origin, joined_source) {
                            (Some(origin), _) => {
                                self.tracker.overwrite(binding, origin);
                                self.joined_bindings.remove(&binding);
                            }

                            (None, Some(candidates)) => {
                                self.tracker.bindings.remove(&binding);
                                self.joined_bindings.insert(binding, candidates);
                            }

                            (None, None) => {
                                self.tracker.bindings.remove(&binding);
                                self.joined_bindings.remove(&binding);
                            }
                        }
                    }
                }

                None
            }

            HirExpression::Block(block) => self.analyze_block(block, false),

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
        let mut origins: Vec<ResultOriginId> = self
            .tracker
            .pending()
            .map(|obligation| obligation.origin)
            .chain(self.exit_pending.iter().copied())
            .chain(self.reproduced_pending.iter().copied())
            .collect();

        // Canonical source-origin order, without duplicate errors
        // when multiple paths leave the same Result unhandled.
        origins.sort_by_key(|origin| origin.0);
        origins.dedup();

        origins
            .into_iter()
            .map(|origin| {
                crate::error::TypeError::new(
                    UNHANDLED_RESULT_DIAGNOSTIC,
                    self.origin_spans[origin.0],
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
    let consuming_contracts =
        local_result_consuming_contracts(file, facts, function_spans, current_module);

    for item in &file.items {
        if let HirItem::Function(function) = item {
            if let Some(body) = &function.body {
                let mut analyzer = ResultProvenanceAnalyzer::new(
                    facts,
                    &forwarding_contracts,
                    &consuming_contracts,
                );
                analyzer.analyze_block(body, true);
                diagnostics.extend(analyzer.diagnostics());
            }
        }
    }

    if !file.top_level_statements.is_empty() {
        let mut analyzer =
            ResultProvenanceAnalyzer::new(facts, &forwarding_contracts, &consuming_contracts);

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
