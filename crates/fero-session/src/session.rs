use fero_diagnostics::{Diagnostic, DiagnosticSink};

#[derive(Debug, Default)]
pub struct CompilationSession {
    diagnostics: DiagnosticSink,
}

impl CompilationSession {
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.diagnostics.diagnostics()
    }
}
