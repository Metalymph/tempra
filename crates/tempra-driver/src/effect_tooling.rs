//! Stable diagnostic and inspection projections over capability facts.
//!
//! These types do not analyze TIR. They join already-computed E1 summaries,
//! E3/E4 contracts, deterministic witnesses, and TIR display metadata.

use crate::{
    contracts::PURE_DENIED, CapabilityContractViolation, ContractSource,
    FunctionCapabilityContract, VmCompilation,
};
use std::collections::HashMap;
use tempra_diagnostics::{Diagnostic, Label, Severity};
use tempra_effects::{
    Capability, CapabilitySet, CapabilityWitness, DirectCapabilitySource, TirLocation,
};
use tempra_tir::ir::{TirFunctionId, TirModule};

/// Stable diagnostic code for a denied capability requirement.
pub const CAPABILITY_DENIED_CODE: &str = "capability-denied";

/// Returns the canonical lowercase tooling spelling for a capability.
pub const fn capability_name(capability: Capability) -> &'static str {
    match capability {
        Capability::Alloc => "alloc",
        Capability::Panic => "panic",
        Capability::Io => "io",
        Capability::Process => "process",
        Capability::Spawn => "spawn",
        Capability::Nondeterminism => "nondeterminism",
        Capability::Unsafe => "unsafe",
    }
}

/// The source construct contributing one denied capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectContractOrigin {
    /// The capability was written in an explicit `@deny(...)` contract.
    Deny,
    /// The capability is part of the canonical `@pure` restriction set.
    Pure,
}

/// Authoritative TIR identity plus optional human-facing metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectFunctionIdentity {
    /// Compilation-local identity used for every semantic join.
    pub id: TirFunctionId,
    /// TIR function name used only for presentation.
    pub display_name: String,
}

/// One named internal call in an existing E1 witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectWitnessCall {
    /// Calling function identity.
    pub caller: EffectFunctionIdentity,
    /// Called function identity.
    pub callee: EffectFunctionIdentity,
    /// Structural TIR callsite; no source span is implied.
    pub callsite: TirLocation,
}

/// Stable semantic kind of a direct capability origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectDirectOriginKind {
    /// A string constant materialized by the current runtime representation.
    StringConstant,
    /// A TIR aggregate construction.
    Make,
    /// A TIR struct construction.
    MakeStruct,
    /// A conservative may-panic division.
    Division,
    /// A potentially overflowing checked `Int64` addition.
    IntegerAddition,
    /// A potentially overflowing checked `Int64` subtraction or negation.
    IntegerSubtraction,
    /// A potentially overflowing checked `Int64` multiplication.
    IntegerMultiplication,
    /// A conservative may-panic unwrap.
    Unwrap,
    /// A constant-index access.
    ConstantIndexGet,
    /// A dynamic-index access.
    DynamicIndexGet,
    /// A slice operation.
    Slice,
    /// String byte-view materialization.
    Bytes,
    /// Functional array push.
    ArrayPush,
    /// A compiler-classified extern call.
    ExternCall,
}

/// Direct capability origin with the owning function's display metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectWitnessOrigin {
    /// Function containing the direct origin.
    pub function: EffectFunctionIdentity,
    /// Capability introduced at this origin.
    pub capability: Capability,
    /// Structural TIR location; no source span is implied.
    pub location: TirLocation,
    /// Stable semantic origin kind.
    pub kind: EffectDirectOriginKind,
    /// Called extern identity for [`EffectDirectOriginKind::ExternCall`].
    pub external: Option<EffectFunctionIdentity>,
}

/// Presentation-ready names joined to an unmodified E1 witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectWitness {
    /// Capability explained by this witness.
    pub capability: Capability,
    /// Named internal calls in causal order.
    pub calls: Vec<EffectWitnessCall>,
    /// Direct operation at the end of the path.
    pub origin: EffectWitnessOrigin,
}

