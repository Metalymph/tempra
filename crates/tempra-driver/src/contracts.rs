//! Declaration contracts and their compilation-local binding.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use tempra_effects::{
    Capability, CapabilitySet, EffectCategory, EffectFamily, EffectOperation, EffectParameter,
    ResourceValue,
};
use tempra_hir::HirFunction;
use tempra_span::Span;
use tempra_tir::ir::TirFunctionId;

/// Identity of the source containing a contract, scoped to one compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContractSource {
    /// The single string passed to the source compilation API; no file path is invented.
    InMemory,
    /// A module loaded by the existing module graph.
    Module {
        /// Module name used by the graph and lowering.
        name: String,
        /// Source path retained from the module node.
        path: PathBuf,
    },
}

/// Canonical value of one specialized deny parameter.
///
/// `Any` is the explicit wildcard. `Exact` preserves declaration-site text or
/// port values. K4.5B matches these values against propagated local-effect facts
/// without changing how the clause was parsed or canonicalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializedDenyValue {
    /// Explicit wildcard `*`.
    Any,
    /// Exact text or port literal.
    Exact(ResourceValue),
}

/// Canonical target of one specialized deny clause.
///
/// Family and category targets remain distinct from operation targets. They are
/// never expanded into operation lists. Matching uses canonical fact metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecializedDenyTarget {
    /// A whole specialized-effect family, such as `fs` or `net`.
    Family(EffectFamily),
    /// A cross-family category, such as `io.read` or `io.write`.
    Category(EffectCategory),
    /// One concrete operation, such as `fs.read` or `io.read()`.
    Operation(EffectOperation),
}

impl SpecializedDenyTarget {
    /// Returns the stable canonical spelling of the target.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Family(family) => family.canonical_name(),
            Self::Category(category) => category.canonical_name(),
            Self::Operation(operation) => operation.canonical_name(),
        }
    }

    /// Returns the concrete operation target, if this clause is operation-scoped.
    pub const fn operation(self) -> Option<EffectOperation> {
        match self {
            Self::Operation(operation) => Some(operation),
            Self::Family(_) | Self::Category(_) => None,
        }
    }
}

/// One canonical `parameter: value` restriction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedDenyParameter {
    /// Canonical parameter key from `tempra-effects`.
    pub parameter: EffectParameter,
    /// Literal or wildcard value.
    pub value: SpecializedDenyValue,
    /// Source span of `parameter: value`.
    pub span: Span,
}

/// One canonical specialized deny clause.
///
/// Clauses are sorted by target kind and taxonomy order, then by canonical
/// parameter order and values. Duplicate target-plus-parameter clauses are
/// removed while retaining the first source occurrence. Family and category
/// clauses remain distinct canonical entries even when their current operation
/// membership overlaps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedDenyClause {
    /// Canonical family, category or operation target.
    pub target: SpecializedDenyTarget,
    /// Parameters in canonical taxonomy order. Family and category targets are
    /// always parameterless.
    pub parameters: Vec<SpecializedDenyParameter>,
    /// Source span of the original clause.
    pub span: Span,
}

/// Explicit user intent attached to one concrete TIR function.
/// E3.2 compares this retained intent with the function's required capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCapabilityContract {
    /// Capabilities written explicitly in one or more `@deny` attributes.
    ///
    /// This remains separate from [`Self::denied`] so tooling can distinguish
    /// explicit source intent from the capabilities contributed by `@pure`.
    pub explicit_denied: CapabilitySet,
    /// Effective denied capabilities: explicit deny spellings union the pure restriction set.
    pub denied: CapabilitySet,
    /// Canonical specialized clauses. These are matched independently against
    /// propagated specialized-effect facts; they never contribute to `denied`.
    pub specialized_denied: Vec<SpecializedDenyClause>,
    /// Existing function declaration span, excluding preceding attributes.
    pub declaration_span: Span,
    /// Complete spans of explicit `@deny` attributes only, in source order.
    pub attribute_spans: Vec<Span>,
    /// Original `@pure` intent, separate from explicit deny provenance.
    pub pure_span: Option<Span>,
    /// Identity qualifying the file-relative spans.
    pub source: ContractSource,
}

