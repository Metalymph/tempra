//! Compiler diagnostics — errors, warnings, and their presentation.
//!
//! Every compiler stage that can fail produces [`Diagnostic`] values. The
//! [`Reporter`] is responsible for formatting and emitting them to stderr.
//! Future iterations will support structured output (JSON) for IDE
//! integration via `tempra-lsp`.

use serde::Serialize;
use tempra_span::Span;
use thiserror::Error;

/// The severity of a diagnostic message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A label attaching a message to a specific source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: Span,
    pub message: Option<String>,
}

/// A structured compiler diagnostic message.
#[derive(Debug, Clone, Error)]
#[error("{severity:?} at {span:?}: {message}")]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: Option<String>,
    pub message: String,
    pub span: Span,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub file_path: Option<String>,
}

impl Diagnostic {
    /// Convenience constructor for an error diagnostic.
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code: None,
            message: message.into(),
            span,
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
            file_path: None,
        }
    }

    /// Convenience constructor for a warning diagnostic.
    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: None,
            message: message.into(),
            span,
            labels: vec![Label {
                span,
                message: None,
            }],
            notes: Vec::new(),
            file_path: None,
        }
    }

    pub fn with_file_path(mut self, path: impl Into<String>) -> Self {
        self.file_path = Some(path.into());
        self
    }

    /// Convert the diagnostic to a public JSON representation without exposing Span internals.
    pub fn to_json_value(&self, file_path: Option<&str>) -> serde_json::Value {
        let path_to_use = file_path.or(self.file_path.as_deref());
        let labels_json: Vec<serde_json::Value> = self
            .labels
            .iter()
            .map(|label| {
                serde_json::json!({
                    "span": {
                        "file": path_to_use,
                        "start": label.span.start,
                        "end": label.span.end,
                    },
                    "message": label.message,
                })
            })
            .collect();

        serde_json::json!({
            "severity": match self.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
                Severity::Hint => "hint",
            },
            "code": self.code,
            "message": self.message,
            "labels": labels_json,
            "notes": self.notes,
        })
    }
}

/// The kind of a code symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SymbolKind {
    Function,
    Struct,
    Enum,
    Method,
    Extension,
    Variable,
    Field,
}

/// A symbol definition (function, struct, enum, etc.) extracted from source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    pub signature: Option<String>,
}

impl Symbol {
    /// Convert the symbol to its public JSON representation.
    pub fn to_json_value(&self, file_path: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "kind": self.kind,
            "span": {
                "file": file_path,
                "start": self.span.start,
                "end": self.span.end,
            },
            "signature": self.signature,
        })
    }
}

/// Collects and emits diagnostics.
pub struct Reporter {
    has_errors: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Reporter {
    pub fn new() -> Self {
        Self {
            has_errors: false,
            diagnostics: Vec::new(),
        }
    }

    /// Emit a diagnostic to stderr and track whether any errors were seen.
    pub fn report(&mut self, diag: Diagnostic) {
        if diag.severity == Severity::Error {
            self.has_errors = true;
        }
        eprintln!("{diag}");
        self.diagnostics.push(diag);
    }

    /// Returns `true` if at least one error-level diagnostic has been reported.
    pub fn has_errors(&self) -> bool {
        self.has_errors
    }

    /// Get all reported diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new()
    }
}