/// One structured denied-capability diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectDiagnostic {
    /// Stable E6 diagnostic identifier.
    pub code: &'static str,
    /// Diagnostic severity. Contract violations are errors.
    pub severity: Severity,
    /// Function whose contract was violated.
    pub function: EffectFunctionIdentity,
    /// Capability denied by the contract and required by the function.
    pub capability: Capability,
    /// Source constructs contributing this particular restriction.
    pub contract_origins: Vec<EffectContractOrigin>,
    /// Complete retained declaration contract and source provenance.
    pub contract: FunctionCapabilityContract,
    /// Existing deterministic E1 witness with display names joined by ID.
    pub witness: EffectWitness,
}

/// One required capability and its deterministic explanation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectRequirementInfo {
    /// Required capability.
    pub capability: Capability,
    /// Existing E1 witness projected with display names.
    pub witness: EffectWitness,
}

/// Capability facts and source intent for one defined TIR function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEffectInfo {
    /// Authoritative function ID and display name.
    pub function: EffectFunctionIdentity,
    /// Capabilities introduced directly by this function.
    pub direct: CapabilitySet,
    /// Direct and transitively required capabilities.
    pub required: CapabilitySet,
    /// Explicit source contract, absent for unannotated functions.
    pub contract: Option<FunctionCapabilityContract>,
    /// One deterministic witness for every required capability.
    pub requirements: Vec<EffectRequirementInfo>,
}

/// Immutable E6 inspection result for one fresh VM compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectInspection {
    /// Defined functions in ascending [`TirFunctionId`] order.
    pub functions: Vec<FunctionEffectInfo>,
}

/// A mismatch between retained compiler facts during E6 projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectProjectionError {
    /// A summary, violation, or witness referenced a function absent from TIR.
    MissingFunction {
        /// Missing compilation-local function ID.
        function: TirFunctionId,
    },
    /// A required capability had no existing E1 witness.
    MissingWitness {
        /// Function requiring the capability.
        function: TirFunctionId,
        /// Capability without a witness.
        capability: Capability,
    },
    /// A violation could not be attributed to retained deny or pure intent.
    MissingContractOrigin {
        /// Function owning the inconsistent contract.
        function: TirFunctionId,
        /// Denied capability lacking source intent.
        capability: Capability,
    },
}

impl std::fmt::Display for EffectProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingFunction { function } => {
                write!(
                    formatter,
                    "missing TIR function {} during effect projection",
                    function.0
                )
            }
            Self::MissingWitness {
                function,
                capability,
            } => write!(
                formatter,
                "missing {} witness for function {} during effect projection",
                capability_name(*capability),
                function.0
            ),
            Self::MissingContractOrigin {
                function,
                capability,
            } => write!(
                formatter,
                "missing {} contract provenance for function {}",
                capability_name(*capability),
                function.0
            ),
        }
    }
}

impl std::error::Error for EffectProjectionError {}

/// Failure of the fresh E6 effect-inspection pipeline.
#[derive(Debug)]
pub enum EffectQueryError {
    /// Ordinary parsing, typing, lowering, analysis, or bytecode failure.
    Compiler(crate::DriverError),
    /// Capability contracts failed and were projected with retained TIR context.
    Diagnostics(Vec<EffectDiagnostic>),
    /// Existing facts were internally inconsistent during projection.
    Projection(EffectProjectionError),
}

impl std::fmt::Display for EffectQueryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compiler(error) => write!(formatter, "{error}"),
            Self::Diagnostics(diagnostics) => write!(
                formatter,
                "capability contract violation(s): {}",
                diagnostics.len()
            ),
            Self::Projection(error) => write!(formatter, "effect projection error: {error}"),
        }
    }
}

impl std::error::Error for EffectQueryError {}

fn function_names(module: &TirModule) -> HashMap<TirFunctionId, EffectFunctionIdentity> {
    module
        .functions
        .iter()
        .map(|function| {
            (
                function.id,
                EffectFunctionIdentity {
                    id: function.id,
                    display_name: function.name.clone(),
                },
            )
        })
        .collect()
}

