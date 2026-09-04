//! Combines declaration intent with already-computed TIR capability facts.

use crate::{DriverError, FunctionCapabilityContract};
use std::collections::HashSet;
use tempra_effects::{Capability, CapabilityAnalysis, CapabilityWitness};
use tempra_tir::ir::{TirFunctionId, TirModule};

/// One denied transitive requirement, retaining the analyzer's canonical witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityContractViolation {
    /// Concrete function ID in this compilation, never a name-based lookup.
    pub function: TirFunctionId,
    /// Capability both denied by the declaration and required by the function.
    pub capability: Capability,
    /// Unmodified E1 witness, including the direct origin and call path.
    pub witness: CapabilityWitness,
    /// Original contract and declaration/attribute source provenance.
    pub contract: FunctionCapabilityContract,
}

/// An inconsistent compiler product must never be interpreted as a satisfied contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityContractInvariantError {
    /// The contract does not identify a function in the retained TIR.
    MissingFunction {
        /// Contract's concrete function ID.
        function: TirFunctionId,
    },
    /// The function has no defined-function capability summary.
    MissingSummary {
        /// Function whose summary is absent.
        function: TirFunctionId,
    },
    /// A denied requirement has no reconstructible direct/transitive origin.
    MissingWitness {
        /// Function requiring the capability.
        function: TirFunctionId,
        /// Required capability without a witness.
        capability: Capability,
    },
}

impl CapabilityContractViolation {
    fn from_witness(
        function: TirFunctionId,
        capability: Capability,
        contract: &FunctionCapabilityContract,
        witness: Option<CapabilityWitness>,
    ) -> Result<Self, CapabilityContractInvariantError> {
        Ok(Self {
            function,
            capability,
            witness: witness.ok_or(CapabilityContractInvariantError::MissingWitness {
                function,
                capability,
            })?,
            contract: contract.clone(),
        })
    }
}

pub(crate) fn enforce_contracts(
    module: &TirModule,
    analysis: &CapabilityAnalysis,
    // ContractBindings::finish supplies ascending, unique function IDs.
    contracts: &[(TirFunctionId, FunctionCapabilityContract)],
) -> Result<(), DriverError> {
    let functions: HashSet<_> = module
        .functions
        .iter()
        .map(|function| function.id)
        .collect();
    let mut violations = Vec::new();
    for (function, contract) in contracts {
        if !functions.contains(function) {
            return Err(DriverError::CapabilityContractInvariantError(
                CapabilityContractInvariantError::MissingFunction {
                    function: *function,
                },
            ));
        }
        let summary =
            analysis
                .summary(*function)
                .ok_or(DriverError::CapabilityContractInvariantError(
                    CapabilityContractInvariantError::MissingSummary {
                        function: *function,
                    },
                ))?;
        // denied intersect required; required already includes transitive calls.
        for capability in contract.denied.iter() {
            if summary.required.contains(capability) {
                violations.push(
                    CapabilityContractViolation::from_witness(
                        *function,
                        capability,
                        contract,
                        analysis.witness(*function, capability),
                    )
                    .map_err(DriverError::CapabilityContractInvariantError)?,
                );
            }
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(DriverError::CapabilityContractViolations(violations))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ContractSource;
    use tempra_effects::CapabilitySet;
    use tempra_span::Span;
    use tempra_tir::{builder::ModuleBuilder, ir::TirType};

    fn contract() -> FunctionCapabilityContract {
        FunctionCapabilityContract {
            denied: CapabilitySet::singleton(Capability::Io),
            declaration_span: Span::new(0, 1),
            attribute_spans: vec![Span::new(0, 1)],
            pure_span: None,
            source: ContractSource::InMemory,
        }
    }

    #[test]
    fn missing_function_and_summary_are_invariant_errors() {
        let mut builder = ModuleBuilder::new();
        let declaration = builder.new_function("declaration", TirType::Unit).build();
        let declared_id = declaration.id;
        builder.add_function(declaration);
        let analysis = tempra_effects::analyze(&builder.module).unwrap();
        for (id, expected) in [
            (
                TirFunctionId(99),
                CapabilityContractInvariantError::MissingFunction {
                    function: TirFunctionId(99),
                },
            ),
            (
                declared_id,
                CapabilityContractInvariantError::MissingSummary {
                    function: declared_id,
                },
            ),
        ] {
            match enforce_contracts(&builder.module, &analysis, &[(id, contract())]).unwrap_err() {
                DriverError::CapabilityContractInvariantError(actual) => {
                    assert_eq!(actual, expected)
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }
    }

    #[test]
    fn absent_witness_is_never_dropped_or_synthesized() {
        // Test the checked construction boundary without corrupting E1's private facts.
        assert_eq!(
            CapabilityContractViolation::from_witness(
                TirFunctionId(0),
                Capability::Io,
                &contract(),
                None
            ),
            Err(CapabilityContractInvariantError::MissingWitness {
                function: TirFunctionId(0),
                capability: Capability::Io
            })
        );
    }
}
