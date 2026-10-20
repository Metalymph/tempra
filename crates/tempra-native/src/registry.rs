use crate::std::{fs, io, path, strings};
use tempra_vm::value::VmValue;

pub type NativeFn = fn(&[VmValue]) -> Result<VmValue, String>;

/// One compiler-owned native registration.
struct NativeRegistration {
    module: &'static str,
    function: &'static str,
    implementation: NativeFn,
}

/// The closed production native registry.
///
/// Names, implementations and order are compiler-owned. Consistency tests derive
/// the registered names from this table rather than maintaining a second copy.
const NATIVE_REGISTRATIONS: &[NativeRegistration] = &[
    NativeRegistration {
        module: "std.fs",
        function: "exists",
        implementation: fs::exists,
    },
    NativeRegistration {
        module: "std.fs",
        function: "read_text",
        implementation: fs::read_text,
    },
    NativeRegistration {
        module: "std.fs",
        function: "write_text",
        implementation: fs::write_text,
    },
    NativeRegistration {
        module: "std.path",
        function: "join",
        implementation: path::join,
    },
    NativeRegistration {
        module: "std.path",
        function: "basename",
        implementation: path::basename,
    },
    NativeRegistration {
        module: "std.strings",
        function: "contains",
        implementation: strings::contains,
    },
    NativeRegistration {
        module: "std.strings",
        function: "starts_with",
        implementation: strings::starts_with,
    },
    NativeRegistration {
        module: "std.strings",
        function: "ends_with",
        implementation: strings::ends_with,
    },
    NativeRegistration {
        module: "std.strings",
        function: "trim",
        implementation: strings::trim,
    },
    NativeRegistration {
        module: "std.strings",
        function: "replace",
        implementation: strings::replace,
    },
    NativeRegistration {
        module: "std.io",
        function: "print",
        implementation: io::print,
    },
    NativeRegistration {
        module: "std.io",
        function: "println",
        implementation: io::println,
    },
];

/// Resolves a native implementation from the closed production registry.
///
/// Resolution identity is trusted. Returning a function pointer does not sandbox
/// the implementation or prove any filesystem, network or process claim.
pub fn resolve(module: &str, function: &str) -> Option<NativeFn> {
    NATIVE_REGISTRATIONS
        .iter()
        .find(|registration| registration.module == module && registration.function == function)
        .map(|registration| registration.implementation)
}

/// Iterates over every registered native as `(module, function)`.
///
/// Tests and tooling use this production enumeration instead of duplicating the
/// registry. Ordering is the stable declaration order of the registry table.
pub fn registered_natives() -> impl Iterator<Item = (&'static str, &'static str)> {
    NATIVE_REGISTRATIONS
        .iter()
        .map(|registration| (registration.module, registration.function))
}
