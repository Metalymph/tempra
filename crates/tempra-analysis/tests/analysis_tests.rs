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
    let (_def_uri, def_span) = session.goto_definition(&uri, 8, 22).unwrap();
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

    let (_def_uri, def_span) = session.definition_at_lsp_position(&uri, pos).unwrap();
    assert!(def_span.start > 0);
}

#[test]
fn test_semantic_resolution_shadowing() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///shadow.tmpr");

    let source = r#"
        fn main() {
            let x = 10;
            {
                let x = 20;
                let y = x;
            }
            let z = x;
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // Let's test y = x reference resolution
    // "let y = x;" is on line 5. y starts at character 20, x starts at character 24.
    // The inner "x = 20" is on line 4, starting at character 20.
    // The outer "x = 10" is on line 2, starting at character 16.

    // Hover on 'x' in "let y = x" (line 5, character 24)
    let hover_inner_x = session.hover(&uri, 5, 24).unwrap();
    assert_eq!(hover_inner_x, "let x: Int"); // or let x, depends on parser

    // Definition of 'x' in "let y = x" should jump to inner x definition on line 4
    let (_def_uri, def_inner_x) = session.goto_definition(&uri, 5, 24).unwrap();

    // Hover on 'x' in "let z = x" (line 7, character 20)
    let hover_outer_x = session.hover(&uri, 7, 20).unwrap();
    assert_eq!(hover_outer_x, "let x: Int");

    // Definition of 'x' in "let z = x" should jump to outer x definition on line 2
    let (_def_uri, def_outer_x) = session.goto_definition(&uri, 7, 20).unwrap();
    assert_ne!(def_inner_x, def_outer_x);
}

#[test]
fn test_semantic_resolution_parameters() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///params.tmpr");

    let source = r#"
        fn greet(name: String) -> String {
            let msg = name;
            return msg
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // "let msg = name;" is on line 2.
    // Hover on "name" reference (line 2, character 22)
    let hover_param = session.hover(&uri, 2, 22).unwrap();
    assert!(hover_param.contains("name: String"));

    // Goto definition of "name" should jump to the parameter declaration on line 1, character 17
    let (_def_uri, def_param) = session.goto_definition(&uri, 2, 22).unwrap();
    assert!(def_param.start > 0);
}

#[test]
fn test_semantic_resolution_for_loops() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///loops.tmpr");

    let source = r#"
        fn process() {
            let items = [1, 2, 3];
            for item in items {
                let val = item;
            }
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // "let val = item;" is on line 4.
    // Hover on "item" reference (line 4, character 26)
    let hover_loop_var = session.hover(&uri, 4, 26).unwrap();
    assert_eq!(hover_loop_var, "let item");

    // Goto definition of "item" should jump to the loop binding on line 3, character 16
    let (_def_uri, def_loop_var) = session.goto_definition(&uri, 4, 26).unwrap();
    assert!(def_loop_var.start > 0);
}

#[test]
fn test_semantic_resolution_struct_fields() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///fields.tmpr");

    let source = r#"
        struct Point {
            x: Int64,
            y: Int64,
        }
        fn test() {
            let p = Point { x: 1, y: 2 };
            let val = p.x;
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // "let val = p.x;" is on line 7.
    // Hover on "x" struct field access (line 7, character 24)
    let hover_field = session.hover(&uri, 7, 24).unwrap();
    assert!(hover_field.contains("Point.x"));

    // Goto definition of "x" field should jump to struct field declaration on line 2
    let (_def_uri, def_field) = session.goto_definition(&uri, 7, 24).unwrap();
    assert!(def_field.start > 0);
}

