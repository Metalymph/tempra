use tempra_driver::{
    ContractSource, DriverError, SpecializedDenyValue, SpecializedMatch, VmCompilation,
};
use tempra_effects::{Capability, CapabilitySet, EffectOperation, EffectParameter, ResourceValue};
use tempra_tir::ir::TirFunctionId;

fn compile(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    tempra_driver::compile_to_bytecode_with_analysis(source, &dir.path().join("fresh.tbc"))
}

fn id(compilation: &VmCompilation, name: &str) -> TirFunctionId {
    compilation
        .tir()
        .functions
        .iter()
        .find(|function| function.name == name)
        .unwrap()
        .id
}

#[test]
fn specialized_syntax_survives_hir_lowering_and_typechecking() {
    let source = "@deny(io)\n@deny(net.connect(host: \"db.internal\", port: 443, protocol: *))\nfn f() -> Int64 { return 1 }";
    let ast = tempra_parser::parse_source(source).unwrap();
    let tempra_ast::Item::Function(function) = &ast.items[0] else {
        panic!()
    };
    let mut hir = tempra_hir::lower_file(source, &ast);
    tempra_typeck::check_file_with_context(&mut hir).unwrap();
    let tempra_hir::HirItem::Function(lowered) = &hir.items[0] else {
        panic!()
    };
    assert_eq!(function.deny_contract, lowered.deny_contract);
    assert_eq!(function.span, lowered.span);
    let contract = lowered.deny_contract.as_ref().unwrap();
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].target,
        tempra_ast::SpecializedDenyTarget::Operation(tempra_ast::SpecializedOperation::NetConnect)
    );
    assert_eq!(
        &source[contract.specialized_denied[0].span.start as usize
            ..contract.specialized_denied[0].span.end as usize],
        "net.connect(host: \"db.internal\", port: 443, protocol: *)"
    );
}

#[test]
fn mixed_broad_and_specialized_contracts_are_canonical_and_retain_spans() {
    let source = "// è\n@deny(io)\n@deny(net.connect(port: 5432, host: \"db.internal\", protocol: *))\n@deny(fs.write(destination: \"/private\"))\n@deny(fs.read(source: \"/config\"))\npub fn f() -> Int64 { return 1 }";
    let compilation = compile(source).unwrap();
    let contract = compilation.contract(id(&compilation, "f")).unwrap();

    assert_eq!(
        contract.explicit_denied,
        CapabilitySet::singleton(Capability::Io)
    );
    assert_eq!(contract.denied, CapabilitySet::singleton(Capability::Io));
    assert!(contract.pure_span.is_none());
    assert_eq!(contract.source, ContractSource::InMemory);
    assert_eq!(
        &source[contract.declaration_span.start as usize..contract.declaration_span.end as usize],
        "fn f() -> Int64 { return 1 }"
    );
    assert_eq!(contract.attribute_spans.len(), 4);
    for span in &contract.attribute_spans {
        assert!(source[span.start as usize..span.end as usize].starts_with("@deny("));
    }

    assert_eq!(
        contract
            .specialized_denied
            .iter()
            .map(|clause| clause.operation)
            .collect::<Vec<_>>(),
        vec![
            EffectOperation::FsRead,
            EffectOperation::FsWrite,
            EffectOperation::NetConnect
        ]
    );

    let read = &contract.specialized_denied[0];
    assert_eq!(
        &source[read.span.start as usize..read.span.end as usize],
        "fs.read(source: \"/config\")"
    );
    assert_eq!(read.parameters.len(), 1);
    assert_eq!(read.parameters[0].parameter, EffectParameter::Source);
    assert_eq!(
        read.parameters[0].value,
        SpecializedDenyValue::Exact(ResourceValue::Text("/config".to_owned()))
    );
    assert_eq!(
        &source[read.parameters[0].span.start as usize..read.parameters[0].span.end as usize],
        "source: \"/config\""
    );

    let write = &contract.specialized_denied[1];
    assert_eq!(
        &source[write.span.start as usize..write.span.end as usize],
        "fs.write(destination: \"/private\")"
    );
    assert_eq!(
        write.parameters[0].value,
        SpecializedDenyValue::Exact(ResourceValue::Text("/private".to_owned()))
    );

    let connect = &contract.specialized_denied[2];
    assert_eq!(
        connect
            .parameters
            .iter()
            .map(|parameter| parameter.parameter)
            .collect::<Vec<_>>(),
        vec![
            EffectParameter::Host,
            EffectParameter::Port,
            EffectParameter::Protocol
        ]
    );
    assert_eq!(
        connect.parameters[0].value,
        SpecializedDenyValue::Exact(ResourceValue::Text("db.internal".to_owned()))
    );
    assert_eq!(
        connect.parameters[1].value,
        SpecializedDenyValue::Exact(ResourceValue::Port(5432))
    );
    assert_eq!(connect.parameters[2].value, SpecializedDenyValue::Any);
}

