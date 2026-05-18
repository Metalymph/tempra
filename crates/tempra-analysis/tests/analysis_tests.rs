use tempra_analysis::{AnalysisSession, DocumentUri, LineIndex, LspPosition};

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
    assert_eq!(symbols[0].signature.as_deref().unwrap(), "fn main()");

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

#[test]
fn test_unicode_coordinate_conversions() {
    // Mixed source:
    // Line 0: "let prefix = \"🐶è中\""
    // Line 1: "fn target() -> Int64 { 1 }"
    let source = "let prefix = \"🐶è\u{4e2d}\"\nfn target() -> Int64 { 1 }";
    let index = LineIndex::new(source);

    // 1. ASCII char test on Line 1 ("fn target")
    // "f" in "fn" on Line 1 is the 0th character.
    let lsp_fn = LspPosition {
        line: 1,
        character: 0,
    };
    let text_fn = index.lsp_to_text_position(source, lsp_fn).unwrap();
    assert_eq!(text_fn.line, 1);
    assert_eq!(text_fn.byte_character, 0);

    let roundtrip_lsp = index.text_to_lsp_position(source, text_fn).unwrap();
    assert_eq!(roundtrip_lsp.character, 0);

    // 2. Accented & Multi-byte UTF-8 test on Line 0
    // "let prefix = \"🐶è中\""
    // Let's check UTF-16 and UTF-8 representations:
    // "let prefix = \"" : 14 chars (all ASCII, 14 bytes, 14 UTF-16 code units)
    // "🐶" (dog emoji): 1 character. UTF-8: 4 bytes. UTF-16: 2 code units (surrogate pair).
    // "è": 1 character. UTF-8: 2 bytes. UTF-16: 1 code unit.
    // "中" (CJK): 1 character. UTF-8: 3 bytes. UTF-16: 1 code unit.

    // Position of "🐶": UTF-16 index is 14.
    let lsp_dog = LspPosition {
        line: 0,
        character: 14,
    };
    let text_dog = index.lsp_to_text_position(source, lsp_dog).unwrap();
    assert_eq!(text_dog.byte_character, 14); // UTF-8 byte index is 14.

    // Position of "è": UTF-16 index is 14 + 2 = 16.
    let lsp_e = LspPosition {
        line: 0,
        character: 16,
    };
    let text_e = index.lsp_to_text_position(source, lsp_e).unwrap();
    assert_eq!(text_e.byte_character, 18); // UTF-8 byte index is 18 (14 + 4 bytes for 🐶).

    // Roundtrip verification
    assert_eq!(
        index
            .text_to_lsp_position(source, text_e)
            .unwrap()
            .character,
        16
    );

    // Position of "中": UTF-16 index is 16 + 1 = 17.
    let lsp_cjk = LspPosition {
        line: 0,
        character: 17,
    };
    let text_cjk = index.lsp_to_text_position(source, lsp_cjk).unwrap();
    assert_eq!(text_cjk.byte_character, 20); // UTF-8: 18 + 2 bytes for è.

    assert_eq!(
        index
            .text_to_lsp_position(source, text_cjk)
            .unwrap()
            .character,
        17
    );

    // Position of trailing quote: UTF-16 index is 17 + 1 = 18.
    let lsp_quote = LspPosition {
        line: 0,
        character: 18,
    };
    let text_quote = index.lsp_to_text_position(source, lsp_quote).unwrap();
    assert_eq!(text_quote.byte_character, 23); // UTF-8: 20 + 3 bytes for 中.

    assert_eq!(
        index
            .text_to_lsp_position(source, text_quote)
            .unwrap()
            .character,
        18
    );

    // 3. Absolute byte offset roundtrip verification
    let offset_cjk = index.lsp_position_to_byte_offset(source, lsp_cjk).unwrap();
    assert_eq!(offset_cjk, 20);

    let roundtrip_lsp_cjk = index
        .byte_offset_to_lsp_position(source, offset_cjk)
        .unwrap();
    assert_eq!(roundtrip_lsp_cjk.line, 0);
    assert_eq!(roundtrip_lsp_cjk.character, 17);

    // 4. Invalid mid-surrogate check
    // "🐶" starts at character 14 and takes 2 code units (14 and 15).
    // Accessing character 15 directly should return None since it is mid-surrogate.
    let lsp_invalid = LspPosition {
        line: 0,
        character: 15,
    };
    assert!(index.lsp_to_text_position(source, lsp_invalid).is_none());
}

#[test]
fn test_lsp_queries_after_unicode() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///unicode_hover.tmpr");

    let source = "let prefix = \"🐶è中\"\nfn target() -> Int64 {\n    return 1\n}\nfn main() {\n    target();\n}";
    session.open_document(uri.clone(), source.to_string(), 1);

    // Hover over "target" inside main():
    // "    target();" is on Line 5.
    // "    " is 4 spaces (4 chars).
    // "target" starts at character 4 on Line 5.
    let pos = LspPosition {
        line: 5,
        character: 4,
    };

    let hover_res = session.hover_at_lsp_position(&uri, pos).unwrap();
    assert_eq!(hover_res, "fn target() -> Int64");

    let def_res = session.definition_at_lsp_position(&uri, pos).unwrap();
    assert!(def_res.start > 0);
}