#[test]
fn test_completion_keywords_and_types() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///completion_test1.tmpr");

    let source = "fn main() {  }".to_string();
    session.open_document(uri.clone(), source, 1);

    // Complete inside main function
    let completions = session.completions_at_position(&uri, 0, 13);

    // Check some keywords
    let fn_item = completions.iter().find(|c| c.label == "fn").unwrap();
    assert_eq!(fn_item.detail.as_deref().unwrap(), "keyword");
    assert_eq!(fn_item.sort_text.as_deref().unwrap(), "01_keyword_fn");
    assert!(completions.iter().any(|c| c.label == "let"));
    assert!(completions.iter().any(|c| c.label == "if"));

    // Check some types
    let int64_item = completions.iter().find(|c| c.label == "Int64").unwrap();
    assert_eq!(int64_item.detail.as_deref().unwrap(), "type");
    assert_eq!(int64_item.sort_text.as_deref().unwrap(), "06_type_Int64");
    assert!(completions.iter().any(|c| c.label == "String"));
    assert!(completions.iter().any(|c| c.label == "Result"));

    // Check namespaces
    let fs_item = completions.iter().find(|c| c.label == "fs").unwrap();
    assert_eq!(fs_item.detail.as_deref().unwrap(), "module");
    assert_eq!(fs_item.sort_text.as_deref().unwrap(), "07_module_fs");

    // Check sorted order
    for i in 0..completions.len() - 1 {
        let a = completions[i].sort_text.as_deref().unwrap_or("");
        let b = completions[i + 1].sort_text.as_deref().unwrap_or("");
        if a == b {
            assert!(
                completions[i].label <= completions[i + 1].label,
                "Expected label '{}' <= '{}'",
                completions[i].label,
                completions[i + 1].label
            );
        } else {
            assert!(a < b, "Expected sort_text '{}' < '{}'", a, b);
        }
    }
}

#[test]
fn test_completion_local_bindings() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///completion_test2.tmpr");

    let source = r#"
        fn foo() {}
        fn main() {
            let my_var = 123
            
        }
    "#
    .to_string();
    session.open_document(uri.clone(), source, 1);

    // Complete on line 4 (after my_var)
    let completions = session.completions_at_position(&uri, 4, 12);

    // Should see top level function
    assert!(completions.iter().any(|c| c.label == "foo"));

    // Should see local variable
    assert!(completions.iter().any(|c| c.label == "my_var"));
}

#[test]
fn test_completion_member_fallback_empty() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///completion_test3.tmpr");

    let source = r#"
        fn main() {
            let obj = 123
            obj.
        }
    "#
    .to_string();
    session.open_document(uri.clone(), source, 1);

    // Complete right after dot (line 3, char 16)
    let completions = session.completions_at_position(&uri, 3, 16);

    // Should be empty since Int64 has no fields or methods defined here
    assert!(completions.is_empty());
}

#[test]
fn test_completion_member() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///completion_test4.tmpr");

    let source = r#"
        struct Point {
            x: Int64
            y: Int64
        }
        extend Point {
            fn display(self) -> String { return "" }
        }
        fn main() {
            let p = Point { x: 0, y: 0 }
            p.
        }
    "#
    .to_string();
    session.open_document(uri.clone(), source, 1);

    // Complete right after dot: line 10, char 14
    let completions = session.completions_at_position(&uri, 10, 14);

    assert!(!completions.is_empty(), "completions should not be empty");

    // Assert sorting: Fields (03) come before Methods (04)
    assert_eq!(completions[0].label, "x");
    assert_eq!(completions[0].kind, tempra_analysis::CompletionKind::Field);
    assert_eq!(completions[0].sort_text.as_deref().unwrap(), "03_field_x");
    assert_eq!(completions[0].detail.as_deref().unwrap(), "field: Int64");

    assert_eq!(completions[1].label, "y");
    assert_eq!(completions[1].kind, tempra_analysis::CompletionKind::Field);
    assert_eq!(completions[1].sort_text.as_deref().unwrap(), "03_field_y");
    assert_eq!(completions[1].detail.as_deref().unwrap(), "field: Int64");

    assert_eq!(completions[2].label, "display");
    assert_eq!(completions[2].kind, tempra_analysis::CompletionKind::Method);
    assert_eq!(
        completions[2].sort_text.as_deref().unwrap(),
        "04_method_display"
    );

    // Should NOT contain random keywords or locals
    assert!(!completions.iter().any(|c| c.label == "p"));
    assert!(!completions.iter().any(|c| c.label == "fn"));
}

