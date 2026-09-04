use tempra_ast::{DenyCapability, Item};
use tempra_parser::parse_source;

#[test]
fn deny_normalizes_names_and_preserves_each_attribute_span() {
    let source = "// è\n@deny(alloc, alloc)\n@deny( panic, io, process, spawn, nondeterminism, )\npub fn f() {}";
    let file = parse_source(source).unwrap();
    let Item::Function(function) = &file.items[0] else {
        panic!()
    };
    let contract = function.deny_contract.as_ref().unwrap();
    assert_eq!(
        contract.denied,
        vec![
            DenyCapability::Alloc,
            DenyCapability::Panic,
            DenyCapability::Io,
            DenyCapability::Process,
            DenyCapability::Spawn,
            DenyCapability::Nondeterminism
        ]
    );
    let attributes: Vec<_> = contract
        .attribute_spans
        .iter()
        .map(|span| &source[span.start as usize..span.end as usize])
        .collect();
    assert_eq!(
        attributes,
        vec![
            "@deny(alloc, alloc)",
            "@deny( panic, io, process, spawn, nondeterminism, )"
        ]
    );
    assert_eq!(
        &source[function.span.start as usize..function.span.end as usize],
        "fn f() {}"
    );
}

#[test]
fn deny_single_and_extend_method_parse_without_affecting_plain_functions() {
    let file = parse_source(
        "@deny(alloc) fn f() {}\nfn plain() {}\nextend Int { @deny(panic) pub fn g(self) {} }",
    )
    .unwrap();
    let Item::Function(f) = &file.items[0] else {
        panic!()
    };
    assert_eq!(
        f.deny_contract.as_ref().unwrap().denied,
        vec![DenyCapability::Alloc]
    );
    let Item::Function(plain) = &file.items[1] else {
        panic!()
    };
    assert!(plain.deny_contract.is_none());
    let Item::Extend(extend) = &file.items[2] else {
        panic!()
    };
    assert_eq!(
        extend.methods[0].deny_contract.as_ref().unwrap().denied,
        vec![DenyCapability::Panic]
    );
}

#[test]
fn invalid_capability_names_point_to_the_argument() {
    for name in ["network", "unsafe", "Unsafe", "IO", "no_alloc"] {
        let source = format!("@deny({name}) fn f() {{}}");
        let error = parse_source(&source).err().unwrap();
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            name
        );
        assert!(error.message.contains("unknown public deny capability"));
    }
}

#[test]
fn empty_or_malformed_attributes_are_rejected() {
    for prefix in [
        "@deny()",
        "@deny(alloc) @deny()",
        "@deny",
        "@deny alloc",
        "@deny(,)",
        "@deny(alloc,,panic)",
        "@deny(alloc panic)",
        "@deny(\"alloc\")",
        "@deny(1)",
        "@deny(alloc",
        "@unsafe",
        "@verified",
        "@discardable",
        "@no_alloc",
        "@no_panic",
        "@deterministic",
        "@allow(io)",
    ] {
        let source = format!("{prefix} fn f() {{}}");
        assert!(parse_source(&source).is_err(), "accepted {source}");
    }
}

#[test]
fn unsupported_placements_are_rejected() {
    for source in [
        "@deny(io) pub extern fn f()",
        "@deny(io) extern fn f()",
        "@deny(io) struct S {}",
        "@deny(io) enum E { A }",
        "@deny(io) interface I {}",
        "@deny(io) import std.io;",
        "@deny(io) let x = 1",
        "@deny(io) {}",
        "@deny(io) fn() {}",
        "@deny(io) fn [x]() {}",
        "pub @deny(io) fn f() {}",
        "@deny(io) extend Int {}",
        "struct S { @deny(io) x: Int }",
        "interface I { @deny(io) fn f() }",
        "fn f() { @deny(io) fn g() {} }",
        "extend Int { @deny(io) extern fn f() }",
    ] {
        assert!(parse_source(source).is_err(), "accepted {source}");
    }
}
