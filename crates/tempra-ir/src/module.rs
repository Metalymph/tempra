use crate::function::Function;

/// A compiled Tempra program ready for execution or code generation.
///
/// Contains all functions defined in (or imported by) the program.
#[derive(Debug)]
pub struct Module {
    pub functions: Vec<Function>,
}

impl Module {
    /// Create an empty module.
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
        }
    }
}

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}