#[test]
fn test_semantic_tokens() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///semantic_tokens.tmpr");

    let source = r#"
        struct Point {
            x: Int64
        }
        fn main() {
            let p = Point { x: 10 };
        }
    "#
    .to_string();

    session.open_document(uri.clone(), source, 1);
    let tokens = session.semantic_tokens(&uri);

    assert!(!tokens.is_empty(), "tokens should not be empty");

    // We should find keywords like "struct", "let", "fn"
    assert!(tokens
        .iter()
        .any(|t| t.kind == tempra_analysis::SemanticTokenKind::Keyword));

    // We should find the struct definition "Point"
    assert!(tokens
        .iter()
        .any(|t| t.kind == tempra_analysis::SemanticTokenKind::Struct));

    // We should find the field definition "x"
    assert!(tokens
        .iter()
        .any(|t| t.kind == tempra_analysis::SemanticTokenKind::Property));
}

#[test]
fn test_incremental_character_insert() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    let source = "fn foo() {}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Insert a parameter 'x: Int' inside 'foo()'
    // 'foo()' starts at line 0, char 3. '()' starts at char 6.
    // Insert at line 0, character 7 (between '(' and ')')
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 0,
            character: 7,
        },
        end: tempra_analysis::LspPosition {
            line: 0,
            character: 7,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range), "x: Int".to_string())], 2)
        .unwrap();

    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.content, "fn foo(x: Int) {}\n");
    assert_eq!(snap.version, 2);
}

#[test]
fn test_incremental_character_delete() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    let source = "fn foo(x: Int) {}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Delete 'x: Int'
    // 'x: Int' starts at line 0, character 7 and ends at character 13
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 0,
            character: 7,
        },
        end: tempra_analysis::LspPosition {
            line: 0,
            character: 13,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range), "".to_string())], 2)
        .unwrap();

    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.content, "fn foo() {}\n");
}

#[test]
fn test_incremental_multiline_edits() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    let source = "fn foo() {\n    let x = 1;\n    return x;\n}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Replace the body: from line 1 character 0 to line 3 character 0 (exclusive/inclusive)
    // Line 1: '    let x = 1;\n'
    // Line 2: '    return x;\n'
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 1,
            character: 0,
        },
        end: tempra_analysis::LspPosition {
            line: 3,
            character: 0,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range), "    return 42;\n".to_string())], 2)
        .unwrap();

    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.content, "fn foo() {\n    return 42;\n}\n");
}

#[test]
fn test_incremental_unicode_edits() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    // "🐶" is 4 bytes (F0 9F 90 B6) but 2 UTF-16 code units.
    // "è" is 2 bytes (C3 A8) but 1 UTF-16 code unit.
    // "中" is 3 bytes (E4 B8 AD) but 1 UTF-16 code unit.
    let source = "let x = \"🐶è中\"\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Let's replace "🐶" (start at line 0, character 9, end at line 0, character 11 in UTF-16) with "🐱" (2 UTF-16 code units)
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 0,
            character: 9,
        },
        end: tempra_analysis::LspPosition {
            line: 0,
            character: 11,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range), "🐱".to_string())], 2)
        .unwrap();

    let snap = session.snapshot(&uri).unwrap();
    assert_eq!(snap.content, "let x = \"🐱è中\"\n");
}

