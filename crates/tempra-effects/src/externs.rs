use crate::{Capability, CapabilitySet, EffectOperation, EffectParameter};

/// One declared resource parameter of a specialized-operation binding.
///
/// `argument_index` is the zero-based positional TIR argument that carries the
/// resource. `None` means the value is not available statically and must be
/// recorded as [`crate::ResourceKnowledge::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceBinding {
    pub(crate) parameter: EffectParameter,
    pub(crate) argument_index: Option<usize>,
}

impl ResourceBinding {
    /// Binds `parameter` to an explicit positional call argument.
    pub(crate) const fn argument(parameter: EffectParameter, argument_index: usize) -> Self {
        Self {
            parameter,
            argument_index: Some(argument_index),
        }
    }

    /// Declares `parameter` without a statically available value.
    pub(crate) const fn unknown(parameter: EffectParameter) -> Self {
        Self {
            parameter,
            argument_index: None,
        }
    }
}

/// A rejected native operation binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingError {
    /// The operation does not declare this parameter key.
    InvalidParameter {
        operation: EffectOperation,
        parameter: EffectParameter,
    },
    /// The same parameter is bound more than once.
    DuplicateParameter {
        operation: EffectOperation,
        parameter: EffectParameter,
    },
}

/// An explicit specialized-operation binding for a known native extern.
///
/// Bindings are validated and canonically ordered at construction. Resources are
/// sorted by [`EffectParameter::ALL`], so callers never observe declaration
/// order as semantic order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternOperationBinding {
    pub(crate) operation: EffectOperation,
    pub(crate) resources: Vec<ResourceBinding>,
}

impl ExternOperationBinding {
    /// Validates and canonicalizes a binding.
    ///
    /// Every parameter must be accepted by
    /// [`EffectOperation::valid_parameter_keys`], must not be duplicated, and is
    /// sorted by its position in [`EffectParameter::ALL`]. An empty resource list
    /// is valid for operations that declare no parameters.
    pub(crate) fn new(
        operation: EffectOperation,
        resources: impl IntoIterator<Item = ResourceBinding>,
    ) -> Result<Self, BindingError> {
        let mut resources: Vec<_> = resources.into_iter().collect();
        let mut seen = [false; EffectParameter::ALL.len()];

        for resource in &resources {
            if !operation.accepts_parameter(resource.parameter) {
                return Err(BindingError::InvalidParameter {
                    operation,
                    parameter: resource.parameter,
                });
            }
            let rank = parameter_rank(resource.parameter);
            if seen[rank] {
                return Err(BindingError::DuplicateParameter {
                    operation,
                    parameter: resource.parameter,
                });
            }
            seen[rank] = true;
        }

        resources.sort_by_key(|resource| parameter_rank(resource.parameter));
        Ok(Self {
            operation,
            resources,
        })
    }
}

fn parameter_rank(parameter: EffectParameter) -> usize {
    EffectParameter::ALL
        .iter()
        .position(|candidate| *candidate == parameter)
        .expect("EffectParameter::ALL is exhaustive")
}

/// One compiler-known extern as observed by analysis.
///
/// This is read-only inspection metadata for tests and tooling. It is not a
/// language declaration, does not grant access, and does not claim that the
/// corresponding native implementation is sandboxed. The operation identity and
/// resource bindings are compiler-owned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownExtern {
    /// Fully qualified extern name, for example `std.fs.read_text`.
    pub name: &'static str,
    /// Broad capabilities implied by calling the extern. An empty set is an
    /// explicit capability-free classification, not missing information.
    pub capabilities: CapabilitySet,
    /// Explicit specialized operation, or `None` for a known extern without a
    /// specialized classification. `None` never means "safe".
    pub operation: Option<EffectOperation>,
    /// Canonically ordered specialized resource bindings. Empty when there is
    /// no specialized operation.
    pub resources: Vec<KnownExternResource>,
}

/// One positional resource binding in [`KnownExtern`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownExternResource {
    /// The resource parameter key.
    pub parameter: EffectParameter,
    /// The zero-based call argument carrying the resource, or `None` when the
    /// value is not statically available.
    pub argument_index: Option<usize>,
}

struct ExternDefinition {
    name: &'static str,
    capabilities: CapabilitySet,
    operation: Option<EffectOperation>,
    resources: &'static [ResourceBinding],
}

const ALLOC: CapabilitySet = CapabilitySet::singleton(Capability::Alloc);
const IO: CapabilitySet = CapabilitySet::singleton(Capability::Io);
const IO_ALLOC: CapabilitySet = IO.union(ALLOC);