fn identity(
    names: &HashMap<TirFunctionId, EffectFunctionIdentity>,
    function: TirFunctionId,
) -> Result<EffectFunctionIdentity, EffectProjectionError> {
    names
        .get(&function)
        .cloned()
        .ok_or(EffectProjectionError::MissingFunction { function })
}

fn project_witness(
    names: &HashMap<TirFunctionId, EffectFunctionIdentity>,
    witness: CapabilityWitness,
) -> Result<EffectWitness, EffectProjectionError> {
    let calls = witness
        .calls
        .iter()
        .map(|edge| {
            Ok(EffectWitnessCall {
                caller: identity(names, edge.caller)?,
                callee: identity(names, edge.callee)?,
                callsite: edge.callsite,
            })
        })
        .collect::<Result<Vec<_>, EffectProjectionError>>()?;
    let origin_function = identity(names, witness.origin.location.function)?;
    let (kind, external) = match &witness.origin.source {
        DirectCapabilitySource::StringConstant => (EffectDirectOriginKind::StringConstant, None),
        DirectCapabilitySource::Make => (EffectDirectOriginKind::Make, None),
        DirectCapabilitySource::MakeStruct => (EffectDirectOriginKind::MakeStruct, None),
        DirectCapabilitySource::Division => (EffectDirectOriginKind::Division, None),
        DirectCapabilitySource::IntegerAddition => (EffectDirectOriginKind::IntegerAddition, None),
        DirectCapabilitySource::IntegerSubtraction => {
            (EffectDirectOriginKind::IntegerSubtraction, None)
        }
        DirectCapabilitySource::IntegerMultiplication => {
            (EffectDirectOriginKind::IntegerMultiplication, None)
        }
        DirectCapabilitySource::Unwrap => (EffectDirectOriginKind::Unwrap, None),
        DirectCapabilitySource::ConstantIndexGet => {
            (EffectDirectOriginKind::ConstantIndexGet, None)
        }
        DirectCapabilitySource::DynamicIndexGet => (EffectDirectOriginKind::DynamicIndexGet, None),
        DirectCapabilitySource::Slice => (EffectDirectOriginKind::Slice, None),
        DirectCapabilitySource::Bytes => (EffectDirectOriginKind::Bytes, None),
        DirectCapabilitySource::ArrayPush => (EffectDirectOriginKind::ArrayPush, None),
        DirectCapabilitySource::ExternCall {
            extern_id, name, ..
        } => (
            EffectDirectOriginKind::ExternCall,
            Some(
                names
                    .get(extern_id)
                    .cloned()
                    .unwrap_or(EffectFunctionIdentity {
                        id: *extern_id,
                        display_name: name.clone(),
                    }),
            ),
        ),
    };
    Ok(EffectWitness {
        capability: witness.capability,
        calls,
        origin: EffectWitnessOrigin {
            function: origin_function,
            capability: witness.origin.capability,
            location: witness.origin.location,
            kind,
            external,
        },
    })
}

fn contract_origins(
    function: TirFunctionId,
    capability: Capability,
    contract: &FunctionCapabilityContract,
) -> Result<Vec<EffectContractOrigin>, EffectProjectionError> {
    let mut origins = Vec::new();
    if contract.explicit_denied.contains(capability) {
        origins.push(EffectContractOrigin::Deny);
    }
    if contract.pure_span.is_some() && PURE_DENIED.contains(capability) {
        origins.push(EffectContractOrigin::Pure);
    }
    if origins.is_empty() {
        Err(EffectProjectionError::MissingContractOrigin {
            function,
            capability,
        })
    } else {
        Ok(origins)
    }
}

/// Projects existing typed contract violations without recomputing capability facts.
pub fn project_effect_diagnostics(
    module: &TirModule,
    violations: &[CapabilityContractViolation],
) -> Result<Vec<EffectDiagnostic>, EffectProjectionError> {
    let names = function_names(module);
    violations
        .iter()
        .map(|violation| {
            Ok(EffectDiagnostic {
                code: CAPABILITY_DENIED_CODE,
                severity: Severity::Error,
                function: identity(&names, violation.function)?,
                capability: violation.capability,
                contract_origins: contract_origins(
                    violation.function,
                    violation.capability,
                    &violation.contract,
                )?,
                contract: violation.contract.clone(),
                witness: project_witness(&names, violation.witness.clone())?,
            })
        })
        .collect()
}

