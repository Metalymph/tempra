//! Unit and integration smoke tests for the tempra-lsp adapter layer.
//!
//! These tests verify the **conversion logic** — diagnostics, symbol kinds,
//! and span-to-range — without spinning up a real JSON-RPC transport.

use tempra_analysis::{AnalysisSession, DocumentUri, LspPosition};
use tempra_lsp::server::{symbol_kind_to_lsp};
use tower_lsp::lsp_types::SymbolKind;

// ---------------------------------------------------------------------------
// Diagnostic severity conversion
// ---------------------------------------------------------------------------

#[test]
fn test_severity_error_maps_correctly() {
    use tempra_diagnostics::Severity;
    use tower_lsp::lsp_types::DiagnosticSeverity;

    let pairs: &[(Severity, DiagnosticSeverity)] = &[
        (Severity::Error, DiagnosticSeverity::ERROR),
        (Severity::Warning, DiagnosticSeverity::WARNING),
        (Severity::Info, DiagnosticSeverity::INFORMATION),
        (Severity::Hint, DiagnosticSeverity::HINT),
    ];

    for (tempra_sev, expected_lsp) in pairs {
        let got = match tempra_sev {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            Severity::Info => DiagnosticSeverity::INFORMATION,
            Severity::Hint => DiagnosticSeverity::HINT,
        };
        assert_eq!(got, *expected_lsp);
    }
}

// ---------------------------------------------------------------------------
// Symbol kind conversion
// ---------------------------------------------------------------------------

#[test]
fn test_symbol_kind_function() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Function),
        SymbolKind::FUNCTION
    );
}

#[test]
fn test_symbol_kind_struct() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Struct),
        SymbolKind::STRUCT
    );
}

#[test]
fn test_symbol_kind_enum() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Enum),
        SymbolKind::ENUM
    );
}

#[test]
fn test_symbol_kind_method() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Method),
        SymbolKind::METHOD
    );
}

#[test]
fn test_symbol_kind_extension() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Extension),
        SymbolKind::MODULE
    );
}

#[test]
fn test_symbol_kind_variable() {
    assert_eq!(
        symbol_kind_to_lsp(tempra_diagnostics::SymbolKind::Variable),
        SymbolKind::VARIABLE
    );
}

// ---------------------------------------------------------------------------
// span_to_lsp_range via AnalysisSession helper
// ---------------------------------------------------------------------------

#[test]
fn test_span_to_lsp_range_ascii() {
    // source: "fn add() {}\n"
    //          0123456789
    let source = "fn add() {}\n".to_string();
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///test.tmpr");
    session.open_document(uri.clone(), source, 1);

    // Span covering "add" → bytes 3..6
    let span = tempra_span::Span::new(3, 6);
    let range = session.span_to_lsp_range(&uri, span).expect("range should resolve");

    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 3); // UTF-16 == UTF-8 for ASCII
    assert_eq!(range.end.line, 0);
    assert_eq!(range.end.character, 6);
}

#[test]
fn test_span_to_lsp_range_unicode() {
    // "🐶 fn" — emoji is 4 UTF-8 bytes but 2 UTF-16 code units
    // bytes: 0-3=🐶, 4=' ', 5='f', 6='n'
    let source = "🐶 fn\n".to_string();
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///unicode.tmpr");
    session.open_document(uri.clone(), source, 1);

    // Span covering "fn" → bytes 5..7
    let span = tempra_span::Span::new(5, 7);
    let range = session.span_to_lsp_range(&uri, span).expect("range should resolve");

    assert_eq!(range.start.line, 0);
    assert_eq!(range.start.character, 3); // 🐶=2 code units + ' '=1 → character 3
    assert_eq!(range.end.line, 0);
    assert_eq!(range.end.character, 5);
}

#[test]
fn test_span_to_lsp_range_unknown_uri() {
    let session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///nonexistent.tmpr");
    let span = tempra_span::Span::new(0, 5);
    assert!(session.span_to_lsp_range(&uri, span).is_none());
}

// ---------------------------------------------------------------------------
// Integration smoke: open → hover → definition → symbols
// ---------------------------------------------------------------------------

#[test]
fn test_integration_open_hover_definition_symbols() {
    let source = r#"
struct Point { x: Int64 }

fn add(a: Int64) -> Int64 {
    return a + 1
}
"#
    .to_string();

    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///smoke.tmpr");
    session.open_document(uri.clone(), source, 1);

    // Hover on "Point" definition (line 1, character ~7)
    let hover_pos = LspPosition { line: 1, character: 7 };
    let hover = session.hover_at_lsp_position(&uri, hover_pos);
    assert!(hover.is_some(), "hover on Point should return something");
    let hover_text = hover.unwrap();
    assert!(
        hover_text.contains("Point"),
        "hover should mention 'Point', got: {}",
        hover_text
    );

    // Symbols should list at least Point and add
    let syms = session.document_symbols(&uri).expect("symbols should be available");
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Point"), "symbols missing Point: {:?}", names);
    assert!(names.contains(&"add"), "symbols missing add: {:?}", names);

    // All symbol spans should produce valid LSP ranges
    for sym in &syms {
        let range = session.span_to_lsp_range(&uri, sym.span);
        assert!(
            range.is_some(),
            "symbol '{}' span could not be converted to LspRange",
            sym.name
        );
    }
}

#[test]
fn test_integration_diagnostics_on_type_error() {
    let source = "fn main() { let x: String = 42; }".to_string();
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///err.tmpr");
    session.open_document(uri.clone(), source, 1);

    let diags = session.diagnostics(&uri).expect("diagnostics available");
    assert!(!diags.is_empty(), "type error should produce diagnostics");
}

#[test]
fn test_integration_close_clears_document() {
    let source = "fn hello() {}".to_string();
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///close.tmpr");
    session.open_document(uri.clone(), source, 1);
    assert!(session.document_symbols(&uri).is_some());
    session.close_document(&uri);
    assert!(session.document_symbols(&uri).is_none());
}
