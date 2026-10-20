//! Deterministic matching of canonical specialized denials against local facts.
//!
//! This module is deliberately independent of TIR, call-graph propagation and
//! diagnostics. It only compares one canonical [`SpecializedDenyClause`] with one
//! propagated [`LocalEffectFact`]. The driver uses the same classifier for direct
//! and transitive facts so there is no second matching or call-graph path.

use crate::contracts::{SpecializedDenyClause, SpecializedDenyParameter, SpecializedDenyValue};
use tempra_effects::{EffectParameter, LocalEffectFact, ResourceFact, ResourceKnowledge};

/// Result of matching one specialized denial against one observed effect fact.
///
/// The declaration order is also the canonical ordering when several facts for
/// the same origin need a deterministic classification tie-break. Only
/// [`Self::Disjoint`] is considered safe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpecializedMatch {
    /// The fact cannot satisfy the denied operation/resource.
    Disjoint,
    /// The fact satisfies every denied operation and resource exactly.
    DefiniteMatch,
    /// The fact may satisfy the denial, but required information is unknown or
    /// the operation could not be classified. This is fail-closed: it is still a
    /// violation.
    PotentialMatch,
}

impl SpecializedMatch {
    /// Returns whether this result requires reporting a violation.
    pub const fn is_violation(self) -> bool {
        !matches!(self, Self::Disjoint)
    }
}

/// Matches one canonical deny clause against one propagated local effect fact.
///
/// Matching is conservative and side-effect free:
///
/// - known operations are disjoint unless their identity is equal;
/// - a wildcard parameter matches every observation, including unknown ones;
/// - an exact parameter matches equal known values and is disjoint from unequal
///   known values;
/// - an unknown or absent observation against an exact restriction is a
///   potential match, never silent safety;
/// - a known value whose kind does not match its canonical parameter is treated
///   as potential metadata drift rather than proof of a disjoint effect;
/// - an unclassified operation matches the denied operation's parent capability
///   potentially and is disjoint from every other parent.
pub fn match_specialized_clause(
    clause: &SpecializedDenyClause,
    fact: &LocalEffectFact,
) -> SpecializedMatch {
    match fact {
        LocalEffectFact::Specialized(observed) => {
            if observed.operation != clause.operation {
                return SpecializedMatch::Disjoint;
            }

            let mut potential = false;
            for denied in &clause.parameters {
                match match_parameter(denied, &observed.resources) {
                    SpecializedMatch::Disjoint => return SpecializedMatch::Disjoint,
                    SpecializedMatch::PotentialMatch => potential = true,
                    SpecializedMatch::DefiniteMatch => {}
                }
            }
            if potential {
                SpecializedMatch::PotentialMatch
            } else {
                SpecializedMatch::DefiniteMatch
            }
        }
        LocalEffectFact::UnclassifiedOperation(observed) => {
            if observed.capability == clause.operation.parent() {
                // The operation is unknown. Even a parameterless denial cannot be
                // proved disjoint because the unknown operation may be the denied
                // one, so this remains a fail-closed potential match.
                SpecializedMatch::PotentialMatch
            } else {
                SpecializedMatch::Disjoint
            }
        }
    }
}

fn match_parameter(
    denied: &SpecializedDenyParameter,
    observed: &[ResourceFact],
) -> SpecializedMatch {
    let mut matching = observed
        .iter()
        .filter(|resource| resource.parameter == denied.parameter);
    let Some(resource) = matching.next() else {
        // An absent parameter is never evidence of safety. A wildcard still
        // restricts every value of the operation, while an exact restriction
        // cannot be proved and must remain a potential violation.
        return match denied.value {
            SpecializedDenyValue::Any => SpecializedMatch::DefiniteMatch,
            SpecializedDenyValue::Exact(_) => SpecializedMatch::PotentialMatch,
        };
    };

    if matching.next().is_some() {
        // Compiler-owned metadata promises exactly one entry per parameter.
        // Preserve fail-closed behavior rather than selecting an arbitrary one.
        return match denied.value {
            SpecializedDenyValue::Any => SpecializedMatch::DefiniteMatch,
            SpecializedDenyValue::Exact(_) => SpecializedMatch::PotentialMatch,
        };
    }

    match (&denied.value, &resource.knowledge) {
        (SpecializedDenyValue::Any, _) => SpecializedMatch::DefiniteMatch,
        (SpecializedDenyValue::Exact(expected), ResourceKnowledge::Exact(actual)) => {
            if !resource_value_matches(denied.parameter, expected)
                || !resource_value_matches(denied.parameter, actual)
            {
                // A value of the wrong kind for its canonical parameter is
                // compiler-owned metadata drift, not proof of a disjoint effect.
                SpecializedMatch::PotentialMatch
            } else if expected == actual {
                SpecializedMatch::DefiniteMatch
            } else {
                SpecializedMatch::Disjoint
            }
        }
        (SpecializedDenyValue::Exact(_), ResourceKnowledge::Unknown) => {
            SpecializedMatch::PotentialMatch
        }
    }
}

