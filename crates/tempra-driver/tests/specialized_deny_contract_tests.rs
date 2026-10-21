use tempra_driver::{
    canonicalize_specialized_denied, ContractSource, DriverError, SpecializedDenyTarget,
    SpecializedDenyValue, SpecializedMatch, VmCompilation,
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

fn canonical_clauses(source: &str) -> Vec<tempra_driver::SpecializedDenyClause> {
    let ast = tempra_parser::parse_source(source).unwrap();
    let tempra_ast::Item::Function(function) = &ast.items[0] else {
        panic!("expected a function item")
    };
    canonicalize_specialized_denied(
        &function
            .deny_contract
            .as_ref()
            .expect("function must carry a deny contract")
            .specialized_denied,
    )
    .unwrap()
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
            .map(|clause| clause.target.operation().unwrap())
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
            .map(|clause| clause.target.operation().unwrap())
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
        contract.specialized_denied[0].target,
        SpecializedDenyTarget::Operation(EffectOperation::FsWrite)
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
        violations[0].contract.specialized_denied[0].target,
        SpecializedDenyTarget::Operation(EffectOperation::FsWrite)
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
    assert_eq!(
        violation.clause.target,
        SpecializedDenyTarget::Operation(EffectOperation::FsWrite)
    );
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
            .map(|clause| clause.target.operation().unwrap())
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
fn family_category_and_new_network_targets_are_bound_and_enforced_at_driver_boundary() {
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
        let compilation = compile(source).unwrap();
        let clause = &compilation.contracts().next().unwrap().1.specialized_denied[0];
        assert_eq!(clause.target.canonical_name(), target);
        assert_eq!(
            &source[clause.span.start as usize..clause.span.end as usize],
            expected_span
        );
    }
}

#[test]
fn canonical_targets_preserve_family_category_and_new_operation_kinds() {
    let source = "@deny(io.write, net.send, io.read, fs, net, io.read(), net.receive, io.write())\nfn f() -> Int64 { return 1 }";
    let clauses = canonical_clauses(source);
    assert_eq!(
        clauses
            .iter()
            .map(|clause| clause.target)
            .collect::<Vec<_>>(),
        vec![
            SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Fs),
            SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Net),
            SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoRead),
            SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoWrite),
            SpecializedDenyTarget::Operation(EffectOperation::NetSend),
            SpecializedDenyTarget::Operation(EffectOperation::NetReceive),
            SpecializedDenyTarget::Operation(EffectOperation::IoRead),
            SpecializedDenyTarget::Operation(EffectOperation::IoWrite),
        ]
    );

    let category = clauses
        .iter()
        .find(|clause| {
            clause.target
                == SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoWrite)
        })
        .unwrap();
    assert_eq!(
        &source[category.span.start as usize..category.span.end as usize],
        "io.write"
    );
    let operation = clauses
        .iter()
        .find(|clause| clause.target == SpecializedDenyTarget::Operation(EffectOperation::IoWrite))
        .unwrap();
    assert_eq!(
        &source[operation.span.start as usize..operation.span.end as usize],
        "io.write()"
    );
}

#[test]
fn canonicalization_keeps_overlapping_targets_distinct_and_retains_first_span() {
    let source =
        "@deny(io.read, fs.read)\n@deny(io.read, io.read(), fs)\nfn f() -> Int64 { return 1 }";
    let clauses = canonical_clauses(source);
    assert_eq!(clauses.len(), 4);
    assert_eq!(
        clauses
            .iter()
            .map(|clause| clause.target)
            .collect::<Vec<_>>(),
        vec![
            SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Fs),
            SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoRead),
            SpecializedDenyTarget::Operation(EffectOperation::FsRead),
            SpecializedDenyTarget::Operation(EffectOperation::IoRead),
        ]
    );

    let first_category = source.find("io.read").unwrap();
    let category = &clauses[1];
    assert_eq!(category.span.start as usize, first_category);
    assert_eq!(
        &source[category.span.start as usize..category.span.end as usize],
        "io.read"
    );
    assert_eq!(
        &source[clauses[0].span.start as usize..clauses[0].span.end as usize],
        "fs"
    );
    assert_eq!(
        clauses
            .iter()
            .filter(|clause| clause.target
                == SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoRead))
            .count(),
        1,
        "repeated semantically identical category clauses must deduplicate"
    );
}

#[test]
fn canonical_operation_parameters_use_declared_order_and_typed_values() {
    let source = "@deny(net.connect(port: 443, host: \"db.internal\", protocol: *))\nfn f() -> Int64 { return 1 }";
    let clauses = canonical_clauses(source);
    assert_eq!(clauses.len(), 1);
    let clause = &clauses[0];
    assert_eq!(
        clause.target,
        SpecializedDenyTarget::Operation(EffectOperation::NetConnect)
    );
    assert_eq!(
        clause
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
        clause.parameters[0].value,
        SpecializedDenyValue::Exact(ResourceValue::Text("db.internal".to_owned()))
    );
    assert_eq!(
        clause.parameters[1].value,
        SpecializedDenyValue::Exact(ResourceValue::Port(443))
    );
    assert_eq!(clause.parameters[2].value, SpecializedDenyValue::Any);
}

