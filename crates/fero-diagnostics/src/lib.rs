//! Compiler diagnostics — errors, warnings, and their presentation.
//!
//! Every compiler stage that can fail produces [`Diagnostic`] values. The
//! [`Reporter`] is responsible for formatting and emitting them to stderr.
//! Future iterations will support structured output (JSON) for IDE
//! integration via `fero-lsp`.

use fero_span::Span;
use thiserror::Error;

/// The severity of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// A single compiler diagnostic tied to a source location.
#[derive(Debug, Error)]
#[error("{severity:?} at {span:?}: {message}")]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    /// Convenience constructor for an error diagnostic.
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            span,
            message: message.into(),
        }
    }

    /// Convenience constructor for a warning diagnostic.
    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            span,
            message: message.into(),
        }
    }
}

/// Collects and emits diagnostics.
pub struct Reporter {
    has_errors: bool,
}

impl Reporter {
    pub fn new() -> Self {
        Self { has_errors: false }
    }

    /// Emit a diagnostic to stderr and track whether any errors were seen.
    pub fn report(&mut self, diag: &Diagnostic) {
        if diag.severity == Severity::Error {
            self.has_errors = true;
        }
        eprintln!("{diag}");
    }

    /// Returns `true` if at least one error-level diagnostic has been reported.
    pub fn has_errors(&self) -> bool {
        self.has_errors
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new()
    }
}