/// Distinct declaration bindings would otherwise overwrite a user contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractBindingError {
    /// TIR ID shared by the conflicting bindings.
    pub function: TirFunctionId,
    /// Source of the previous declaration.
    pub previous_source: ContractSource,
    /// Span of the previous declaration.
    pub previous_span: Span,
    /// Source of the incoming declaration.
    pub incoming_source: ContractSource,
    /// Span of the incoming declaration.
    pub incoming_span: Span,
}

impl std::fmt::Display for ContractBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ambiguous contract binding for TIR function {}",
            self.function.0
        )
    }
}

impl std::error::Error for ContractBindingError {}

/// A target/parameter combination that cannot form a canonical specialized
/// contract.
///
/// Parser validation normally makes these unreachable. Driver binding still
/// checks them so a programmatically constructed AST cannot silently produce an
/// ambiguous or malformed contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecializedDenyBindingError {
    /// A family or category target was given resource parameters.
    ParametersNotAllowed {
        /// The parameterized family or category target.
        target: SpecializedDenyTarget,
        /// Span of the first disallowed parameter.
        span: Span,
    },
    /// A concrete operation was given a parameter key it does not declare.
    ParameterNotAccepted {
        /// The operation target.
        target: SpecializedDenyTarget,
        /// The rejected parameter key.
        parameter: EffectParameter,
        /// Span of the rejected parameter.
        span: Span,
    },
    /// An operation clause repeated the same parameter key.
    DuplicateParameter {
        /// The operation target.
        target: SpecializedDenyTarget,
        /// The repeated parameter key.
        parameter: EffectParameter,
        /// Span of the repeated parameter.
        span: Span,
    },
    /// A parameter value's kind does not match its parameter key.
    InvalidParameterValue {
        /// The operation target.
        target: SpecializedDenyTarget,
        /// The parameter whose value has the wrong kind.
        parameter: EffectParameter,
        /// Span of the invalid value.
        span: Span,
    },
}

impl std::fmt::Display for SpecializedDenyBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParametersNotAllowed { target, .. } => write!(
                f,
                "specialized deny target '{}' does not accept resource parameters",
                target.canonical_name()
            ),
            Self::ParameterNotAccepted {
                target, parameter, ..
            } => write!(
                f,
                "specialized deny target '{}' does not accept parameter '{}'",
                target.canonical_name(),
                parameter.canonical_name()
            ),
            Self::DuplicateParameter {
                target, parameter, ..
            } => write!(
                f,
                "specialized deny target '{}' repeats parameter '{}'",
                target.canonical_name(),
                parameter.canonical_name()
            ),
            Self::InvalidParameterValue {
                target, parameter, ..
            } => write!(
                f,
                "specialized deny target '{}' has an invalid value for parameter '{}'",
                target.canonical_name(),
                parameter.canonical_name()
            ),
        }
    }
}

impl std::error::Error for SpecializedDenyBindingError {}

/// A recognized specialized target unsupported by an executable compilation.
///
/// Retained for API compatibility. K4.9D enforces every valid v0.2 target, so
/// binding no longer produces this error for families, categories or operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSpecializedTarget {
    /// Canonical target spelling from the bound clause.
    pub target: String,
    /// Source span of the original clause.
    pub span: Span,
}

impl std::fmt::Display for UnsupportedSpecializedTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "specialized deny target '{}' is recognized but not yet enforceable",
            self.target
        )
    }
}

impl std::error::Error for UnsupportedSpecializedTarget {}

// These are the sole frontend-to-canonical contract conversions. Neither AST nor
// HIR imports analyzer types or reasons about capability requirements.
pub(crate) const PURE_DENIED: CapabilitySet = CapabilitySet::singleton(Capability::Io)
    .union(CapabilitySet::singleton(Capability::Process))
    .union(CapabilitySet::singleton(Capability::Spawn))
    .union(CapabilitySet::singleton(Capability::Nondeterminism));

