use tempra_ast::{
    DenyCapability, Item, SpecializedOperation, SpecializedParameter, SpecializedValue,
};
use tempra_parser::parse_source;

fn function_contract(
    source: &str,
) -> (
    tempra_ast::FunctionItem,
    tempra_ast::DenyContract,
    Option<tempra_span::Span>,
) {
    let file = parse_source(source).unwrap();
    let Item::Function(function) = &file.items[0] else {
        panic!("expected function")
    };
    (
        function.clone(),
        function.deny_contract.clone().unwrap(),
        function.pure_span,
    )
}

#[test]
fn broad_only_deny_keeps_its_existing_shape() {
    let source = "@deny(alloc, alloc)\n@deny(io,)\nfn f() {}";
    let (function, contract, pure) = function_contract(source);
    assert_eq!(
        contract.denied,
        vec![DenyCapability::Alloc, DenyCapability::Io]
    );
    assert!(contract.specialized_denied.is_empty());
    assert_eq!(contract.attribute_spans.len(), 2);
    assert!(pure.is_none());
    assert_eq!(
        &source[function.span.start as usize..function.span.end as usize],
        "fn f() {}"
    );
}

#[test]
fn mixed_and_repeated_attributes_preserve_source_spans() {
    let source = "// è\n@deny(io)\n@deny(fs.write(destination: \"/private\"), fs.read(source: \"/config\"))\npub fn f() {}";
    let (_, contract, _) = function_contract(source);

    assert_eq!(contract.denied, vec![DenyCapability::Io]);
    assert_eq!(contract.specialized_denied.len(), 2);
    assert_eq!(contract.attribute_spans.len(), 2);
    assert_eq!(
        &source
            [contract.attribute_spans[0].start as usize..contract.attribute_spans[0].end as usize],
        "@deny(io)"
    );
    assert_eq!(
        &source
            [contract.attribute_spans[1].start as usize..contract.attribute_spans[1].end as usize],
        "@deny(fs.write(destination: \"/private\"), fs.read(source: \"/config\"))"
    );

    let write = &contract.specialized_denied[0];
    assert_eq!(write.operation, SpecializedOperation::FsWrite);
    assert_eq!(
        &source[write.span.start as usize..write.span.end as usize],
        "fs.write(destination: \"/private\")"
    );
    assert_eq!(write.parameters.len(), 1);
    assert_eq!(
        write.parameters[0].parameter,
        SpecializedParameter::Destination
    );
    assert_eq!(
        write.parameters[0].value,
        SpecializedValue::Text("/private".to_owned())
    );
    assert_eq!(
        &source[write.parameters[0].span.start as usize..write.parameters[0].span.end as usize],
        "destination: \"/private\""
    );

    let read = &contract.specialized_denied[1];
    assert_eq!(read.operation, SpecializedOperation::FsRead);
    assert_eq!(read.parameters[0].parameter, SpecializedParameter::Source);
    assert_eq!(
        read.parameters[0].value,
        SpecializedValue::Text("/config".to_owned())
    );
}

#[test]
fn broad_and_specialized_clauses_mix_in_one_attribute() {
    let source = "@deny(io, fs.read(source: \"/config\"), panic)\nfn f() {}";
    let (_, contract, _) = function_contract(source);
    assert_eq!(
        contract.denied,
        vec![DenyCapability::Panic, DenyCapability::Io]
    );
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        SpecializedOperation::FsRead
    );
}

#[test]
fn exact_text_port_and_wildcard_values_are_parsed() {
    let source =
        "@deny(net.connect(host: \"db.internal\", port: 5432, protocol: *), io.write(stream: \"stdout\"))\nfn f() {}";
    let (_, contract, _) = function_contract(source);

    let connect = &contract.specialized_denied[0];
    assert_eq!(connect.operation, SpecializedOperation::NetConnect);
    assert_eq!(
        connect
            .parameters
            .iter()
            .map(|parameter| parameter.parameter)
            .collect::<Vec<_>>(),
        vec![
            SpecializedParameter::Host,
            SpecializedParameter::Port,
            SpecializedParameter::Protocol
        ]
    );
    assert_eq!(
        connect.parameters[0].value,
        SpecializedValue::Text("db.internal".to_owned())
    );
    assert_eq!(connect.parameters[1].value, SpecializedValue::Port(5432));
    assert_eq!(connect.parameters[2].value, SpecializedValue::Any);

    assert_eq!(
        contract.specialized_denied[1].operation,
        SpecializedOperation::IoWrite
    );
    assert_eq!(
        contract.specialized_denied[1].parameters[0].value,
        SpecializedValue::Text("stdout".to_owned())
    );
}