#[test]
fn duplicate_clauses_are_removed_after_parameter_canonicalization() {
    let source = "@deny(fs.write( destination: \"/x\" ), fs.write(destination: \"/x\"))\n@deny(net.connect(host: \"h\", port: 1), net.connect(port: 1, host: \"h\"))\n@deny(fs.write(destination: \"/y\"))\nfn f() -> Int64 { return 1 }";
    let compilation = compile(source).unwrap();
    let contract = compilation.contract(id(&compilation, "f")).unwrap();

    assert_eq!(contract.specialized_denied.len(), 3);
    assert_eq!(
        contract
            .specialized_denied
            .iter()
            .map(|clause| clause.operation)
            .collect::<Vec<_>>(),
        vec![
            EffectOperation::FsWrite,
            EffectOperation::FsWrite,
            EffectOperation::NetConnect
        ]
    );
    assert_eq!(
        &source[contract.specialized_denied[0].span.start as usize
            ..contract.specialized_denied[0].span.end as usize],
        "fs.write( destination: \"/x\" )"
    );
    assert_eq!(
        contract.specialized_denied[1].parameters[0].value,
        SpecializedDenyValue::Exact(ResourceValue::Text("/y".to_owned()))
    );
    assert_eq!(
        contract.specialized_denied[2]
            .parameters
            .iter()
            .map(|parameter| parameter.parameter)
            .collect::<Vec<_>>(),
        vec![EffectParameter::Host, EffectParameter::Port]
    );
    assert_eq!(contract.denied, CapabilitySet::empty());
}

#[test]
fn pure_semantics_are_unchanged_and_specialized_clauses_remain_separate() {
    let source = "@pure @deny(fs.write(destination: \"x\"))\nfn f() -> Int64 { return 1 }";
    let compilation = compile(source).unwrap();
    let contract = compilation.contract(id(&compilation, "f")).unwrap();
    let pure_denied = [
        Capability::Io,
        Capability::Process,
        Capability::Spawn,
        Capability::Nondeterminism,
    ]
    .into_iter()
    .fold(CapabilitySet::empty(), |set, capability| {
        set.union(CapabilitySet::singleton(capability))
    });
    assert_eq!(contract.explicit_denied, CapabilitySet::empty());
    assert_eq!(contract.denied, pure_denied);
    assert!(contract.pure_span.is_some());
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
}

#[test]
fn broad_denial_remains_authoritative_with_specialized_clauses() {
    let source = "import std.io;\n@deny(io, fs.write(destination: \"x\"))\npub fn main() { std.io.println(\"blocked\") }";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("broad.tmpr");
    std::fs::write(&path, source).unwrap();
    let DriverError::CapabilityContractViolations(violations) =
        tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap_err()
    else {
        panic!("expected broad Io enforcement")
    };
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].capability, Capability::Io);
    assert_eq!(
        violations[0].contract.explicit_denied,
        CapabilitySet::singleton(Capability::Io)
    );
    assert_eq!(violations[0].contract.specialized_denied.len(), 1);
    assert_eq!(
        violations[0].contract.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
}

#[test]
fn specialized_only_contracts_are_enforced_without_broad_denial() {
    let source = "import std.fs;\n@deny(fs.write(destination: \"blocked.txt\"))\npub fn main() -> Int64 {\n    let result = std.fs.write_text(\"blocked.txt\", \"data\");\n    return match result { Ok(_) => 0, Err(_) => 1 };\n}";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("specialized.tmpr");
    std::fs::write(&path, source).unwrap();
    let DriverError::SpecializedContractViolations(violations) =
        tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap_err()
    else {
        panic!("expected specialized enforcement")
    };
    assert_eq!(violations.len(), 1);
    let violation = &violations[0];
    assert_eq!(violation.contract.denied, CapabilitySet::empty());
    assert_eq!(violation.contract.explicit_denied, CapabilitySet::empty());
    assert_eq!(violation.contract.specialized_denied.len(), 1);
    assert_eq!(violation.clause.operation, EffectOperation::FsWrite);
    assert_eq!(violation.classification, SpecializedMatch::DefiniteMatch);
    assert_eq!(
        &source[violation.clause.span.start as usize..violation.clause.span.end as usize],
        "fs.write(destination: \"blocked.txt\")"
    );
}

#[test]
fn specialized_contract_binding_collisions_are_still_detected() {
    for source in [
        "@deny(fs.write(destination: \"x\")) fn f() {}\nfn f() {}",
        "fn f() {}\n@deny(fs.write(destination: \"x\")) fn f() {}",
        "@deny(fs.write(destination: \"x\")) fn f() {}\n@deny(fs.write(destination: \"x\")) fn f() {}",
    ] {
        let Err(DriverError::ContractBindingError(error)) = compile(source) else {
            panic!("expected binding failure for {source}")
        };
        assert_ne!(error.previous_span, error.incoming_span);
        assert_eq!(error.previous_source, ContractSource::InMemory);
        assert_eq!(error.incoming_source, ContractSource::InMemory);
    }
}