fn canonical_capability(name: tempra_ast::DenyCapability) -> Capability {
    use tempra_ast::DenyCapability;
    match name {
        DenyCapability::Alloc => Capability::Alloc,
        DenyCapability::Panic => Capability::Panic,
        DenyCapability::Io => Capability::Io,
        DenyCapability::Process => Capability::Process,
        DenyCapability::Spawn => Capability::Spawn,
        DenyCapability::Nondeterminism => Capability::Nondeterminism,
    }
}

fn canonical_operation(name: tempra_ast::SpecializedOperation) -> EffectOperation {
    use tempra_ast::SpecializedOperation;
    match name {
        SpecializedOperation::FsRead => EffectOperation::FsRead,
        SpecializedOperation::FsWrite => EffectOperation::FsWrite,
        SpecializedOperation::FsRemove => EffectOperation::FsRemove,
        SpecializedOperation::NetConnect => EffectOperation::NetConnect,
        SpecializedOperation::NetListen => EffectOperation::NetListen,
        SpecializedOperation::NetSend => EffectOperation::NetSend,
        SpecializedOperation::NetReceive => EffectOperation::NetReceive,
        SpecializedOperation::IoRead => EffectOperation::IoRead,
        SpecializedOperation::IoWrite => EffectOperation::IoWrite,
        SpecializedOperation::ProcessExec => EffectOperation::ProcessExec,
        SpecializedOperation::ProcessExit => EffectOperation::ProcessExit,
        SpecializedOperation::SpawnTask => EffectOperation::SpawnTask,
        SpecializedOperation::SpawnActor => EffectOperation::SpawnActor,
    }
}

fn canonical_family(family: tempra_ast::SpecializedFamily) -> EffectFamily {
    match family {
        tempra_ast::SpecializedFamily::Fs => EffectFamily::Fs,
        tempra_ast::SpecializedFamily::Net => EffectFamily::Net,
    }
}

fn canonical_category(category: tempra_ast::SpecializedCategory) -> EffectCategory {
    match category {
        tempra_ast::SpecializedCategory::IoRead => EffectCategory::IoRead,
        tempra_ast::SpecializedCategory::IoWrite => EffectCategory::IoWrite,
    }
}

fn canonical_target(target: tempra_ast::SpecializedDenyTarget) -> SpecializedDenyTarget {
    match target {
        tempra_ast::SpecializedDenyTarget::Family(family) => {
            SpecializedDenyTarget::Family(canonical_family(family))
        }
        tempra_ast::SpecializedDenyTarget::Category(category) => {
            SpecializedDenyTarget::Category(canonical_category(category))
        }
        tempra_ast::SpecializedDenyTarget::Operation(operation) => {
            SpecializedDenyTarget::Operation(canonical_operation(operation))
        }
    }
}

fn canonical_parameter(name: tempra_ast::SpecializedParameter) -> EffectParameter {
    use tempra_ast::SpecializedParameter;
    match name {
        SpecializedParameter::Source => EffectParameter::Source,
        SpecializedParameter::Destination => EffectParameter::Destination,
        SpecializedParameter::Host => EffectParameter::Host,
        SpecializedParameter::Port => EffectParameter::Port,
        SpecializedParameter::Protocol => EffectParameter::Protocol,
        SpecializedParameter::Executable => EffectParameter::Executable,
        SpecializedParameter::Stream => EffectParameter::Stream,
        SpecializedParameter::Scope => EffectParameter::Scope,
    }
}

fn canonical_specialized_value(value: &tempra_ast::SpecializedValue) -> SpecializedDenyValue {
    match value {
        tempra_ast::SpecializedValue::Any => SpecializedDenyValue::Any,
        tempra_ast::SpecializedValue::Text(text) => {
            SpecializedDenyValue::Exact(ResourceValue::Text(text.clone()))
        }
        tempra_ast::SpecializedValue::Port(port) => {
            SpecializedDenyValue::Exact(ResourceValue::Port(*port))
        }
    }
}