impl EffectDiagnostic {
    /// Converts this effect-specific diagnostic to Tempra's general diagnostic model.
    ///
    /// The effect witness remains structured on `self`; the general diagnostic
    /// receives only a concise textual note for existing consumers.
    pub fn to_diagnostic(&self) -> Diagnostic {
        let capability = capability_name(self.capability);
        let deny = self.contract_origins.contains(&EffectContractOrigin::Deny);
        let pure = self.contract_origins.contains(&EffectContractOrigin::Pure);
        let message = match (deny, pure) {
            (true, true) => format!(
                "function `{}` requires capability `{capability}`, denied by @deny and @pure",
                self.function.display_name
            ),
            (true, false) => format!(
                "function `{}` violates @deny({capability})",
                self.function.display_name
            ),
            (false, true) => format!(
                "pure function `{}` requires capability `{capability}`",
                self.function.display_name
            ),
            (false, false) => unreachable!("effect diagnostics require contract provenance"),
        };
        let primary_span = if deny {
            self.contract
                .attribute_spans
                .first()
                .copied()
                .unwrap_or(self.contract.declaration_span)
        } else {
            self.contract
                .pure_span
                .unwrap_or(self.contract.declaration_span)
        };
        let mut labels = Vec::new();
        if deny {
            labels.extend(
                self.contract
                    .attribute_spans
                    .iter()
                    .copied()
                    .map(|span| Label {
                        span,
                        message: Some("explicit @deny contract provenance".into()),
                    }),
            );
        }
        if pure {
            if let Some(span) = self.contract.pure_span {
                labels.push(Label {
                    span,
                    message: Some("@pure contract provenance".into()),
                });
            }
        }
        if labels.is_empty() {
            labels.push(Label {
                span: primary_span,
                message: None,
            });
        }
        Diagnostic {
            severity: self.severity,
            code: Some(self.code.into()),
            message,
            span: primary_span,
            labels,
            notes: vec![format!("required through: {}", self.witness_path())],
            file_path: match &self.contract.source {
                ContractSource::InMemory => None,
                ContractSource::Module { path, .. } => Some(path.to_string_lossy().into_owned()),
            },
        }
    }

    /// Returns a concise deterministic function-to-origin witness path.
    pub fn witness_path(&self) -> String {
        let mut names = vec![self.function.display_name.clone()];
        for call in &self.witness.calls {
            if names.last() != Some(&call.callee.display_name) {
                names.push(call.callee.display_name.clone());
            }
        }
        if names.last() != Some(&self.witness.origin.function.display_name) {
            names.push(self.witness.origin.function.display_name.clone());
        }
        if let Some(external) = &self.witness.origin.external {
            names.push(external.display_name.clone());
        }
        names.push(capability_name(self.capability).into());
        names.join(" -> ")
    }
}

pub(crate) fn inspect_compilation(
    compilation: &VmCompilation,
) -> Result<EffectInspection, EffectProjectionError> {
    let names = function_names(compilation.tir());
    let mut functions = Vec::new();
    for summary in compilation.capabilities().summaries() {
        let requirements = summary
            .required
            .iter()
            .map(|capability| {
                let witness = compilation
                    .capabilities()
                    .witness(summary.function, capability)
                    .ok_or(EffectProjectionError::MissingWitness {
                        function: summary.function,
                        capability,
                    })?;
                Ok(EffectRequirementInfo {
                    capability,
                    witness: project_witness(&names, witness)?,
                })
            })
            .collect::<Result<Vec<_>, EffectProjectionError>>()?;
        functions.push(FunctionEffectInfo {
            function: identity(&names, summary.function)?,
            direct: summary.direct,
            required: summary.required,
            contract: compilation.contract(summary.function).cloned(),
            requirements,
        });
    }
    Ok(EffectInspection { functions })
}
