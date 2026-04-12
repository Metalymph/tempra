use fero_diagnostics::Diagnostic;
use fero_hir::HirProgram;

#[derive(Debug, Default)]
pub struct TypeCheckResult {
    pub diagnostics: Vec<Diagnostic>,
}

pub struct TypeChecker;

impl TypeChecker {
    pub fn check(_program: &HirProgram) -> TypeCheckResult {
        TypeCheckResult::default()
    }
}