#[test]
fn repeated_broad_and_specialized_attributes_deduplicate_only_specialized_clauses() {
    let source =
        "@deny(io)\n@deny(fs)\n@deny(fs, io.read)\n@deny(io.read)\nfn f() -> Int64 { return 1 }";
    let clauses = canonical_clauses(source);
    assert_eq!(
        clauses
            .iter()
            .map(|clause| clause.target)
            .collect::<Vec<_>>(),
        vec![
            SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Fs),
            SpecializedDenyTarget::Category(tempra_effects::EffectCategory::IoRead),
        ]
    );
    let first_family = source.find("fs").unwrap();
    assert_eq!(clauses[0].span.start as usize, first_family);
}

#[test]
fn canonical_binding_rejects_target_parameter_incompatibilities() {
    use tempra_ast::{
        SpecializedDenyClause as AstClause, SpecializedDenyParameter as AstParameter,
        SpecializedDenySyntax, SpecializedDenyTarget as AstTarget, SpecializedFamily,
        SpecializedOperation, SpecializedParameter, SpecializedValue,
    };
    use tempra_driver::SpecializedDenyBindingError;
    use tempra_span::Span;

    let clause = |target, parameters| AstClause {
        target,
        parameters,
        span: Span::new(0, 0),
        syntax: SpecializedDenySyntax::Bare,
    };
    let parameter = |parameter| AstParameter {
        parameter,
        value: SpecializedValue::Any,
        span: Span::new(0, 0),
    };

    let cases = [
        (
            clause(
                AstTarget::Family(SpecializedFamily::Fs),
                vec![parameter(SpecializedParameter::Source)],
            ),
            SpecializedDenyBindingError::ParametersNotAllowed {
                target: SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Fs),
                span: Span::new(0, 0),
            },
        ),
        (
            clause(
                AstTarget::Operation(SpecializedOperation::NetSend),
                vec![parameter(SpecializedParameter::Source)],
            ),
            SpecializedDenyBindingError::ParameterNotAccepted {
                target: SpecializedDenyTarget::Operation(EffectOperation::NetSend),
                parameter: EffectParameter::Source,
                span: Span::new(0, 0),
            },
        ),
        (
            clause(
                AstTarget::Operation(SpecializedOperation::FsWrite),
                vec![
                    parameter(SpecializedParameter::Destination),
                    parameter(SpecializedParameter::Destination),
                ],
            ),
            SpecializedDenyBindingError::DuplicateParameter {
                target: SpecializedDenyTarget::Operation(EffectOperation::FsWrite),
                parameter: EffectParameter::Destination,
                span: Span::new(0, 0),
            },
        ),
        (
            clause(
                AstTarget::Operation(SpecializedOperation::FsWrite),
                vec![AstParameter {
                    parameter: SpecializedParameter::Destination,
                    value: SpecializedValue::Port(80),
                    span: Span::new(0, 0),
                }],
            ),
            SpecializedDenyBindingError::InvalidParameterValue {
                target: SpecializedDenyTarget::Operation(EffectOperation::FsWrite),
                parameter: EffectParameter::Destination,
                span: Span::new(0, 0),
            },
        ),
    ];

    for (clause, expected) in cases {
        assert_eq!(canonicalize_specialized_denied(&[clause]), Err(expected));
    }
}

#[test]
fn family_targets_are_preserved_in_extend_methods_and_modules() {
    let extend = "extend Int64 { @deny(net) fn same(self) -> Int64 { return self } } fn main() {}";
    let compilation = compile(extend).unwrap();
    let clause = &compilation.contracts().next().unwrap().1.specialized_denied[0];
    assert_eq!(clause.target.canonical_name(), "net");
    assert_eq!(
        &extend[clause.span.start as usize..clause.span.end as usize],
        "net"
    );

    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("helper.tmpr"),
        "pub fn value() -> Int64 { return 1 }",
    )
    .unwrap();
    let main_path = dir.path().join("main.tmpr");
    std::fs::write(
        &main_path,
        "import helper;\n@deny(fs)\npub fn main() -> Int64 { return helper.value() }",
    )
    .unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main_path).unwrap();
    assert_eq!(
        compilation.contracts().next().unwrap().1.specialized_denied[0].target,
        SpecializedDenyTarget::Family(tempra_effects::EffectFamily::Fs)
    );
}

#[test]
fn family_violation_is_rejected_before_program_execution() {
    let dir = tempfile::tempdir().unwrap();
    let written = dir.path().join("must-not-exist.txt");
    let source = format!(
        "import std.fs;\n@deny(fs)\npub fn main() -> Int64 {{\n    let result = std.fs.write_text({:?}, \"data\");\n    return match result {{ Ok(_) => 0, Err(_) => 1 }};\n}}",
        written.display().to_string()
    );
    let path = dir.path().join("unsupported.tmpr");
    std::fs::write(&path, source).unwrap();
    for jit in [false, true] {
        let Err(DriverError::SpecializedContractViolations(violations)) =
            tempra_driver::run_file_with_experimental_vm_jit(&path, &[], jit)
        else {
            panic!("expected family violation before VM/JIT execution")
        };
        assert_eq!(violations.len(), 1);
        assert_eq!(
            violations[0].classification,
            tempra_driver::SpecializedMatch::DefiniteMatch
        );
    }
    assert!(
        !written.exists(),
        "a program violating a family contract must not execute"
    );
}
