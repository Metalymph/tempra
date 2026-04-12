/// A type annotation as written in source code.
///
/// These map 1:1 with the surface syntax; the type checker resolves them
/// into the compiler's internal type representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ty {
    /// `int`
    Int,
    /// `float`
    Float,
    /// `string`
    String,
    /// `bool`
    Bool,
}
