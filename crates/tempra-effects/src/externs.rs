use crate::{Capability, CapabilitySet, EffectOperation};

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
// Declaration-only for now: no caller attaches these facts until K4.3B.
#[allow(dead_code)]
pub(crate) fn operation_for_extern(name: &str) -> Option<EffectOperation> {
    match name {
        "std.fs.read_text" => Some(EffectOperation::FsRead),
        "std.fs.write_text" => Some(EffectOperation::FsWrite),
        "std.io.print" | "std.io.println" => Some(EffectOperation::IoWrite),
        _ => None,
    }
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
