/// An IR-level type descriptor.
///
/// These are resolved, monomorphic types used by backends to determine
/// memory layout and calling conventions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrTy {
    Int,
    Float,
    Bool,
    Unit,
}
