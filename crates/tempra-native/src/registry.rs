use crate::std::{fs, io, path, strings};
use tempra_vm::value::VmValue;

pub type NativeFn = fn(&[VmValue]) -> Result<VmValue, String>;

pub fn resolve(module: &str, function: &str) -> Option<NativeFn> {
    match (module, function) {
        ("std.fs", "exists") => Some(fs::exists),
        ("std.fs", "read_text") => Some(fs::read_text),
        ("std.fs", "write_text") => Some(fs::write_text),
        ("std.path", "join") => Some(path::join),
        ("std.path", "basename") => Some(path::basename),
        ("std.strings", "contains") => Some(strings::contains),
        ("std.strings", "starts_with") => Some(strings::starts_with),
        ("std.strings", "ends_with") => Some(strings::ends_with),
        ("std.io", "print") => Some(io::print),
        ("std.io", "println") => Some(io::println),
        _ => None,
    }
}