#[test]
fn zero_parameter_and_omitted_parameter_clauses_are_supported() {
    let source = "@deny(process.exit(), fs.read(), spawn.task(scope: \"workers\"))\nfn f() {}";
    let (_, contract, _) = function_contract(source);
    assert_eq!(contract.specialized_denied.len(), 3);
    assert_eq!(
        contract.specialized_denied[0].operation,
        SpecializedOperation::ProcessExit
    );
    assert!(contract.specialized_denied[0].parameters.is_empty());
    assert_eq!(
        contract.specialized_denied[1].operation,
        SpecializedOperation::FsRead
    );
    assert!(contract.specialized_denied[1].parameters.is_empty());
    assert_eq!(
        contract.specialized_denied[2].parameters[0].value,
        SpecializedValue::Text("workers".to_owned())
    );

    let source = "@deny(process.exit)\nfn f() {}";
    let (_, contract, _) = function_contract(source);
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        SpecializedOperation::ProcessExit
    );
    assert_eq!(
        &source[contract.specialized_denied[0].span.start as usize
            ..contract.specialized_denied[0].span.end as usize],
        "process.exit"
    );
}

#[test]
fn pure_and_specialized_attributes_coexist_without_conversion() {
    let source = "@pure @deny(fs.write(destination: \"x\"))\nfn f() {}";
    let (_, contract, pure) = function_contract(source);
    assert!(contract.denied.is_empty());
    assert_eq!(contract.specialized_denied.len(), 1);
    let pure = pure.unwrap();
    assert_eq!(&source[pure.start as usize..pure.end as usize], "@pure");
}

#[test]
fn unknown_operations_are_rejected_with_the_operation_span() {
    for operation in ["fs.delete", "net.fetch", "io.open", "spawn.thread"] {
        let source = format!("@deny({operation}(source: \"x\")) fn f() {{}}");
        let error = parse_source(&source).err().unwrap();
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            operation
        );
        assert!(error.message.contains("unknown specialized deny operation"));
    }
}

#[test]
fn invalid_parameters_are_rejected_with_the_key_span() {
    for (source, parameter) in [
        ("@deny(fs.write(source: \"x\")) fn f() {}", "source"),
        ("@deny(process.exit(host: \"x\")) fn f() {}", "host"),
        (
            "@deny(net.connect(executable: \"x\")) fn f() {}",
            "executable",
        ),
    ] {
        let error = parse_source(source).err().unwrap();
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            parameter
        );
        assert!(error.message.contains("not valid"));
    }
    for parameter in ["path", "Destination", "filesystem"] {
        let source = format!("@deny(fs.write({parameter}: \"x\")) fn f() {{}}");
        let error = parse_source(&source).err().unwrap();
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            parameter
        );
        assert!(error.message.contains("unknown specialized deny parameter"));
    }
}

#[test]
fn duplicate_parameter_keys_are_rejected_with_the_duplicate_span() {
    let source = "@deny(fs.write(destination: \"a\", destination: \"b\")) fn f() {}";
    let error = parse_source(source).err().unwrap();
    assert!(error
        .message
        .contains("duplicate specialized deny parameter"));
    let duplicate = source.rfind("destination").unwrap();
    assert_eq!(error.span.start as usize, duplicate);
    assert_eq!(
        &source[error.span.start as usize..error.span.end as usize],
        "destination"
    );
}

#[test]
fn incompatible_literal_types_are_rejected_with_the_value_span() {
    for (source, value) in [
        ("@deny(fs.write(destination: 1)) fn f() {}", "1"),
        ("@deny(fs.read(source: 5432)) fn f() {}", "5432"),
        ("@deny(net.connect(port: \"5432\")) fn f() {}", "\"5432\""),
        ("@deny(net.connect(port: 65536)) fn f() {}", "65536"),
        ("@deny(fs.read(source: 1.5)) fn f() {}", "1.5"),
    ] {
        let error = parse_source(source).err().unwrap();
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            value
        );
    }
}

#[test]
fn malformed_wildcards_are_rejected() {
    for source in [
        "@deny(fs.read(source: **)) fn f() {}",
        "@deny(fs.read(source: *foo)) fn f() {}",
        "@deny(fs.read(source: * \"x\")) fn f() {}",
        "@deny(fs.read(source: * destination: \"x\")) fn f() {}",
    ] {
        let error = parse_source(source).err().unwrap();
        assert!(error.message.contains("wildcard"));
        let star = source.find('*').unwrap();
        assert_eq!(error.span.start as usize, star);
    }
    parse_source("@deny(fs.read(source: *)) fn f() {}").unwrap();
}

#[test]
fn unsupported_clause_forms_are_rejected() {
    for source in [
        "@deny(fs.read) fn f() {}",
        "@deny(net.connect.extra(host: \"x\")) fn f() {}",
        "@deny(fs.read(source)) fn f() {}",
        "@deny(fs.read(source: \"x\") fn f() {}",
        "@deny(io(alloc)) fn f() {}",
        "@deny(fs.read(source: tcp)) fn f() {}",
        "@deny(fs.read(source: true)) fn f() {}",
        "@deny(fs.read(source: -1)) fn f() {}",
        "@deny(fs.read(source: \"x\") extra) fn f() {}",
    ] {
        assert!(parse_source(source).is_err(), "accepted {source}");
    }
    let source = "@deny(fs.read) fn f() {}";
    let error = parse_source(source).err().unwrap();
    assert_eq!(
        &source[error.span.start as usize..error.span.end as usize],
        "fs.read"
    );
    parse_source("@deny( fs . read ( source : \"x\" ) ) fn f() {}").unwrap();
}
