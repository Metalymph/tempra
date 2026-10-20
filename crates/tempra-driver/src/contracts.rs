//! Declaration contracts and their compilation-local binding.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;
use tempra_effects::{Capability, CapabilitySet, EffectOperation, EffectParameter, ResourceValue};
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
/// Clauses are sorted by operation taxonomy order and then by canonical
/// parameter order. Duplicate clauses are removed while retaining the first
/// source occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedDenyClause {
    /// Canonical operation from `tempra-effects`.
    pub operation: EffectOperation,
    /// Parameters in canonical taxonomy order.
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

/// A parsed specialized target that this milestone does not yet enforce.
///
/// K4.9A adds family, category and new network targets to the syntax and
/// taxonomy. Driver binding, matching and enforcement remain at v0.1 until
/// K4.9B–K4.9D. This error prevents newly parsed contracts from being silently
/// treated as enforced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedSpecializedTarget {
    /// Canonical target spelling from the AST.
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

/// Maps only the v0.1 operation targets that the current matcher can enforce.
///
/// `net.send` and `net.receive` exist in the K4.9A taxonomy and syntax but are
/// deliberately rejected at this boundary until K4.9B–K4.9D bind and enforce
/// them.
fn canonical_enforceable_operation(
    name: tempra_ast::SpecializedOperation,
) -> Option<EffectOperation> {
    use tempra_ast::SpecializedOperation;
    match name {
        SpecializedOperation::FsRead => Some(EffectOperation::FsRead),
        SpecializedOperation::FsWrite => Some(EffectOperation::FsWrite),
        SpecializedOperation::FsRemove => Some(EffectOperation::FsRemove),
        SpecializedOperation::NetConnect => Some(EffectOperation::NetConnect),
        SpecializedOperation::NetListen => Some(EffectOperation::NetListen),
        SpecializedOperation::IoRead => Some(EffectOperation::IoRead),
        SpecializedOperation::IoWrite => Some(EffectOperation::IoWrite),
        SpecializedOperation::ProcessExec => Some(EffectOperation::ProcessExec),
        SpecializedOperation::ProcessExit => Some(EffectOperation::ProcessExit),
        SpecializedOperation::SpawnTask => Some(EffectOperation::SpawnTask),
        SpecializedOperation::SpawnActor => Some(EffectOperation::SpawnActor),
        SpecializedOperation::NetSend | SpecializedOperation::NetReceive => None,
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

fn operation_order(operation: EffectOperation) -> usize {
    EffectOperation::ALL
        .iter()
        .position(|candidate| *candidate == operation)
        .expect("EffectOperation::ALL is exhaustive")
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
    operation_order(left.operation)
        .cmp(&operation_order(right.operation))
        .then_with(|| compare_specialized_parameters(&left.parameters, &right.parameters))
}

fn canonicalize_specialized_denied(
    clauses: &[tempra_ast::SpecializedDenyClause],
) -> Result<Vec<SpecializedDenyClause>, UnsupportedSpecializedTarget> {
    let mut canonical = Vec::with_capacity(clauses.len());
    for clause in clauses {
        let operation = match clause.target {
            tempra_ast::SpecializedDenyTarget::Operation(operation) => {
                canonical_enforceable_operation(operation).ok_or_else(|| {
                    UnsupportedSpecializedTarget {
                        target: clause.target.canonical_name().to_owned(),
                        span: clause.span,
                    }
                })?
            }
            tempra_ast::SpecializedDenyTarget::Family(_)
            | tempra_ast::SpecializedDenyTarget::Category(_) => {
                return Err(UnsupportedSpecializedTarget {
                    target: clause.target.canonical_name().to_owned(),
                    span: clause.span,
                });
            }
        };
        canonical.push(SpecializedDenyClause {
            operation,
            parameters: clause
                .parameters
                .iter()
                .map(|parameter| SpecializedDenyParameter {
                    parameter: canonical_parameter(parameter.parameter),
                    value: canonical_specialized_value(&parameter.value),
                    span: parameter.span,
                })
                .collect(),
            span: clause.span,
        });
    }

    for clause in &mut canonical {
        clause
            .parameters
            .sort_by_key(|parameter| parameter_order(parameter.parameter));
    }

    // Stable sorting keeps the first source occurrence of an identical clause.
    canonical.sort_by(compare_specialized_clauses);
    canonical.dedup_by(|left, right| {
        left.operation == right.operation
            && left.parameters.len() == right.parameters.len()
            && left
                .parameters
                .iter()
                .zip(&right.parameters)
                .all(|(left, right)| left.parameter == right.parameter && left.value == right.value)
    });
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
                specialized_denied = canonicalize_specialized_denied(&syntax.specialized_denied)
                    .map_err(|error| {
                        crate::DriverError::UnsupportedSpecializedTarget(Box::new(error))
                    })?;
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
            contract.specialized_denied[0].operation,
            EffectOperation::FsWrite
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
                .map(|clause| clause.operation)
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
