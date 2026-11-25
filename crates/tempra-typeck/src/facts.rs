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
    /// Access mode of the resolved method receiver, keyed by call span.
    pub receiver_accesses: HashMap<Span, bool>,
    /// Actual direct function identity and parameter index for each source argument.
    pub direct_call_parameters: HashMap<Span, (String, Vec<Option<usize>>)>,
    /// Concrete Slice parameter with a proven, non-escaping builtin observation.
    pub(crate) slice_observer_parameters: HashMap<Span, usize>,
    /// Mapping from reference span to target module and definition span.
    pub cross_file_resolutions: HashMap<Span, (String, Span)>,
}