fn resource_value_matches(
    parameter: EffectParameter,
    value: &tempra_effects::ResourceValue,
) -> bool {
    use tempra_effects::ResourceValue;
    match parameter {
        EffectParameter::Port => matches!(value, ResourceValue::Port(_)),
        EffectParameter::Source
        | EffectParameter::Destination
        | EffectParameter::Host
        | EffectParameter::Executable
        | EffectParameter::Stream => matches!(value, ResourceValue::Text(_)),
        EffectParameter::Protocol | EffectParameter::Scope => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempra_effects::{
        Capability, EffectOperation, EffectParameter, ResourceValue, SpecializedOperationFact,
        TirLocation, UnclassifiedOperationFact,
    };
    use tempra_span::Span;
    use tempra_tir::ir::{TirBlockId, TirFunctionId};

    fn parameter(
        parameter: EffectParameter,
        value: SpecializedDenyValue,
    ) -> SpecializedDenyParameter {
        SpecializedDenyParameter {
            parameter,
            value,
            span: Span::new(0, 0),
        }
    }

    fn clause(
        operation: EffectOperation,
        parameters: Vec<SpecializedDenyParameter>,
    ) -> SpecializedDenyClause {
        SpecializedDenyClause {
            operation,
            parameters,
            span: Span::new(0, 0),
        }
    }

    fn resource(parameter: EffectParameter, knowledge: ResourceKnowledge) -> ResourceFact {
        ResourceFact {
            parameter,
            knowledge,
        }
    }

    fn specialized(operation: EffectOperation, resources: Vec<ResourceFact>) -> LocalEffectFact {
        LocalEffectFact::Specialized(SpecializedOperationFact {
            operation,
            resources,
            location: TirLocation {
                function: TirFunctionId(1),
                block: TirBlockId(0),
                instruction_index: 2,
            },
            extern_id: TirFunctionId(100),
            extern_name: format!("test.{}", operation.canonical_name()),
        })
    }

    fn unclassified(capability: Capability) -> LocalEffectFact {
        LocalEffectFact::UnclassifiedOperation(UnclassifiedOperationFact {
            capability,
            location: TirLocation {
                function: TirFunctionId(1),
                block: TirBlockId(0),
                instruction_index: 2,
            },
            extern_id: TirFunctionId(100),
            extern_name: "test.unclassified".to_owned(),
        })
    }

    fn text(value: &str) -> ResourceKnowledge {
        ResourceKnowledge::Exact(ResourceValue::Text(value.to_owned()))
    }

    fn exact_text(value: &str) -> SpecializedDenyValue {
        SpecializedDenyValue::Exact(ResourceValue::Text(value.to_owned()))
    }

    #[test]
    fn exact_equal_resources_are_definite_matches() {
        let denied = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                exact_text("out.txt"),
            )],
        );
        let observed = specialized(
            EffectOperation::FsWrite,
            vec![resource(EffectParameter::Destination, text("out.txt"))],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::DefiniteMatch
        );
    }

    #[test]
    fn exact_unequal_resources_are_disjoint() {
        let denied = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                exact_text("out.txt"),
            )],
        );
        let observed = specialized(
            EffectOperation::FsWrite,
            vec![resource(EffectParameter::Destination, text("other.txt"))],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::Disjoint
        );
    }

    #[test]
    fn wrong_resource_kind_fails_closed() {
        let denied = clause(
            EffectOperation::NetConnect,
            vec![parameter(
                EffectParameter::Port,
                SpecializedDenyValue::Exact(ResourceValue::Port(5432)),
            )],
        );
        let observed = specialized(
            EffectOperation::NetConnect,
            vec![resource(
                EffectParameter::Port,
                ResourceKnowledge::Exact(ResourceValue::Text("5432".to_owned())),
            )],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::PotentialMatch
        );
    }

    #[test]
    fn wildcard_matches_exact_unknown_and_missing_observations() {
        let denied = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                SpecializedDenyValue::Any,
            )],
        );
        for observed in [
            specialized(
                EffectOperation::FsWrite,
                vec![resource(EffectParameter::Destination, text("out.txt"))],
            ),
            specialized(
                EffectOperation::FsWrite,
                vec![resource(
                    EffectParameter::Destination,
                    ResourceKnowledge::Unknown,
                )],
            ),
            specialized(EffectOperation::FsWrite, Vec::new()),
        ] {
            assert_eq!(
                match_specialized_clause(&denied, &observed),
                SpecializedMatch::DefiniteMatch
            );
        }
    }

    #[test]
    fn unknown_observed_resource_is_potential_against_exact() {
        let denied = clause(
            EffectOperation::FsRead,
            vec![parameter(EffectParameter::Source, exact_text("input.txt"))],
        );
        let observed = specialized(
            EffectOperation::FsRead,
            vec![resource(
                EffectParameter::Source,
                ResourceKnowledge::Unknown,
            )],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::PotentialMatch
        );
    }

    #[test]
    fn missing_observed_parameter_is_potential_against_exact() {
        let denied = clause(
            EffectOperation::FsRead,
            vec![parameter(EffectParameter::Source, exact_text("input.txt"))],
        );
        let observed = specialized(EffectOperation::FsRead, Vec::new());
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::PotentialMatch
        );
    }

    #[test]
    fn different_known_operations_are_disjoint_even_with_the_same_parent() {
        let denied = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                exact_text("out.txt"),
            )],
        );
        let observed = specialized(
            EffectOperation::FsRead,
            vec![resource(EffectParameter::Source, text("out.txt"))],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::Disjoint
        );
    }

    #[test]
    fn unclassified_operations_are_potential_only_for_the_denied_parent() {
        let denied = clause(EffectOperation::FsWrite, Vec::new());
        assert_eq!(
            match_specialized_clause(&denied, &unclassified(Capability::Io)),
            SpecializedMatch::PotentialMatch
        );
        assert_eq!(
            match_specialized_clause(&denied, &unclassified(Capability::Process)),
            SpecializedMatch::Disjoint
        );
        assert_eq!(
            match_specialized_clause(&denied, &unclassified(Capability::Spawn)),
            SpecializedMatch::Disjoint
        );
    }

    #[test]
    fn multiple_parameters_combine_conjunctively() {
        let denied = clause(
            EffectOperation::NetConnect,
            vec![
                parameter(EffectParameter::Host, exact_text("db.internal")),
                parameter(
                    EffectParameter::Port,
                    SpecializedDenyValue::Exact(ResourceValue::Port(5432)),
                ),
                parameter(EffectParameter::Protocol, SpecializedDenyValue::Any),
            ],
        );
        let matching = specialized(
            EffectOperation::NetConnect,
            vec![
                resource(EffectParameter::Host, text("db.internal")),
                resource(
                    EffectParameter::Port,
                    ResourceKnowledge::Exact(ResourceValue::Port(5432)),
                ),
                resource(EffectParameter::Protocol, ResourceKnowledge::Unknown),
            ],
        );
        assert_eq!(
            match_specialized_clause(&denied, &matching),
            SpecializedMatch::DefiniteMatch
        );

        let one_disjoint = specialized(
            EffectOperation::NetConnect,
            vec![
                resource(EffectParameter::Host, text("db.internal")),
                resource(
                    EffectParameter::Port,
                    ResourceKnowledge::Exact(ResourceValue::Port(5433)),
                ),
                resource(EffectParameter::Protocol, ResourceKnowledge::Unknown),
            ],
        );
        assert_eq!(
            match_specialized_clause(&denied, &one_disjoint),
            SpecializedMatch::Disjoint
        );

        let one_unknown = specialized(
            EffectOperation::NetConnect,
            vec![
                resource(EffectParameter::Host, ResourceKnowledge::Unknown),
                resource(
                    EffectParameter::Port,
                    ResourceKnowledge::Exact(ResourceValue::Port(5432)),
                ),
                resource(EffectParameter::Protocol, ResourceKnowledge::Unknown),
            ],
        );
        assert_eq!(
            match_specialized_clause(&denied, &one_unknown),
            SpecializedMatch::PotentialMatch
        );
    }

    #[test]
    fn parameterless_clauses_restrict_the_whole_known_operation() {
        let denied = clause(EffectOperation::ProcessExit, Vec::new());
        let observed = specialized(EffectOperation::ProcessExit, Vec::new());
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::DefiniteMatch
        );
        assert_eq!(
            match_specialized_clause(&denied, &specialized(EffectOperation::FsWrite, Vec::new())),
            SpecializedMatch::Disjoint
        );
    }

    #[test]
    fn empty_clause_parameters_match_every_resource_of_the_operation() {
        let denied = clause(EffectOperation::FsWrite, Vec::new());
        let observed = specialized(
            EffectOperation::FsWrite,
            vec![resource(EffectParameter::Destination, text("out.txt"))],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::DefiniteMatch
        );
    }

    #[test]
    fn duplicate_observed_parameter_metadata_fails_closed() {
        let exact = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                exact_text("out.txt"),
            )],
        );
        let duplicate = specialized(
            EffectOperation::FsWrite,
            vec![
                resource(EffectParameter::Destination, text("out.txt")),
                resource(EffectParameter::Destination, text("other.txt")),
            ],
        );
        assert_eq!(
            match_specialized_clause(&exact, &duplicate),
            SpecializedMatch::PotentialMatch
        );

        let wildcard = clause(
            EffectOperation::FsWrite,
            vec![parameter(
                EffectParameter::Destination,
                SpecializedDenyValue::Any,
            )],
        );
        assert_eq!(
            match_specialized_clause(&wildcard, &duplicate),
            SpecializedMatch::DefiniteMatch
        );
    }

    #[test]
    fn extra_observed_parameters_do_not_narrow_the_denial() {
        let denied = clause(
            EffectOperation::NetConnect,
            vec![parameter(EffectParameter::Host, exact_text("db.internal"))],
        );
        let observed = specialized(
            EffectOperation::NetConnect,
            vec![
                resource(EffectParameter::Host, text("db.internal")),
                resource(
                    EffectParameter::Port,
                    ResourceKnowledge::Exact(ResourceValue::Port(5432)),
                ),
            ],
        );
        assert_eq!(
            match_specialized_clause(&denied, &observed),
            SpecializedMatch::DefiniteMatch
        );
    }
}