#[test]
fn test_incremental_lazy_diagnostics_completion_tokens() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    // Open a document with a syntax error
    let source = "fn foo() {\n    let x =\n}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Diagnostics should find the error (let x = )
    let diags = session.diagnostics(&uri).unwrap();
    assert!(!diags.is_empty(), "should have diagnostics error");

    // Make an edit that fixes the syntax error
    // Line 1: '    let x =\n'
    // Replace '\n' or insert ' 10;\n'
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 1,
            character: 11,
        },
        end: tempra_analysis::LspPosition {
            line: 1,
            character: 11,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range), " 10;".to_string())], 2)
        .unwrap();

    // Verify it is not analyzed yet (still dirty)
    // (We don't expose dirty_documents directly, but we can query diagnostics and verify it has cleared)
    let diags_fixed = session.diagnostics(&uri).unwrap();
    assert!(
        diags_fixed.is_empty(),
        "diagnostics should be empty after fix"
    );

    // Test completions work on lazy update too
    let range2 = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 2,
            character: 1,
        },
        end: tempra_analysis::LspPosition {
            line: 2,
            character: 1,
        },
    };
    session
        .update_document_incremental(&uri, vec![(Some(range2), "retur\n".to_string())], 3)
        .unwrap();

    // Query completions at line 2 character 5 (after "retur")
    let completions = session.completions_at_position(&uri, 2, 5);
    assert!(
        completions.iter().any(|c| c.label == "return"),
        "should suggest return"
    );

    // Verify semantic tokens lazy evaluation
    let tokens = session.semantic_tokens(&uri);
    assert!(!tokens.is_empty(), "should compute semantic tokens lazily");
}

#[test]
fn test_incremental_errors_on_invalid_range() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///incremental.tmpr");
    let source = "fn foo() {}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Out of bounds line
    let range = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 10,
            character: 0,
        },
        end: tempra_analysis::LspPosition {
            line: 10,
            character: 1,
        },
    };
    let res = session.update_document_incremental(&uri, vec![(Some(range), "foo".to_string())], 2);
    assert!(res.is_err(), "should return error on invalid range");

    // Out of bounds character
    let range2 = tempra_analysis::LspRange {
        start: tempra_analysis::LspPosition {
            line: 0,
            character: 50,
        },
        end: tempra_analysis::LspPosition {
            line: 0,
            character: 51,
        },
    };
    let res2 =
        session.update_document_incremental(&uri, vec![(Some(range2), "foo".to_string())], 3);
    assert!(
        res2.is_err(),
        "should return error on invalid character range"
    );
}

#[test]
fn test_references_and_highlights() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///references.tmpr");

    let source = "fn test_func(x: Int64) -> Int64 {\n    let y = x + 1;\n    let x = y + 2;\n    let z = x + 3;\n    for i in [1, 2] {\n        let w = i + x;\n    }\n    return z;\n}\n".to_string();
    session.open_document(uri.clone(), source, 1);

    // Let's first test outer parameter x at line 0, character 13:
    // include_declaration = true
    let refs_outer_inc = session.references_at_position(&uri, 0, 13, true).unwrap();
    // Should have 2 spans: the declaration (line 0, col 13) and the reference (line 1, col 12).
    assert_eq!(refs_outer_inc.len(), 2);

    // include_declaration = false
    let refs_outer_exc = session.references_at_position(&uri, 0, 13, false).unwrap();
    // Should have 1 span: reference only.
    assert_eq!(refs_outer_exc.len(), 1);

    // Let's get the inner shadowed x references (e.g. line 2, character 8):
    let refs_inner = session.references_at_position(&uri, 2, 8, true).unwrap();
    // Inner x has 3 spans: declaration (line 2, col 8), reference on line 3, col 12, and reference on line 5, col 20.
    assert_eq!(refs_inner.len(), 3);

    // Verify disjointness to prove shadowing correctness (outer references must not include inner shadowed references)
    for span in &refs_outer_inc {
        assert!(
            !refs_inner.contains(span),
            "outer reference list must be completely disjoint from shadowed inner reference list"
        );
    }

    // Check highlights for outer x at line 0, character 13:
    let highlights_outer = session.document_highlights(&uri, 0, 13).unwrap();
    // Should contain 1 Write highlight (the declaration) and 1 Read highlight (the reference).
    assert_eq!(highlights_outer.len(), 2);
    assert_eq!(
        highlights_outer[0].kind,
        tempra_analysis::DocumentHighlightKind::Write
    );
    assert_eq!(
        highlights_outer[1].kind,
        tempra_analysis::DocumentHighlightKind::Read
    );

    // Check loop variable i (line 4, character 8):
    let refs_loop = session.references_at_position(&uri, 4, 8, true).unwrap();
    // Should contain declaration (line 4, col 8) and reference (line 5, col 16).
    assert_eq!(refs_loop.len(), 2);

    let highlights_loop = session.document_highlights(&uri, 4, 8).unwrap();
    assert_eq!(highlights_loop.len(), 2);
    assert_eq!(
        highlights_loop[0].kind,
        tempra_analysis::DocumentHighlightKind::Write
    );
    assert_eq!(
        highlights_loop[1].kind,
        tempra_analysis::DocumentHighlightKind::Read
    );

    // Query on an invalid offset or non-existent symbol (e.g. whitespace):
    let refs_none = session.references_at_position(&uri, 0, 0, true);
    assert!(refs_none.is_none());
}

