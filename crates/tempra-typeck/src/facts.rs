use crate::types::Type;
use std::collections::HashMap;
use tempra_span::Span;

/// Local borrowed origin relation emitted by the source borrow-flow pass.
///
/// `borrower` and `owner` identify declaration sites, not textual names.
/// A relation records only facts that the pass can prove locally; it does
/// not represent a cross-module BorrowSummary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowFact {
    pub borrower: Span,
    pub owner: Span,
    pub origin: Span,
}

/// Authoritative facts collected during structural typechecking.
#[derive(Debug, Clone, Default)]
pub struct TypecheckFacts {
    /// Deterministic, ordered local source borrow origins.
    pub borrow_facts: Vec<BorrowFact>,
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
