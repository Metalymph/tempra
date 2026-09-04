use tempra_ast::{DenyCapability, Item, Visibility};
use tempra_parser::parse_source;

#[test]
fn pure_is_a_marker_with_its_own_span_and_the_existing_function_placement() {
    let source =
        "// è\n@pure\npub fn f() {}\nfn pure() {}\nextend Int { @pure pub fn method(self) {} }";
    let file = parse_source(source).unwrap();
    let Item::Function(function) = &file.items[0] else {
        panic!()
    };
    let span = function.pure_span.unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], "@pure");
    assert_eq!(
        &source[function.span.start as usize..function.span.end as usize],
        "fn f() {}"
    );
    assert_eq!(function.visibility, Visibility::Public);
    assert!(function.deny_contract.is_none());
    let Item::Function(plain) = &file.items[1] else {
        panic!()
    };
    assert!(plain.pure_span.is_none());
    assert!(plain.deny_contract.is_none());
    let Item::Extend(extension) = &file.items[2] else {
        panic!()
    };
    assert!(extension.methods[0].pure_span.is_some());
    assert!(extension.methods[0].deny_contract.is_none());
    // No keyword: ordinary function, parameter and local identifier positions.
    parse_source("fn pure(pure: Int) -> Int { return pure } fn f() { let pure = 1 }").unwrap();
}

#[test]
fn mixed_attributes_preserve_only_explicit_deny_intent_in_deny_contract() {
    for prefix in [
        "@pure @deny(alloc, alloc) @deny(io,)",
        "@deny(alloc, alloc) @pure @deny(io,)",
        "@deny(alloc, alloc) @deny(io,) @pure",
    ] {
        let source = format!("{prefix} fn f() {{}}");
        let file = parse_source(&source).unwrap();
        let Item::Function(function) = &file.items[0] else {
            panic!()
        };
        let contract = function.deny_contract.as_ref().unwrap();
        assert_eq!(
            contract.denied,
            vec![DenyCapability::Alloc, DenyCapability::Io]
        );
        assert_eq!(contract.attribute_spans.len(), 2);
        for span in &contract.attribute_spans {
            assert!(source[span.start as usize..span.end as usize].starts_with("@deny("));
        }
        let span = function.pure_span.unwrap();
        assert_eq!(&source[span.start as usize..span.end as usize], "@pure");
    }
}

#[test]
fn pure_arguments_duplicates_and_malformed_syntax_are_rejected() {
    for prefix in ["@pure()", "@pure(io)", "@pure(anything)", "@pure\n()"] {
        let error = parse_source(&format!("{prefix} fn f() {{}}"))
            .err()
            .unwrap();
        assert!(error.message.contains("@pure does not accept"));
    }
    for prefix in ["@pure @pure", "@pure @deny(io) @pure"] {
        let source = format!("{prefix} fn f() {{}}");
        let error = parse_source(&source).err().unwrap();
        assert_eq!(error.message, "duplicate @pure attribute");
        assert_eq!(error.span.start as usize, source.rfind("@pure").unwrap());
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            "@pure"
        );
    }
    for source in [
        "@pure",
        "@pure(",
        "@pure io fn f() {}",
        "@Pure fn f() {}",
        "@pure @deny() fn f() {}",
        "@pure @unknown fn f() {}",
    ] {
        assert!(parse_source(source).is_err(), "accepted {source}");
    }
}

#[test]
fn pure_does_not_broaden_supported_attribute_placement() {
    for source in [
        "@pure pub extern fn f()",
        "@pure extern fn f()",
        "@pure struct S {}",
        "@pure enum E { A }",
        "@pure interface I {}",
        "@pure import std.io;",
        "@pure let x = 1",
        "@pure var x = 1",
        "@pure {}",
        "@pure fn() {}",
        "@pure fn [x]() {}",
        "pub @pure fn f() {}",
        "@pure extend Int {}",
        "struct S { @pure x: Int }",
        "interface I { @pure fn f() }",
        "fn f() { @pure fn g() {} }",
        "extend Int { @pure extern fn f() }",
    ] {
        assert!(parse_source(source).is_err(), "accepted {source}");
    }
}
