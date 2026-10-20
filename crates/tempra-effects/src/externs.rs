use crate::{Capability, CapabilitySet, EffectOperation, EffectParameter};

/// An explicit specialized-operation binding for a known native extern.
///
/// `argument_index` is the zero-based positional TIR argument that carries the
/// resource. `None` means the resource is not represented by a call argument and
/// must be treated as unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ExternOperationBinding {
    pub(crate) operation: EffectOperation,
    pub(crate) parameter: EffectParameter,
    pub(crate) argument_index: Option<usize>,
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
pub(crate) fn operation_binding_for_extern(name: &str) -> Option<ExternOperationBinding> {
    match name {
        "std.fs.read_text" => Some(ExternOperationBinding {
            operation: EffectOperation::FsRead,
            parameter: EffectParameter::Source,
            argument_index: Some(0),
        }),
        "std.fs.write_text" => Some(ExternOperationBinding {
            operation: EffectOperation::FsWrite,
            parameter: EffectParameter::Destination,
            argument_index: Some(0),
        }),
        "std.io.print" | "std.io.println" => Some(ExternOperationBinding {
            operation: EffectOperation::IoWrite,
            parameter: EffectParameter::Stream,
            argument_index: None,
        }),
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