fn family_order(family: EffectFamily) -> usize {
    EffectFamily::ALL
        .iter()
        .position(|candidate| *candidate == family)
        .expect("EffectFamily::ALL is exhaustive")
}

fn category_order(category: EffectCategory) -> usize {
    EffectCategory::ALL
        .iter()
        .position(|candidate| *candidate == category)
        .expect("EffectCategory::ALL is exhaustive")
}

fn operation_order(operation: EffectOperation) -> usize {
    EffectOperation::ALL
        .iter()
        .position(|candidate| *candidate == operation)
        .expect("EffectOperation::ALL is exhaustive")
}

fn target_order(target: SpecializedDenyTarget) -> (u8, usize) {
    match target {
        SpecializedDenyTarget::Family(family) => (0, family_order(family)),
        SpecializedDenyTarget::Category(category) => (1, category_order(category)),
        SpecializedDenyTarget::Operation(operation) => (2, operation_order(operation)),
    }
}

fn parameter_order(parameter: EffectParameter) -> usize {
    EffectParameter::ALL
        .iter()
        .position(|candidate| *candidate == parameter)
        .expect("EffectParameter::ALL is exhaustive")
}

fn compare_specialized_values(
    left: &SpecializedDenyValue,
    right: &SpecializedDenyValue,
) -> Ordering {
    use SpecializedDenyValue::Exact;
    match (left, right) {
        (SpecializedDenyValue::Any, SpecializedDenyValue::Any) => Ordering::Equal,
        (SpecializedDenyValue::Any, _) => Ordering::Less,
        (_, SpecializedDenyValue::Any) => Ordering::Greater,
        (Exact(ResourceValue::Text(left)), Exact(ResourceValue::Text(right))) => left.cmp(right),
        (Exact(ResourceValue::Port(left)), Exact(ResourceValue::Port(right))) => left.cmp(right),
        (Exact(ResourceValue::Text(_)), Exact(ResourceValue::Port(_))) => Ordering::Less,
        (Exact(ResourceValue::Port(_)), Exact(ResourceValue::Text(_))) => Ordering::Greater,
    }
}

fn compare_specialized_parameters(
    left: &[SpecializedDenyParameter],
    right: &[SpecializedDenyParameter],
) -> Ordering {
    for (left, right) in left.iter().zip(right) {
        let ordering = parameter_order(left.parameter)
            .cmp(&parameter_order(right.parameter))
            .then_with(|| compare_specialized_values(&left.value, &right.value));
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.len().cmp(&right.len())
}

fn compare_specialized_clauses(
    left: &SpecializedDenyClause,
    right: &SpecializedDenyClause,
) -> Ordering {
    target_order(left.target)
        .cmp(&target_order(right.target))
        .then_with(|| compare_specialized_parameters(&left.parameters, &right.parameters))
}

fn specialized_parameters_equal(
    left: &[SpecializedDenyParameter],
    right: &[SpecializedDenyParameter],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.parameter == right.parameter && left.value == right.value)
}

fn parameter_value_kind_matches(parameter: EffectParameter, value: &SpecializedDenyValue) -> bool {
    match value {
        SpecializedDenyValue::Any => true,
        SpecializedDenyValue::Exact(ResourceValue::Text(_)) => parameter != EffectParameter::Port,
        SpecializedDenyValue::Exact(ResourceValue::Port(_)) => parameter == EffectParameter::Port,
    }
}