/// The single compiler-owned source of truth for known externs.
///
/// Every entry, including capability-free externs and externs without a
/// specialized operation, is explicit. Consistency tests compare this table with
/// the production native registry and the declared standard-library signatures.
const EXTERN_DEFINITIONS: &[ExternDefinition] = &[
    ExternDefinition {
        name: "std.fs.exists",
        capabilities: IO,
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.fs.read_text",
        capabilities: IO_ALLOC,
        operation: Some(EffectOperation::FsRead),
        resources: &[ResourceBinding::argument(EffectParameter::Source, 0)],
    },
    ExternDefinition {
        name: "std.fs.write_text",
        capabilities: IO_ALLOC,
        operation: Some(EffectOperation::FsWrite),
        resources: &[ResourceBinding::argument(EffectParameter::Destination, 0)],
    },
    ExternDefinition {
        name: "std.io.print",
        capabilities: IO,
        operation: Some(EffectOperation::IoWrite),
        resources: &[ResourceBinding::unknown(EffectParameter::Stream)],
    },
    ExternDefinition {
        name: "std.io.println",
        capabilities: IO,
        operation: Some(EffectOperation::IoWrite),
        resources: &[ResourceBinding::unknown(EffectParameter::Stream)],
    },
    ExternDefinition {
        name: "std.path.join",
        capabilities: ALLOC,
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.path.basename",
        capabilities: ALLOC,
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.strings.trim",
        capabilities: ALLOC,
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.strings.replace",
        capabilities: ALLOC,
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.strings.contains",
        capabilities: CapabilitySet::empty(),
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.strings.starts_with",
        capabilities: CapabilitySet::empty(),
        operation: None,
        resources: &[],
    },
    ExternDefinition {
        name: "std.strings.ends_with",
        capabilities: CapabilitySet::empty(),
        operation: None,
        resources: &[],
    },
];

fn definition_for_extern(name: &str) -> Option<&'static ExternDefinition> {
    EXTERN_DEFINITIONS
        .iter()
        .find(|definition| definition.name == name)
}

/// Iterates over every compiler-known extern in production declaration order.
///
/// This exposes the existing classifier metadata so the native boundary can be
/// tested without maintaining a second copy of the table. Unknown externs are
/// absent and remain fail-closed in analysis.
pub fn known_externs() -> impl Iterator<Item = KnownExtern> {
    EXTERN_DEFINITIONS.iter().map(|definition| KnownExtern {
        name: definition.name,
        capabilities: definition.capabilities,
        operation: definition.operation,
        resources: definition
            .resources
            .iter()
            .map(|resource| KnownExternResource {
                parameter: resource.parameter,
                argument_index: resource.argument_index,
            })
            .collect(),
    })
}

/// Builds one compiler-owned static binding, panicking on invalid metadata.
///
/// A panic here reports an internal compiler-metadata defect. It is deliberately
/// not a [`BindingError`] returned through analysis, so it can never become a
/// recoverable Tempra source diagnostic.
pub(crate) fn static_binding(
    operation: EffectOperation,
    resources: impl IntoIterator<Item = ResourceBinding>,
) -> ExternOperationBinding {
    ExternOperationBinding::new(operation, resources)
        .expect("compiler-owned extern operation binding must be valid")
}

/// Returns the explicit resource binding for a known extern.
///
/// Bindings were verified against the declared standard-library signatures in
/// `std/fs.tmpr` and `std/io.tmpr`, and against the native implementations in
/// `crates/tempra-native/src/std/fs.rs` and `io.rs`:
///
/// - `read_text(path: String) -> String!`: argument 0 is the `source` path.
/// - `write_text(path: String, text: String) -> Unit!`: argument 0 is the
///   `destination` path; argument 1 is file content, never a resource.
/// - `print(value: String)` / `println(value: String)`: there is no stream
///   argument. Argument 0 is printed content and must never be interpreted as
///   the stream, so `stream` stays [`crate::ResourceKnowledge::Unknown`].
///
/// `None` means the extern has no specialized operation binding, either because
/// it is genuinely unknown or because it is a known extern with no specialized
/// operation (for example `std.fs.exists`). It never represents invalid
/// compiler-owned metadata: that panics through [`static_binding`] instead of
/// degrading into a recoverable source diagnostic. The table is independently
/// checked by tests.
pub(crate) fn operation_binding_for_extern(name: &str) -> Option<ExternOperationBinding> {
    let definition = definition_for_extern(name)?;
    let operation = definition.operation?;
    Some(static_binding(
        operation,
        definition.resources.iter().copied(),
    ))
}

/// Returns the specialized operation for a known extern, if one is explicitly
/// classified.
///
/// This is declaration metadata only. It does not add to or relax
/// [`capabilities_for_extern`], propagate specialized facts, or claim runtime
/// availability. `None` means "no specialized classification", never "safe":
/// unknown externs remain [`CapabilityAnalysisError::UnclassifiedExtern`] when
/// called.
///
/// [`CapabilityAnalysisError::UnclassifiedExtern`]:
/// crate::CapabilityAnalysisError::UnclassifiedExtern
// Metadata/test helper; TIR extraction uses `operation_binding_for_extern`.
#[allow(dead_code)]
pub(crate) fn operation_for_extern(name: &str) -> Option<EffectOperation> {
    definition_for_extern(name).and_then(|definition| definition.operation)
}

pub(crate) fn capabilities_for_extern(name: &str) -> Option<CapabilitySet> {
    definition_for_extern(name).map(|definition| definition.capabilities)
}
