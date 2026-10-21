//! Combines declaration intent with already-computed TIR capability facts.

use crate::specialized_matching::{match_specialized_clause, SpecializedMatch};
use crate::{DriverError, FunctionCapabilityContract, SpecializedDenyClause};
use std::collections::HashSet;
use tempra_effects::{
    Capability, CapabilityAnalysis, CapabilityWitness, LocalEffectFact, SpecializedEffectWitness,
};
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

/// One specialized denial matched against a directly or transitively reachable
/// operation origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecializedContractViolation {
    /// Concrete function ID in this compilation, never a name-based lookup.
    pub function: TirFunctionId,
    /// Canonical denied clause, retaining its declaration-site span.
    pub clause: SpecializedDenyClause,
    /// Matching propagated local-effect fact, retaining its original location.
    pub fact: LocalEffectFact,
    /// Definite or potential classification of this match.
    pub classification: SpecializedMatch,
    /// Deterministic shortest witness to the fact's original operation origin.
    pub witness: SpecializedEffectWitness,
    /// Original contract, including declaration, attribute and source provenance.
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
    /// A specialized contract has no defined-function effect summary.
    MissingSpecializedSummary {
        /// Function whose specialized summary is absent.
        function: TirFunctionId,
    },
    /// A matching specialized requirement has no reconstructible origin witness.
    MissingSpecializedWitness {
        /// Function whose specialized summary should contain the fact.
        function: TirFunctionId,
        /// Matching fact without a witness.
        fact: LocalEffectFact,
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

impl SpecializedContractViolation {
    fn from_witness(
        function: TirFunctionId,
        clause: &SpecializedDenyClause,
        fact: &LocalEffectFact,
        classification: SpecializedMatch,
        contract: &FunctionCapabilityContract,
        witness: Option<SpecializedEffectWitness>,
    ) -> Result<Self, CapabilityContractInvariantError> {
        Ok(Self {
            function,
            clause: clause.clone(),
            fact: fact.clone(),
            classification,
            witness: witness.ok_or(
                CapabilityContractInvariantError::MissingSpecializedWitness {
                    function,
                    fact: fact.clone(),
                },
            )?,
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
    let mut specialized_violations = Vec::new();
    for (function, contract) in contracts {
        if !functions.contains(function) {
            return Err(DriverError::CapabilityContractInvariantError(
                CapabilityContractInvariantError::MissingFunction {
                    function: *function,
                },
            ));
        }

        // Missing specialized metadata is a compiler invariant, never a reason to
        // treat the denial as satisfied. Check it before broad analysis so a
        // specialized-only contract on a bodyless declaration reports the
        // dedicated invariant.
        let specialized_summary = if contract.specialized_denied.is_empty() {
            None
        } else {
            Some(analysis.specialized_summary(*function).ok_or(
                DriverError::CapabilityContractInvariantError(
                    CapabilityContractInvariantError::MissingSpecializedSummary {
                        function: *function,
                    },
                ),
            )?)
        };

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

        let Some(specialized_summary) = specialized_summary else {
            continue;
        };
        for clause in &contract.specialized_denied {
            let mut matches: Vec<(&LocalEffectFact, SpecializedMatch)> = specialized_summary
                .reachable()
                .filter_map(|fact| {
                    let classification = match_specialized_clause(clause, fact);
                    classification
                        .is_violation()
                        .then_some((fact, classification))
                })
                .collect();
            // The canonical clause order comes from the contract; within a clause
            // order by the original fact location and then the classifier.
            matches.sort_by(|(left_fact, left_match), (right_fact, right_match)| {
                local_fact_order(left_fact)
                    .cmp(&local_fact_order(right_fact))
                    .then_with(|| left_match.cmp(right_match))
            });

            for (fact, classification) in matches {
                specialized_violations.push(
                    SpecializedContractViolation::from_witness(
                        *function,
                        clause,
                        fact,
                        classification,
                        contract,
                        analysis.specialized_witness(*function, fact),
                    )
                    .map_err(DriverError::CapabilityContractInvariantError)?,
                );
            }
        }
    }

    // Broad denials subsume every specialized refinement of their capability.
    // Return the unchanged broad variant first when both categories are present.
    if !violations.is_empty() {
        return Err(DriverError::CapabilityContractViolations(violations));
    }

    if !specialized_violations.is_empty() {
        specialized_violations.sort_by(|left, right| {
            left.function
                .0
                .cmp(&right.function.0)
                .then_with(|| {
                    clause_order(&left.contract, &left.clause)
                        .cmp(&clause_order(&right.contract, &right.clause))
                })
                .then_with(|| local_fact_order(&left.fact).cmp(&local_fact_order(&right.fact)))
                .then_with(|| left.classification.cmp(&right.classification))
        });
        // Identical function/clause/fact/classification/witness/contract entries
        // are the only duplicates removed. Keeping this explicit avoids dropping
        // independent origins that share operation and resource values.
        let mut unique = Vec::with_capacity(specialized_violations.len());
        for violation in specialized_violations {
            if !unique.contains(&violation) {
                unique.push(violation);
            }
        }
        return Err(DriverError::SpecializedContractViolations(unique));
    }

    Ok(())
}

fn clause_order(contract: &FunctionCapabilityContract, clause: &SpecializedDenyClause) -> usize {
    contract
        .specialized_denied
        .iter()
        .position(|candidate| candidate == clause)
        .unwrap_or(usize::MAX)
}

fn local_fact_order(fact: &LocalEffectFact) -> (usize, usize, usize, u8, u8) {
    let location = fact.location();
    let (variant, detail) = match fact {
        LocalEffectFact::Specialized(fact) => (0, fact.operation as u8),
        LocalEffectFact::UnclassifiedOperation(fact) => (1, fact.capability as u8),
    };
    (
        location.function.0,
        location.block.0,
        location.instruction_index,
        variant,
        detail,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::{SpecializedDenyParameter, SpecializedDenyValue};
    use crate::{ContractSource, SpecializedDenyTarget};
    use tempra_effects::{
        CapabilitySet, EffectOperation, EffectParameter, ResourceKnowledge, TirLocation,
    };
    use tempra_span::Span;
    use tempra_tir::{
        builder::ModuleBuilder,
        ir::{TirBlockId, TirType},
    };

    fn contract() -> FunctionCapabilityContract {
        FunctionCapabilityContract {
            explicit_denied: CapabilitySet::singleton(Capability::Io),
            denied: CapabilitySet::singleton(Capability::Io),
            specialized_denied: Vec::new(),
            declaration_span: Span::new(0, 1),
            attribute_spans: vec![Span::new(0, 1)],
            pure_span: None,
            source: ContractSource::InMemory,
        }
    }

    fn specialized_contract() -> FunctionCapabilityContract {
        let mut contract = contract();
        contract.explicit_denied = CapabilitySet::empty();
        contract.denied = CapabilitySet::empty();
        contract.specialized_denied = vec![SpecializedDenyClause {
            target: SpecializedDenyTarget::Operation(EffectOperation::FsWrite),
            parameters: vec![SpecializedDenyParameter {
                parameter: EffectParameter::Destination,
                value: SpecializedDenyValue::Any,
                span: Span::new(0, 1),
            }],
            span: Span::new(0, 1),
        }];
        contract
    }

    fn specialized_fact() -> LocalEffectFact {
        LocalEffectFact::Specialized(tempra_effects::SpecializedOperationFact {
            operation: EffectOperation::FsWrite,
            resources: vec![tempra_effects::ResourceFact {
                parameter: EffectParameter::Destination,
                knowledge: ResourceKnowledge::Unknown,
            }],
            location: TirLocation {
                function: TirFunctionId(0),
                block: TirBlockId(0),
                instruction_index: 0,
            },
            extern_id: TirFunctionId(100),
            extern_name: "std.fs.write_text".to_owned(),
        })
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

    #[test]
    fn missing_specialized_summary_is_an_explicit_invariant() {
        let mut builder = ModuleBuilder::new();
        let declaration = builder.new_function("declaration", TirType::Unit).build();
        let declared_id = declaration.id;
        builder.add_function(declaration);
        let analysis = tempra_effects::analyze(&builder.module).unwrap();
        match enforce_contracts(
            &builder.module,
            &analysis,
            &[(declared_id, specialized_contract())],
        )
        .unwrap_err()
        {
            DriverError::CapabilityContractInvariantError(
                CapabilityContractInvariantError::MissingSpecializedSummary { function },
            ) => assert_eq!(function, declared_id),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn missing_specialized_witness_is_never_synthesized() {
        let clause = specialized_contract().specialized_denied[0].clone();
        let fact = specialized_fact();
        assert_eq!(
            SpecializedContractViolation::from_witness(
                TirFunctionId(0),
                &clause,
                &fact,
                SpecializedMatch::PotentialMatch,
                &specialized_contract(),
                None,
            ),
            Err(
                CapabilityContractInvariantError::MissingSpecializedWitness {
                    function: TirFunctionId(0),
                    fact: fact.clone(),
                }
            )
        );
    }
}