fn validate_specialized_parameters(
    target: SpecializedDenyTarget,
    parameters: &[SpecializedDenyParameter],
) -> Result<(), SpecializedDenyBindingError> {
    let SpecializedDenyTarget::Operation(operation) = target else {
        if let Some(parameter) = parameters.first() {
            return Err(SpecializedDenyBindingError::ParametersNotAllowed {
                target,
                span: parameter.span,
            });
        }
        return Ok(());
    };

    for (index, parameter) in parameters.iter().enumerate() {
        if !operation.accepts_parameter(parameter.parameter) {
            return Err(SpecializedDenyBindingError::ParameterNotAccepted {
                target,
                parameter: parameter.parameter,
                span: parameter.span,
            });
        }
        if parameters[..index]
            .iter()
            .any(|previous| previous.parameter == parameter.parameter)
        {
            return Err(SpecializedDenyBindingError::DuplicateParameter {
                target,
                parameter: parameter.parameter,
                span: parameter.span,
            });
        }
        if !parameter_value_kind_matches(parameter.parameter, &parameter.value) {
            return Err(SpecializedDenyBindingError::InvalidParameterValue {
                target,
                parameter: parameter.parameter,
                span: parameter.span,
            });
        }
    }
    Ok(())
}

fn bind_specialized_denied(
    clauses: &[tempra_ast::SpecializedDenyClause],
) -> Result<Vec<SpecializedDenyClause>, SpecializedDenyBindingError> {
    let mut canonical = Vec::with_capacity(clauses.len());
    for clause in clauses {
        let target = canonical_target(clause.target);
        let parameters: Vec<_> = clause
            .parameters
            .iter()
            .map(|parameter| SpecializedDenyParameter {
                parameter: canonical_parameter(parameter.parameter),
                value: canonical_specialized_value(&parameter.value),
                span: parameter.span,
            })
            .collect();
        validate_specialized_parameters(target, &parameters)?;
        canonical.push(SpecializedDenyClause {
            target,
            parameters,
            span: clause.span,
        });
    }
    Ok(canonical)
}

fn finalize_specialized_denied(canonical: &mut Vec<SpecializedDenyClause>) {
    for clause in &mut *canonical {
        clause
            .parameters
            .sort_by_key(|parameter| parameter_order(parameter.parameter));
    }

    // Stable sorting keeps the first source occurrence of an identical clause.
    canonical.sort_by(compare_specialized_clauses);
    canonical.dedup_by(|left, right| {
        left.target == right.target
            && specialized_parameters_equal(&left.parameters, &right.parameters)
    });
}

/// Binds and canonicalizes specialized deny syntax without deciding whether the
/// current compiler milestone can enforce it.
///
/// The result retains family and category targets as distinct canonical clauses
/// and never expands them into operations. It is suitable for tooling and tests
/// that need to inspect the complete v0.2 contract vocabulary. Executable
/// compilation enforces these same canonical targets against analyzed facts.
pub fn canonicalize_specialized_denied(
    clauses: &[tempra_ast::SpecializedDenyClause],
) -> Result<Vec<SpecializedDenyClause>, SpecializedDenyBindingError> {
    let mut canonical = bind_specialized_denied(clauses)?;
    finalize_specialized_denied(&mut canonical);
    Ok(canonical)
}

struct Binding {
    source: ContractSource,
    span: Span,
    contract: Option<FunctionCapabilityContract>,
}

/// Temporary registry also tracks unannotated definitions to catch overwrites
/// in either order. Only explicit contracts survive in the compilation product.
#[derive(Default)]
pub(crate) struct ContractBindings {
    seen: HashMap<TirFunctionId, Binding>,
}

