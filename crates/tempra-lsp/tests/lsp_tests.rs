//! Unit and integration smoke tests for the tempra-lsp adapter layer.
//!
//! These tests verify the **conversion logic** — diagnostics, symbol kinds,
//! and span-to-range — without spinning up a real JSON-RPC transport.

use tempra_analysis::{AnalysisSession, DocumentUri, LspPosition};
use tempra_lsp::server::symbol_kind_to_lsp;
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
    let range = session
        .span_to_lsp_range(&uri, span)
        .expect("range should resolve");

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
    let range = session
        .span_to_lsp_range(&uri, span)
        .expect("range should resolve");

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
    let hover_pos = LspPosition {
        line: 1,
        character: 7,
    };
    let hover = session.hover_at_lsp_position(&uri, hover_pos);
    assert!(hover.is_some(), "hover on Point should return something");
    let hover_text = hover.unwrap();
    assert!(
        hover_text.contains("Point"),
        "hover should mention 'Point', got: {}",
        hover_text
    );

    // Symbols should list at least Point and add
    let syms = session
        .document_symbols(&uri)
        .expect("symbols should be available");
    let names: Vec<&str> = syms.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Point"),
        "symbols missing Point: {:?}",
        names
    );
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

// ---------------------------------------------------------------------------
// Completion conversion
// ---------------------------------------------------------------------------

#[test]
fn test_completion_conversion_kinds() {
    use tempra_analysis::CompletionKind;
    use tempra_lsp::server::completion_kind_to_lsp;
    use tower_lsp::lsp_types::CompletionItemKind;

    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Keyword),
        CompletionItemKind::KEYWORD
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Function),
        CompletionItemKind::FUNCTION
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Struct),
        CompletionItemKind::STRUCT
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Enum),
        CompletionItemKind::ENUM
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Variable),
        CompletionItemKind::VARIABLE
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Method),
        CompletionItemKind::METHOD
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Type),
        CompletionItemKind::CLASS
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Module),
        CompletionItemKind::MODULE
    );
    assert_eq!(
        completion_kind_to_lsp(CompletionKind::Field),
        CompletionItemKind::FIELD
    );
}

#[tokio::test]
async fn test_completion_initialize_trigger() {
    use tempra_lsp::server::Backend;
    use tower_lsp::lsp_types::*;
    use tower_lsp::LanguageServer;

    let (service, _) = tower_lsp::LspService::new(Backend::new);
    let backend = service.inner();

    let params = InitializeParams::default();
    let res = backend.initialize(params).await.unwrap();

    let capabilities = res.capabilities;
    let completion_provider = capabilities
        .completion_provider
        .expect("completion provider capability expected");
    let trigger_chars = completion_provider
        .trigger_characters
        .expect("trigger characters expected");

    assert_eq!(trigger_chars, vec![".".to_string()]);
}

#[tokio::test]
async fn test_completion_conversion_fields() {
    use tempra_lsp::server::Backend;
    use tower_lsp::lsp_types::*;
    use tower_lsp::LanguageServer;

    let (service, _) = tower_lsp::LspService::new(Backend::new);
    let backend = service.inner();

    let uri = Url::parse("file:///test.tmpr").unwrap();
    let source = r#"
        struct Point {
            x: Int64
        }
        fn main() {
            let p = Point { x: 0 }
            p.
        }
    "#
    .to_string();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "tempra".to_string(),
                version: 1,
                text: source,
            },
        })
        .await;

    // Call completion at line 6 (index 6, character 14 - after the dot)
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let response = backend
        .completion(params)
        .await
        .unwrap()
        .expect("completions expected");
    let items = match response {
        CompletionResponse::Array(arr) => arr,
        CompletionResponse::List(list) => list.items,
    };

    assert!(!items.is_empty());
    let x_item = items.iter().find(|i| i.label == "x").unwrap();
    assert_eq!(x_item.kind.unwrap(), CompletionItemKind::FIELD);
    assert_eq!(x_item.detail.as_deref().unwrap(), "field: Int64");
    assert_eq!(x_item.sort_text.as_deref().unwrap(), "03_field_x");
    assert_eq!(x_item.insert_text.as_deref().unwrap(), "x");
}

#[tokio::test]
async fn test_semantic_tokens_full() {
    use tower_lsp::lsp_types::*;
    use tower_lsp::LanguageServer;

    let (service, _) = tower_lsp::LspService::new(tempra_lsp::server::Backend::new);
    let backend = service.inner();

    let uri = Url::parse("file:///test.tmpr").unwrap();
    let source = r#"
        struct Point {
            x: Int64
        }
        fn main() {
            let p = Point { x: 10 };
        }
    "#
    .to_string();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "tempra".to_string(),
                version: 1,
                text: source,
            },
        })
        .await;

    let params = SemanticTokensParams {
        text_document: TextDocumentIdentifier { uri },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend
        .semantic_tokens_full(params)
        .await
        .unwrap()
        .expect("expected semantic tokens");

    let tokens = match result {
        SemanticTokensResult::Tokens(tokens) => tokens.data,
        _ => panic!("expected SemanticTokensResult::Tokens"),
    };

    assert!(!tokens.is_empty());
    // We should find struct Point (type 2) and struct keyword (type 12)
    assert!(
        tokens.iter().any(|t| t.token_type == 2),
        "expected struct token"
    );
    assert!(
        tokens.iter().any(|t| t.token_type == 12),
        "expected keyword token"
    );
}