#[test]
fn family_and_category_targets_survive_hir_lowering_without_flattening() {
    let source =
        "@deny(fs, net, io.read, io.write, io.read(), net.send()) fn f() -> Int64 { return 1 }";
    let ast = tempra_parser::parse_source(source).unwrap();
    let tempra_ast::Item::Function(function) = &ast.items[0] else {
        panic!()
    };
    let mut hir = tempra_hir::lower_file(source, &ast);
    tempra_typeck::check_file_with_context(&mut hir).unwrap();
    let tempra_hir::HirItem::Function(lowered) = &hir.items[0] else {
        panic!()
    };
    assert_eq!(function.deny_contract, lowered.deny_contract);

    let contract = lowered.deny_contract.as_ref().unwrap();
    let targets: Vec<_> = contract
        .specialized_denied
        .iter()
        .map(|clause| clause.target)
        .collect();
    assert_eq!(
        targets,
        vec![
            tempra_ast::SpecializedDenyTarget::Family(tempra_ast::SpecializedFamily::Fs),
            tempra_ast::SpecializedDenyTarget::Family(tempra_ast::SpecializedFamily::Net),
            tempra_ast::SpecializedDenyTarget::Category(tempra_ast::SpecializedCategory::IoRead),
            tempra_ast::SpecializedDenyTarget::Category(tempra_ast::SpecializedCategory::IoWrite),
            tempra_ast::SpecializedDenyTarget::Operation(tempra_ast::SpecializedOperation::IoRead),
            tempra_ast::SpecializedDenyTarget::Operation(tempra_ast::SpecializedOperation::NetSend),
        ]
    );
    assert_eq!(
        contract
            .specialized_denied
            .iter()
            .map(|clause| clause.syntax)
            .collect::<Vec<_>>(),
        vec![
            tempra_ast::SpecializedDenySyntax::Bare,
            tempra_ast::SpecializedDenySyntax::Bare,
            tempra_ast::SpecializedDenySyntax::Bare,
            tempra_ast::SpecializedDenySyntax::Bare,
            tempra_ast::SpecializedDenySyntax::Parenthesized,
            tempra_ast::SpecializedDenySyntax::Parenthesized,
        ]
    );
}

#[test]
fn parameterless_existing_operations_remain_enforceable() {
    let source = "@deny(fs.read, fs.write, fs.remove, net.connect, net.listen, process.exec, process.exit, spawn.task, spawn.actor, io.read(), io.write())\nfn f() -> Int64 { return 1 }";
    let compilation = compile(source).unwrap();
    let contract = compilation.contract(id(&compilation, "f")).unwrap();
    assert_eq!(
        contract
            .specialized_denied
            .iter()
            .map(|clause| clause.operation)
            .collect::<Vec<_>>(),
        vec![
            EffectOperation::FsRead,
            EffectOperation::FsWrite,
            EffectOperation::FsRemove,
            EffectOperation::NetConnect,
            EffectOperation::NetListen,
            EffectOperation::IoRead,
            EffectOperation::IoWrite,
            EffectOperation::ProcessExec,
            EffectOperation::ProcessExit,
            EffectOperation::SpawnTask,
            EffectOperation::SpawnActor,
        ]
    );
}

#[test]
fn unenforced_family_category_and_new_network_targets_are_rejected_at_driver_boundary() {
    for (source, target, expected_span) in [
        ("@deny(fs) fn f() -> Int64 { return 1 }", "fs", "fs"),
        ("@deny(net) fn f() -> Int64 { return 1 }", "net", "net"),
        (
            "@deny(io.read) fn f() -> Int64 { return 1 }",
            "io.read",
            "io.read",
        ),
        (
            "@deny(io.write) fn f() -> Int64 { return 1 }",
            "io.write",
            "io.write",
        ),
        (
            "@deny(net.send) fn f() -> Int64 { return 1 }",
            "net.send",
            "net.send",
        ),
        (
            "@deny(net.receive) fn f() -> Int64 { return 1 }",
            "net.receive",
            "net.receive",
        ),
        (
            "@deny(net.send()) fn f() -> Int64 { return 1 }",
            "net.send",
            "net.send()",
        ),
        (
            "@deny(net.receive()) fn f() -> Int64 { return 1 }",
            "net.receive",
            "net.receive()",
        ),
    ] {
        let Err(DriverError::UnsupportedSpecializedTarget(error)) = compile(source) else {
            panic!("expected unsupported target rejection for {source}")
        };
        assert_eq!(error.target, target);
        assert_eq!(
            &source[error.span.start as usize..error.span.end as usize],
            expected_span
        );
    }
}