impl ContractBindings {
    pub(crate) fn register(
        &mut self,
        function: TirFunctionId,
        declaration: &HirFunction,
        source: &ContractSource,
    ) -> Result<(), crate::DriverError> {
        let contract = if declaration.deny_contract.is_some() || declaration.pure_span.is_some() {
            let mut explicit_denied = CapabilitySet::empty();
            let mut specialized_denied = Vec::new();
            let mut attribute_spans = Vec::new();
            if let Some(syntax) = &declaration.deny_contract {
                for name in &syntax.denied {
                    explicit_denied.insert(canonical_capability(*name));
                }
                let mut bound =
                    bind_specialized_denied(&syntax.specialized_denied).map_err(|error| {
                        crate::DriverError::SpecializedDenyBindingError(Box::new(error))
                    })?;
                finalize_specialized_denied(&mut bound);
                specialized_denied = bound;
                attribute_spans.clone_from(&syntax.attribute_spans);
            }
            let denied = if declaration.pure_span.is_some() {
                explicit_denied.union(PURE_DENIED)
            } else {
                explicit_denied
            };
            Some(FunctionCapabilityContract {
                explicit_denied,
                denied,
                specialized_denied,
                declaration_span: declaration.span,
                attribute_spans,
                pure_span: declaration.pure_span,
                source: source.clone(),
            })
        } else {
            None
        };
        if let Some(previous) = self.seen.get(&function) {
            let same_binding = previous.source == *source
                && previous.span == declaration.span
                && previous.contract == contract;
            if !same_binding && (previous.contract.is_some() || contract.is_some()) {
                return Err(crate::DriverError::ContractBindingError(Box::new(
                    ContractBindingError {
                        function,
                        previous_source: previous.source.clone(),
                        previous_span: previous.span,
                        incoming_source: source.clone(),
                        incoming_span: declaration.span,
                    },
                )));
            }
        }
        self.seen.insert(
            function,
            Binding {
                source: source.clone(),
                span: declaration.span,
                contract,
            },
        );
        Ok(())
    }

    pub(crate) fn finish(self) -> Vec<(TirFunctionId, FunctionCapabilityContract)> {
        let mut contracts: Vec<_> = self
            .seen
            .into_iter()
            .filter_map(|(id, binding)| binding.contract.map(|contract| (id, contract)))
            .collect();
        contracts.sort_by_key(|(id, _)| id.0);
        contracts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempra_tir::builder::ModuleBuilder;

    #[test]
    fn pure_specializations_keep_distinct_ids_and_shared_source_intent() {
        let source = "@pure fn work(type T, value: T) -> T { return value }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let mut module = ModuleBuilder::new();
        let mut bindings = ContractBindings::default();
        let origin = ContractSource::Module {
            name: "a".into(),
            path: PathBuf::from("a.tmpr"),
        };
        let mut ids = Vec::new();
        for ty in [tempra_hir::Type::Int, tempra_hir::Type::String] {
            let concrete = tempra_tir_lower::lower::lower_function_specialization(
                function,
                &context,
                &mut module,
                None,
                Some("a"),
                &[ty],
            )
            .unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            ids.push(concrete.id);
            module.add_function(concrete);
        }
        let compilation = crate::compilation::compile_tir_module(module.module, bindings).unwrap();
        assert_ne!(ids[0], ids[1]);
        assert_eq!(compilation.contracts().count(), 2);
        assert_eq!(compilation.contract(ids[0]), compilation.contract(ids[1]));
        let contract = compilation.contract(ids[0]).unwrap();
        assert_eq!(contract.source, origin);
        assert_eq!(contract.pure_span, function.pure_span);
        assert!(contract.attribute_spans.is_empty());
        assert_eq!(contract.denied, PURE_DENIED);
    }

    #[test]
    fn specialized_specializations_inherit_one_source_intent_on_distinct_ids() {
        let source = "@deny(fs.write(destination: \"/private\")) fn work(type T, value: T) -> T { return value }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let mut module = ModuleBuilder::new();
        let mut bindings = ContractBindings::default();
        let origin = ContractSource::Module {
            name: "a".into(),
            path: PathBuf::from("a.tmpr"),
        };
        let mut ids = Vec::new();
        for ty in [tempra_hir::Type::Int, tempra_hir::Type::String] {
            let concrete = tempra_tir_lower::lower::lower_function_specialization(
                function,
                &context,
                &mut module,
                None,
                Some("a"),
                &[ty],
            )
            .unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            ids.push(concrete.id);
            module.add_function(concrete);
        }
        let compilation = crate::compilation::compile_tir_module(module.module, bindings).unwrap();
        assert_ne!(ids[0], ids[1]);
        assert_eq!(compilation.contract(ids[0]), compilation.contract(ids[1]));
        let contract = compilation.contract(ids[0]).unwrap();
        assert_eq!(contract.denied, CapabilitySet::empty());
        assert_eq!(contract.specialized_denied.len(), 1);
        assert_eq!(
            contract.specialized_denied[0].target,
            SpecializedDenyTarget::Operation(EffectOperation::FsWrite)
        );
        assert_eq!(
            contract.specialized_denied[0].parameters[0].value,
            SpecializedDenyValue::Exact(ResourceValue::Text("/private".to_owned()))
        );
        assert_eq!(contract.source, origin);
        assert_eq!(contract.declaration_span, function.span);
        assert_eq!(
            &source[contract.specialized_denied[0].span.start as usize
                ..contract.specialized_denied[0].span.end as usize],
            "fs.write(destination: \"/private\")"
        );
    }

    #[test]
    fn family_target_is_inherited_by_distinct_generic_specializations() {
        let source = "@deny(fs) fn work(type T, value: T) -> T { return value }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let origin = ContractSource::Module {
            name: "a".into(),
            path: PathBuf::from("a.tmpr"),
        };
        let mut module = ModuleBuilder::new();
        let mut bindings = ContractBindings::default();
        for ty in [tempra_hir::Type::Int, tempra_hir::Type::String] {
            let concrete = tempra_tir_lower::lower::lower_function_specialization(
                function,
                &context,
                &mut module,
                None,
                Some("a"),
                &[ty],
            )
            .unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
        }
        let contracts = bindings.finish();
        assert_eq!(contracts.len(), 2);
        assert_ne!(contracts[0].0, contracts[1].0);
        assert_eq!(contracts[0].1, contracts[1].1);
        let clause = &contracts[0].1.specialized_denied[0];
        assert_eq!(
            clause.target,
            SpecializedDenyTarget::Family(EffectFamily::Fs)
        );
        assert_eq!(
            &source[clause.span.start as usize..clause.span.end as usize],
            "fs"
        );
    }

