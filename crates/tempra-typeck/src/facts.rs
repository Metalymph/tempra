use crate::types::Type;
use std::collections::HashMap;
use tempra_span::Span;

/// Authoritative facts collected during structural typechecking.
#[derive(Debug, Clone, Default)]
pub struct TypecheckFacts {
    /// Mapping from expression span to its type.
    pub expr_types: HashMap<Span, Type>,
    /// Mapping from let/var/parameter binding identifier span to its resolved type.
    pub binding_types: HashMap<Span, Type>,
    /// Mapping from variable reference span to its definition span.
    pub resolved_symbols: HashMap<Span, Span>,
    /// Mapping from struct member/field identifier span to its definition span.
    pub member_resolutions: HashMap<Span, Span>,
    /// Mapping from enum variant constructor span to its definition span.
    pub variant_resolutions: HashMap<Span, Span>,
    /// Mapping from method call name span to its implementation/definition span.
    pub method_resolutions: HashMap<Span, Span>,
    /// Mapping from reference span to target module and definition span.
    pub cross_file_resolutions: HashMap<Span, (String, Span)>,
}

