use crate::{Capability, CapabilitySet};

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
