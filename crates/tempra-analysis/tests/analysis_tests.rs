use tempra_analysis::{AnalysisSession, DocumentUri};

#[test]
fn test_analysis_session_lifecycle() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///test.tmpr");

    assert!(!session.contains_document(&uri));

    let content = "fn main() { let x = 123; }".to_string();
    session.open_document(uri.clone(), content.clone(), 1);

    assert!(session.contains_document(&uri));

    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.content, content);
    assert_eq!(snap.version, 1);

    // Check diagnostics (valid file should be empty)
    let diags = session.diagnostics(&uri).unwrap();
    assert!(
        diags.is_empty(),
        "expected no diagnostics for valid file: {:?}",
        diags
    );

    // Check symbols
    let symbols = session.document_symbols(&uri).unwrap();
    assert_eq!(symbols.len(), 1);
    assert_eq!(symbols[0].name, "main");
    assert_eq!(
        symbols[0].signature.as_deref().unwrap(),
        "fn main()"
    );

    // Close
    session.close_document(&uri);
    assert!(!session.contains_document(&uri));
    assert!(session.snapshot(&uri).is_none());
    assert!(session.diagnostics(&uri).is_none());
}

#[test]
fn test_analysis_session_errors() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///error.tmpr");

    // Syntax error
    session.open_document(uri.clone(), "fn main( {".to_string(), 1);
    let diags = session.diagnostics(&uri).unwrap();
    assert!(!diags.is_empty(), "expected syntax error diagnostics");
    assert!(diags[0].message.to_lowercase().contains("expected"));

    // Update to typecheck error
    session.update_document(&uri, "fn main() { let x: String = 123; }".to_string(), 2);
    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.version, 2);

    let diags = session.diagnostics(&uri).unwrap();
    assert!(!diags.is_empty(), "expected type error diagnostics");

    let symbols = session.document_symbols(&uri).unwrap();
    assert_eq!(symbols.len(), 1);
}

#[test]
fn test_hover_and_goto_definition() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///hover.tmpr");

    let source = r#"
        struct Point { x: Int64 }
        
        fn add(a: Int64) -> Int64 {
            return a + 1
        }
        
        fn main() {
            let p = Point { x: 10 };
            add(5);
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // "Point" is defined on line 1, and referenced on line 8.
    // Let's test hover at line 1, character 18 (inside "Point")
    let hover_point = session.hover(&uri, 1, 18).unwrap();
    assert_eq!(hover_point, "struct Point");

    // Goto definition at line 8, character 22 (referencing "Point")
    let def_span = session.goto_definition(&uri, 8, 22).unwrap();
    assert!(def_span.start > 0);

    // Hover on "add" at line 9, character 14
    let hover_add = session.hover(&uri, 9, 14).unwrap();
    assert_eq!(hover_add, "fn add(a: Int64) -> Int64");
}
