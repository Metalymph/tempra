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

fn binding(
    operation: EffectOperation,
    resources: impl IntoIterator<Item = ResourceBinding>,
) -> Option<ExternOperationBinding> {
    ExternOperationBinding::new(operation, resources).ok()
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
/// Invalid static metadata yields `None` and therefore fails closed as an
/// unclassified operation; the table is independently checked by tests.
pub(crate) fn operation_binding_for_extern(name: &str) -> Option<ExternOperationBinding> {
    match name {
        "std.fs.read_text" => binding(
            EffectOperation::FsRead,
            [ResourceBinding::argument(EffectParameter::Source, 0)],
        ),
        "std.fs.write_text" => binding(
            EffectOperation::FsWrite,
            [ResourceBinding::argument(EffectParameter::Destination, 0)],
        ),
        "std.io.print" | "std.io.println" => binding(
            EffectOperation::IoWrite,
            [ResourceBinding::unknown(EffectParameter::Stream)],
        ),
        _ => None,
    }
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
    operation_binding_for_extern(name).map(|binding| binding.operation)
}

pub(crate) fn capabilities_for_extern(name: &str) -> Option<CapabilitySet> {
    let alloc = CapabilitySet::singleton(Capability::Alloc);
    let io = CapabilitySet::singleton(Capability::Io);
    let io_alloc = io.union(alloc);

    match name {
        "std.fs.exists" => Some(io),
        "std.fs.read_text" | "std.fs.write_text" => Some(io_alloc),
        "std.io.print" | "std.io.println" => Some(io),
        "std.path.join" | "std.path.basename" => Some(alloc),
        "std.strings.trim" | "std.strings.replace" => Some(alloc),
        "std.strings.contains" | "std.strings.starts_with" | "std.strings.ends_with" => {
            Some(CapabilitySet::empty())
        }
        _ => None,
    }
}