    #[test]
    fn specialized_contracts_inside_one_declaration_deduplicate_canonically() {
        let source = "@deny(fs.write(destination: \"/x\"), fs.write(destination: \"/x\"), net.connect(host: \"h\", port: 1), net.connect(port: 1, host: \"h\")) fn f() -> Int64 { return 1 }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let origin = ContractSource::InMemory;
        let mut bindings = ContractBindings::default();
        bindings
            .register(TirFunctionId(0), function, &origin)
            .unwrap();
        let contracts = bindings.finish();
        assert_eq!(contracts.len(), 1);
        let contract = &contracts[0].1;
        assert_eq!(contract.specialized_denied.len(), 2);
        assert_eq!(
            contract
                .specialized_denied
                .iter()
                .map(|clause| clause.target.operation().unwrap())
                .collect::<Vec<_>>(),
            vec![EffectOperation::FsWrite, EffectOperation::NetConnect]
        );
        assert_eq!(
            &source[contract.specialized_denied[0].span.start as usize
                ..contract.specialized_denied[0].span.end as usize],
            "fs.write(destination: \"/x\")"
        );
    }

    #[test]
    fn each_specialization_enforces_its_own_id_with_inherited_provenance() {
        let source = "@deny(alloc) fn work(type T, value: T) -> T { let evidence = \"allocation\"; return value }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let origin = ContractSource::Module {
            name: "a".into(),
            path: PathBuf::from("a.tmpr"),
        };
        let mut module = ModuleBuilder::new();
        let mut bindings = ContractBindings::default();
        let mut ids = Vec::new();
        for ty in [tempra_hir::Type::Int, tempra_hir::Type::String] {
            let concrete = tempra_tir_lower::lower::lower_function_specialization(
                function,
                &context,
                &mut module,
                None,
                Some("a"),
                &[ty],
            )
            .unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            ids.push(concrete.id);
            module.add_function(concrete);
        }
        let analysis = tempra_effects::analyze(&module.module).unwrap();
        let crate::DriverError::CapabilityContractViolations(errors) =
            crate::compilation::compile_tir_module(module.module, bindings).unwrap_err()
        else {
            panic!("expected specialization violations")
        };
        assert_ne!(ids[0], ids[1]);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].contract, errors[1].contract);
        for (id, error) in ids.into_iter().zip(errors) {
            assert_eq!(error.function, id);
            assert_eq!(error.contract.source, origin);
            assert_eq!(error.contract.declaration_span, function.span);
            assert_eq!(
                error.witness,
                analysis.witness(id, Capability::Alloc).unwrap()
            );
        }
    }

    #[test]
    fn concrete_int_and_string_specializations_keep_independent_bindings() {
        let source = "@deny(alloc) fn work(type T, value: T) -> T { return value }";
        let ast = tempra_parser::parse_source(source).unwrap();
        let mut hir = tempra_hir::lower_file(source, &ast);
        let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
        let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
            panic!()
        };
        let origin = ContractSource::Module {
            name: "a".into(),
            path: PathBuf::from("a.tmpr"),
        };
        let mut module = ModuleBuilder::new();
        let mut bindings = ContractBindings::default();
        let mut ids = Vec::new();
        // Directly exercise the existing specialization lowerer. Multiple source
        // callback specializations can hit a separate bytecode ID/index mismatch.
        for ty in [tempra_hir::Type::Int, tempra_hir::Type::String] {
            let concrete = tempra_tir_lower::lower::lower_function_specialization(
                function,
                &context,
                &mut module,
                None,
                Some("a"),
                &[ty],
            )
            .unwrap();
            bindings.register(concrete.id, function, &origin).unwrap();
            // Repeated registration of the same declaration is idempotent.
            bindings.register(concrete.id, function, &origin).unwrap();
            ids.push(concrete.id);
            module.add_function(concrete);
        }
        assert_ne!(ids[0], ids[1]);

        let other_source = format!("{}fn work() {{}}", " ".repeat(function.span.start as usize));
        let other_ast = tempra_parser::parse_source(&other_source).unwrap();
        let mut other_hir = tempra_hir::lower_file(&other_source, &other_ast);
        let other_context = tempra_typeck::check_file_with_context(&mut other_hir).unwrap();
        let tempra_hir::HirItem::Function(other) = &other_hir.items[0] else {
            panic!()
        };
        assert_eq!(function.span.start, other.span.start);
        let concrete = tempra_tir_lower::lower::lower_function(
            other,
            &other_context,
            &mut module,
            None,
            Some("b"),
        )
        .unwrap();
        let other_id = concrete.id;
        bindings
            .register(
                other_id,
                other,
                &ContractSource::Module {
                    name: "b".into(),
                    path: PathBuf::from("b.tmpr"),
                },
            )
            .unwrap();
        module.add_function(concrete);
        let compilation = crate::compilation::compile_tir_module(module.module, bindings).unwrap();
        assert_eq!(compilation.contracts().count(), 2);
        assert_eq!(compilation.contract(ids[0]), compilation.contract(ids[1]));
        let contract = compilation.contract(ids[0]).unwrap();
        assert_eq!(contract.source, origin);
        assert_eq!(contract.declaration_span, function.span);
        assert_eq!(
            contract.attribute_spans,
            function.deny_contract.as_ref().unwrap().attribute_spans
        );
        assert_eq!(contract.denied, CapabilitySet::singleton(Capability::Alloc));
        assert!(compilation.contract(other_id).is_none());
        for (name, value) in [
            ("a.work$Int", tempra_vm::VmValue::Int64(7)),
            ("a.work$String", tempra_vm::VmValue::String("test".into())),
        ] {
            assert_eq!(
                tempra_vm::run_function(compilation.bytecode(), name, vec![value.clone()]).unwrap(),
                value
            );
        }
    }
}