#[test]
fn test_rename_local_variables_and_shadowing() {
    let mut session = AnalysisSession::new();
    let uri = DocumentUri::new("file:///rename_test.tmpr");

    let source = r#"fn main(x: Int64) {
    let y = x + 1;
    let x = 10;
    let z = x + 2;
}"#
    .to_string();

    session.open_document(uri.clone(), source, 1);

    // 1. Rename outer parameter 'x' (line 0, col 8)
    let prep_outer = session.prepare_rename(&uri, 0, 8);
    assert!(prep_outer.is_some(), "outer x should be renameable");
    let prep_outer_range = prep_outer.unwrap();
    assert_eq!(prep_outer_range.start.line, 0);
    assert_eq!(prep_outer_range.start.character, 8);
    assert_eq!(prep_outer_range.end.line, 0);
    assert_eq!(prep_outer_range.end.character, 9);

    let edits_outer = session.rename_edits(&uri, 0, 8, "outer_x").unwrap();
    let text_edits_outer = edits_outer.changes.get(&uri).unwrap();
    // Should rename parameter at (0, 8) and usage at (1, 12).
    // Because they are sorted descending:
    assert_eq!(text_edits_outer.len(), 2);
    // Highest start offset first:
    assert_eq!(text_edits_outer[0].range.start.line, 1);
    assert_eq!(text_edits_outer[0].range.start.character, 12);
    assert_eq!(text_edits_outer[1].range.start.line, 0);
    assert_eq!(text_edits_outer[1].range.start.character, 8);

    // 2. Rename inner shadowed variable 'x' (line 2, col 8)
    let prep_inner = session.prepare_rename(&uri, 2, 8);
    assert!(prep_inner.is_some(), "inner x should be renameable");
    let edits_inner = session.rename_edits(&uri, 2, 8, "inner_x").unwrap();
    let text_edits_inner = edits_inner.changes.get(&uri).unwrap();
    // Should rename declaration at (2, 8) and usage at (3, 12)
    assert_eq!(text_edits_inner.len(), 2);
    assert_eq!(text_edits_inner[0].range.start.line, 3);
    assert_eq!(text_edits_inner[0].range.start.character, 12);
    assert_eq!(text_edits_inner[1].range.start.line, 2);
    assert_eq!(text_edits_inner[1].range.start.character, 8);

    // 3. Reject invalid identifier
    let invalid_rename = session.rename_edits(&uri, 0, 8, "123invalid");
    assert!(
        invalid_rename.is_none(),
        "should reject invalid identifier starting with digit"
    );

    let invalid_keyword = session.rename_edits(&uri, 0, 8, "struct");
    assert!(invalid_keyword.is_none(), "should reject keyword");

    // 4. Reject rename on non-variable symbol (like function 'main')
    let prep_main = session.prepare_rename(&uri, 0, 4);
    assert!(prep_main.is_none(), "should reject renaming function main");
}

