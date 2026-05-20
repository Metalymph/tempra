use tempra_span::Span;

/// A type error emitted during the checking pass.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
    pub module_name: Option<String>,
}

impl TypeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            module_name: None,
        }
    }
}

