//! Declaration contracts and their compilation-local binding, without enforcement.

use std::collections::HashMap;
use std::path::PathBuf;
use tempra_effects::{Capability, CapabilitySet};
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

/// Explicit user intent attached to one concrete TIR function.
/// E3.1 retains this data but does not compare it with required capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionCapabilityContract {
    /// Canonical set obtained from validated public deny spellings.
    pub denied: CapabilitySet,
    /// Existing function declaration span, excluding preceding attributes.
    pub declaration_span: Span,
    /// Complete spans of the merged attributes, in source order.
    pub attribute_spans: Vec<Span>,
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

// This is the sole frontend-to-canonical capability conversion. Neither AST nor
// HIR imports analyzer types or reasons about capability requirements.
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
        let contract = declaration.deny_contract.as_ref().map(|syntax| {
            let mut denied = CapabilitySet::empty();
            for name in &syntax.denied {
                denied.insert(canonical_capability(*name));
            }
            FunctionCapabilityContract {
                denied,
                declaration_span: declaration.span,
                attribute_spans: syntax.attribute_spans.clone(),
                source: source.clone(),
            }
        });
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