#[test]
fn test_rename_validation_and_overlap_rejection() {
    use tempra_analysis::{LspRange, TextEdit, WorkspaceEdit};
    let uri = DocumentUri::new("file:///overlap.tmpr");

    let mut edits = Vec::new();
    edits.push(TextEdit {
        range: LspRange {
            start: LspPosition {
                line: 1,
                character: 5,
            },
            end: LspPosition {
                line: 1,
                character: 10,
            },
        },
        new_text: "foo".to_string(),
    });
    edits.push(TextEdit {
        range: LspRange {
            start: LspPosition {
                line: 1,
                character: 8,
            },
            end: LspPosition {
                line: 1,
                character: 15,
            },
        },
        new_text: "bar".to_string(),
    });

    let mut changes = std::collections::HashMap::new();
    changes.insert(uri, edits);
    let mut ws_edit = WorkspaceEdit { changes };

    // Should fail validation due to overlap (1:8 < 1:10)
    let res = ws_edit.validate_and_clean();
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Overlapping"));
}

#[test]
fn test_cross_file_features() {
    let mut session = AnalysisSession::new();
    let uri_main = DocumentUri::new("file:///main.tmpr");
    let uri_math = DocumentUri::new("file:///math.tmpr");

    let source_math = r#"
        pub struct Point { x: Int64, y: Int64 }
        pub fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn private_helper() {}
    "#
    .to_string();

    let source_main = r#"
        import math
        fn process(p: math.Point) {
            math.add(1, 2);
            math.
        }
    "#
    .to_string();

    session.open_document(uri_math.clone(), source_math, 1);
    session.open_document(uri_main.clone(), source_main, 1);

    let diags_math = session.diagnostics(&uri_math).unwrap();
    println!("Math diags: {:?}", diags_math);
    let diags_main = session.diagnostics(&uri_main).unwrap();
    println!("Main diags: {:?}", diags_main);

    // 1. Cross-file Goto-Definition
    // "math.add" is on line 3, char 18 ('d' in add)
    let def_add = session.goto_definition(&uri_main, 3, 18);
    assert!(def_add.is_some(), "Goto-Definition on math.add failed");
    let (def_uri, _) = def_add.unwrap();
    assert_eq!(def_uri, uri_math);

    // "math.Point" is on line 2, char 27 ('P' in Point)
    let def_point = session.goto_definition(&uri_main, 2, 27);
    assert!(def_point.is_some(), "Goto-Definition on math.Point failed");
    let (def_uri_pt, _) = def_point.unwrap();
    assert_eq!(def_uri_pt, uri_math);

    // 2. Cross-file Hover
    let hover_add = session.hover(&uri_main, 3, 18).unwrap();
    println!("Hover add: {:?}", hover_add);
    assert!(hover_add.contains("pub fn add") || hover_add.contains("fn add"));

    let hover_point = session.hover(&uri_main, 2, 27);
    assert!(hover_point.is_some(), "Hover on math.Point failed");
    assert!(hover_point.unwrap().contains("pub struct Point"));

    // 3. Cross-file Completions
    // "math." is on line 4, char 17
    let completions = session.completions_at_position(&uri_main, 4, 17);
    assert!(completions.iter().any(|c| c.label == "Point"));
    assert!(completions.iter().any(|c| c.label == "add"));
    assert!(!completions.iter().any(|c| c.label == "private_helper"));

    // 4. Module Graph Diagnostics
    let uri_cycle1 = DocumentUri::new("file:///cycle1.tmpr");
    let uri_cycle2 = DocumentUri::new("file:///cycle2.tmpr");
    session.open_document(uri_cycle1.clone(), "import cycle2\n".to_string(), 1);
    session.open_document(uri_cycle2.clone(), "import cycle1\n".to_string(), 1);

    let diags = session.diagnostics(&uri_cycle1).unwrap();
    assert!(!diags.is_empty(), "Expected cyclic import diagnostics");
    assert!(diags[0].message.contains("cyclic import"));
}
