use tempra_driver::{CapabilityContractViolation, ContractSource, DriverError, VmCompilation};
use tempra_effects::Capability;
use tempra_vm::VmValue;

fn compile(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    tempra_driver::compile_to_bytecode_with_analysis(source, &dir.path().join("fresh.tbc"))
}

fn violations(source: &str) -> Vec<CapabilityContractViolation> {
    let DriverError::CapabilityContractViolations(errors) = compile(source).unwrap_err() else {
        panic!("expected unchanged E3.2 violation")
    };
    errors
}

#[test]
fn pure_intent_survives_frontend_and_normalizes_only_in_driver() {
    let source = "// è\n@pure\nfn f() {} fn main() {}";
    let ast = tempra_parser::parse_source(source).unwrap();
    let tempra_ast::Item::Function(function) = &ast.items[0] else {
        panic!()
    };
    let span = function.pure_span.unwrap();
    assert!(function.deny_contract.is_none());
    let mut hir = tempra_hir::lower_file(source, &ast);
    tempra_typeck::check_file_with_context(&mut hir).unwrap();
    let tempra_hir::HirItem::Function(lowered) = &hir.items[0] else {
        panic!()
    };
    assert_eq!(lowered.pure_span, Some(span));
    assert!(lowered.deny_contract.is_none());
    assert!(hir.contains_capability_contracts());
    let compilation = compile(source).unwrap();
    let (id, contract) = compilation.contracts().next().unwrap();
    assert_eq!(compilation.contracts().count(), 1);
    assert_eq!(compilation.contract(id), Some(contract));
    assert_eq!(
        contract.denied.iter().collect::<Vec<_>>(),
        vec![
            Capability::Io,
            Capability::Process,
            Capability::Spawn,
            Capability::Nondeterminism
        ]
    );
    for allowed in [Capability::Alloc, Capability::Panic, Capability::Unsafe] {
        assert!(!contract.denied.contains(allowed));
    }
    assert_eq!(contract.pure_span, Some(span));
    assert_eq!(contract.explicit_denied.iter().count(), 0);
    assert!(contract.attribute_spans.is_empty());
    assert_eq!(contract.declaration_span, function.span);
    assert_eq!(contract.source, ContractSource::InMemory);
    assert_eq!(&source[span.start as usize..span.end as usize], "@pure");
    let main = compilation
        .tir()
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    assert!(compilation.contract(main.id).is_none());
}

#[test]
fn composition_is_union_in_either_order_without_synthetic_deny_spans() {
    for prefix in [
        "@pure @deny(alloc, panic, io, io)",
        "@deny(alloc, panic, io, io) @pure",
    ] {
        let source = format!("{prefix} fn main() {{}}");
        let compilation = compile(&source).unwrap();
        let (_, contract) = compilation.contracts().next().unwrap();
        assert_eq!(
            contract.denied.iter().collect::<Vec<_>>(),
            vec![
                Capability::Alloc,
                Capability::Panic,
                Capability::Io,
                Capability::Process,
                Capability::Spawn,
                Capability::Nondeterminism
            ]
        );
        assert_eq!(contract.attribute_spans.len(), 1);
        assert_eq!(
            contract.explicit_denied.iter().collect::<Vec<_>>(),
            vec![Capability::Alloc, Capability::Panic, Capability::Io]
        );
        let explicit = contract.attribute_spans[0];
        assert_eq!(
            &source[explicit.start as usize..explicit.end as usize],
            "@deny(alloc, panic, io, io)"
        );
        let pure = contract.pure_span.unwrap();
        assert_eq!(&source[pure.start as usize..pure.end as usize], "@pure");
    }
    let compilation = compile("@pure @deny(io) fn main() {}").unwrap();
    assert_eq!(
        compilation
            .contracts()
            .next()
            .unwrap()
            .1
            .denied
            .iter()
            .count(),
        4
    );
}

#[test]
fn pure_allows_alloc_panic_and_result_but_composed_deny_still_enforces() {
    let alloc = compile("@pure fn main() -> String { return \"allocated\" }").unwrap();
    let (id, _) = alloc.contracts().next().unwrap();
    assert!(alloc
        .capabilities()
        .summary(id)
        .unwrap()
        .required
        .contains(Capability::Alloc));
    assert_eq!(
        tempra_vm::run_function(alloc.bytecode(), "main", vec![]).unwrap(),
        VmValue::String("allocated".into())
    );

    let source = "@pure fn maybe_panics(a: Int, b: Int) -> Int { return a / b } fn main() -> Int { return maybe_panics(10, 2) }";
    let panic = compile(source).unwrap();
    let (id, _) = panic.contracts().next().unwrap();
    assert!(panic
        .capabilities()
        .summary(id)
        .unwrap()
        .required
        .contains(Capability::Panic));
    assert_eq!(
        tempra_vm::run_function(panic.bytecode(), "main", vec![]).unwrap(),
        VmValue::Int64(5)
    );
    let errors = violations(&source.replace("@pure", "@pure @deny(panic)"));
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].capability, Capability::Panic);
    assert!(errors[0].contract.pure_span.is_some());
    let errors = violations("@pure @deny(alloc) fn main() -> String { return \"allocated\" }");
    assert_eq!(errors[0].capability, Capability::Alloc);

    let result = compile("@pure fn value() -> Result(Int64, String) { return Ok(42) } fn main() -> Result(Int64, String) { return value() }").unwrap();
    tempra_vm::run_function(result.bytecode(), "main", vec![]).unwrap();
}

#[test]
fn pure_io_uses_existing_direct_and_transitive_witnesses_and_non_entry_policy() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("inner.tmpr"),
        "import std.io; pub fn log() { std.io.println(\"origin\") }",
    )
    .unwrap();
    let path = dir.path().join("app.tmpr");
    for source in [
        "import std.io; @pure pub fn main() { std.io.println(\"origin\") }",
        "import inner; @pure pub fn main() { inner.log() }",
        "import std.io; @pure fn unused() { std.io.println(\"origin\") } pub fn main() {}",
    ] {
        std::fs::write(&path, source).unwrap();
        let DriverError::CapabilityContractViolations(errors) =
            tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap_err()
        else {
            panic!()
        };
        assert_eq!(errors.len(), 1);
        let violation = &errors[0];
        assert_eq!(violation.capability, Capability::Io);
        let span = violation.contract.pure_span.unwrap();
        assert_eq!(&source[span.start as usize..span.end as usize], "@pure");
        assert!(violation.contract.attribute_spans.is_empty());
        assert!(
            matches!(&violation.contract.source, ContractSource::Module { name, .. } if name == "app")
        );
        assert_eq!(
            violation.witness.calls.len(),
            usize::from(source.contains("import inner"))
        );

        // Remove the marker only to retain a successful E1 product for comparison.
        // No names/declaration order or runtime operations change.
        std::fs::write(&path, source.replace("@pure", "     ")).unwrap();
        let baseline = tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap();
        assert_eq!(
            violation.witness,
            baseline
                .capabilities()
                .witness(violation.function, Capability::Io)
                .unwrap()
        );
        // Equivalent explicit restriction yields exactly the same E3.2 witness.
        std::fs::write(&path, source.replace("@pure", "@deny(io)")).unwrap();
        let DriverError::CapabilityContractViolations(explicit) =
            tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap_err()
        else {
            panic!()
        };
        assert_eq!(violation.witness, explicit[0].witness);
        assert!(explicit[0].contract.pure_span.is_none());
    }
}

#[test]
fn pure_and_explicit_violations_aggregate_in_existing_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.tmpr");
    std::fs::write(&path, "import std.io; @pure @deny(alloc, panic) fn unused() { std.io.println(\"x\"); let n = 10 / 2 } @pure fn other() { std.io.println(\"y\") } pub fn main() {}").unwrap();
    let mut previous = None;
    for _ in 0..5 {
        let DriverError::CapabilityContractViolations(errors) =
            tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap_err()
        else {
            panic!()
        };
        assert_eq!(errors.len(), 4);
        assert_eq!(
            errors.iter().map(|v| v.capability).collect::<Vec<_>>(),
            vec![
                Capability::Alloc,
                Capability::Panic,
                Capability::Io,
                Capability::Io
            ]
        );
        let keys: Vec<_> = errors
            .iter()
            .map(|v| (v.function.0, v.capability as u8))
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(errors.iter().all(|v| v.contract.pure_span.is_some()));
        if let Some(previous) = &previous {
            assert_eq!(&errors, previous);
        }
        previous = Some(errors);
    }
}

#[test]
fn pure_reuses_all_execution_guards_but_not_frontend_or_emit_enforcement() {
    for source in [
        "@pure fn main() -> Int64 { return 10 / 2 }",
        "@pure fn unused() {} fn main() {}",
        "@pure fn generic(type T, value: T) -> T { return value } fn main() {}",
        "extend Int64 { @pure fn same(self) -> Int64 { return self } } fn main() {}",
    ] {
        let ast = tempra_parser::parse_source(source).unwrap();
        let hir = tempra_hir::lower_file(source, &ast);
        for result in [
            tempra_interpreter::run(&hir),
            tempra_interpreter::run_with_args(&hir, vec![]),
        ] {
            assert!(
                matches!(result, Err(tempra_interpreter::RuntimeError::Message(message)) if message.contains("Capability-contract enforcement is not supported"))
            );
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.tmpr");
        std::fs::write(&path, source).unwrap();
        for result in [
            tempra_driver::run_source(source),
            tempra_driver::run_file(&path),
            tempra_driver::pipeline::test_source(source),
            tempra_driver::pipeline::interpret_tir_source(source, "main", vec![]).map(|_| ()),
        ] {
            assert!(
                matches!(result, Err(DriverError::RuntimeError(message)) if message.contains("Capability-contract enforcement is not supported"))
            );
        }
        tempra_driver::check_source(source).unwrap();
    }
    tempra_driver::emit_tir_source("@pure @deny(panic) fn main() -> Int64 { return 10 / 2 }")
        .unwrap();
}

#[test]
fn pure_extend_method_retains_its_original_marker() {
    let source = "extend Int64 { @pure fn same(self) -> Int64 { return self } } fn main() {}";
    let compilation = compile(source).unwrap();
    let (id, contract) = compilation.contracts().next().unwrap();
    assert_eq!(
        compilation
            .tir()
            .functions
            .iter()
            .find(|f| f.id == id)
            .unwrap()
            .name,
        "Int64.same"
    );
    let span = contract.pure_span.unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], "@pure");
}
