use tempra_driver::{CapabilityContractViolation, DriverError, VmCompilation};
use tempra_effects::{Capability, DirectCapabilitySource};

fn compile(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    tempra_driver::compile_to_bytecode_with_analysis(source, &dir.path().join("fresh.tbc"))
}

fn violations(source: &str) -> Vec<CapabilityContractViolation> {
    match compile(source).unwrap_err() {
        DriverError::CapabilityContractViolations(violations) => violations,
        error => panic!("expected contract violations, got {error:?}"),
    }
}

#[test]
fn unannotated_effects_and_disjoint_contracts_execute() {
    for contract in ["", "@deny(io)"] {
        let compilation = compile(&format!(
            "{contract} fn main() -> Int64 {{ let text = \"allocates\"; return 84 / 2 }}"
        ))
        .unwrap();
        assert_eq!(
            tempra_vm::run_function(compilation.bytecode(), "main", vec![]).unwrap(),
            tempra_vm::VmValue::Int64(42)
        );
        let main = &compilation.tir().functions[0];
        let required = compilation
            .capabilities()
            .summary(main.id)
            .unwrap()
            .required;
        assert!(required.contains(Capability::Alloc));
        assert!(required.contains(Capability::Panic));
    }
}

#[test]
fn direct_alloc_and_conservative_panic_are_enforced() {
    for (source, capability) in [
        (
            "@deny(alloc) fn main() -> String { return \"x\" }",
            Capability::Alloc,
        ),
        // E1 intentionally classifies Div as may-panic, even for 10 / 2.
        // This is not verified reasoning about a known nonzero divisor.
        (
            "@deny(panic) fn main() -> Int64 { return 10 / 2 }",
            Capability::Panic,
        ),
    ] {
        let errors = violations(source);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].capability, capability);
        assert!(errors[0].witness.calls.is_empty());
        assert_eq!(
            errors[0].witness.origin.location.function,
            errors[0].function
        );
    }
}

#[test]
fn transitive_non_entry_violations_are_aggregated_with_exact_e1_witnesses() {
    let source = r#"
@deny(panic, alloc, alloc)
@deny(panic)
fn a() -> Int64 { return b() }
fn b() -> Int64 { return c() }
fn c() -> Int64 { let text = "origin"; return 10 / 2 }
@deny(alloc, panic)
fn detached() -> Int64 { let text = "unused"; return 20 / 2 }
fn main() -> Int64 { return 42 }
"#;
    // Independently use the real lowerer and E1 without driver enforcement,
    // retaining exact source offsets and the same declaration order/IDs.
    let ast = tempra_parser::parse_source(source).unwrap();
    let mut hir = tempra_hir::lower_file(source, &ast);
    let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
    let mut builder = tempra_tir::builder::ModuleBuilder::new();
    for item in &hir.items {
        if let tempra_hir::HirItem::Function(function) = item {
            let lowered = tempra_tir_lower::lower::lower_function(
                function,
                &context,
                &mut builder,
                None,
                None,
            )
            .unwrap();
            builder.add_function(lowered);
        }
    }
    tempra_tir::verifier::verify_module(&builder.module).unwrap();
    let analysis = tempra_effects::analyze(&builder.module).unwrap();
    let errors = violations(source);
    assert_eq!(errors.len(), 4);
    let keys: Vec<_> = errors
        .iter()
        .map(|v| (v.function.0, v.capability as u8))
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(errors[0].contract.attribute_spans.len(), 2);
    assert_eq!(errors[0].witness.calls.len(), 2);
    assert_eq!(errors[1].witness.calls.len(), 2);
    for error in &errors {
        assert_eq!(
            error.witness,
            analysis.witness(error.function, error.capability).unwrap()
        );
    }
    for _ in 0..8 {
        assert_eq!(violations(source), errors);
    }
}

#[test]
fn multi_module_transitive_io_uses_existing_extern_witness() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("c.tmpr"),
        "import std.io; pub fn work() { std.io.println(\"origin\") }",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.tmpr"),
        "import c; pub fn work() { c.work() }",
    )
    .unwrap();
    let main = dir.path().join("app.tmpr");
    std::fs::write(&main, "import b; @deny(io) pub fn main() { b.work() }").unwrap();
    let DriverError::CapabilityContractViolations(errors) =
        tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap_err()
    else {
        panic!("expected contract error")
    };
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].capability, Capability::Io);
    assert_eq!(errors[0].witness.calls.len(), 2);
    assert!(matches!(&errors[0].witness.origin.source,
        DirectCapabilitySource::ExternCall { name, .. } if name == "std.io.println"));
}

#[test]
fn known_empty_extern_does_not_violate_a_contract() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.tmpr");
    std::fs::write(&path, "import std.strings; @deny(io, panic) pub fn main() -> Bool { return std.strings.contains(\"abc\", \"b\") }").unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap();
    assert_eq!(
        tempra_vm::run_function_with_resolver(
            compilation.bytecode(),
            "app.main",
            vec![],
            Default::default(),
            Some(tempra_native::registry::resolve)
        )
        .unwrap(),
        tempra_vm::VmValue::Bool(true)
    );
}

#[test]
fn all_source_execution_adapters_reject_any_contract_before_execution() {
    for source in [
        "@deny(panic) fn main() -> Int64 { return 10 / 2 }",
        "@deny(io) fn unused() {} fn main() -> Int64 { return 42 }",
        "@deny(io) fn generic(type T, value: T) -> T { return value } fn main() {}",
        "extend Int64 { @deny(io) fn same(self) -> Int64 { return self } } fn main() {}",
    ] {
        let ast = tempra_parser::parse_source(source).unwrap();
        let hir = tempra_hir::lower_file(source, &ast);
        assert!(hir.contains_capability_contracts());
        assert!(
            matches!(tempra_interpreter::run(&hir), Err(tempra_interpreter::RuntimeError::Message(message)) if message.contains("Capability-contract enforcement is not supported"))
        );
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
    }
}

#[test]
fn raw_tir_interpretation_remains_independent_of_source_contracts() {
    let source = "@deny(panic) fn main() -> Int64 { return 10 / 2 }";
    let ast = tempra_parser::parse_source(source).unwrap();
    let mut hir = tempra_hir::lower_file(source, &ast);
    let context = tempra_typeck::check_file_with_context(&mut hir).unwrap();
    let tempra_hir::HirItem::Function(function) = &hir.items[0] else {
        panic!()
    };
    let mut builder = tempra_tir::builder::ModuleBuilder::new();
    let lowered =
        tempra_tir_lower::lower::lower_function(function, &context, &mut builder, None, None)
            .unwrap();
    builder.add_function(lowered);
    assert_eq!(
        tempra_tir_interp::interpret_function(&builder.module, "main", vec![]).unwrap(),
        tempra_tir_interp::TirRuntimeValue::Int64(5)
    );
    // Source entry point rejects before losing the same declaration's metadata.
    assert!(matches!(
        tempra_driver::pipeline::interpret_tir_source(source, "main", vec![]),
        Err(DriverError::RuntimeError(_))
    ));
}

#[test]
fn normal_and_profiled_vm_execute_a_real_enforcement_aware_cache_hit() {
    // Public APIs use the default cache path. Restore any prior artifact even
    // if an assertion fails; all other cache cases use isolated temp paths.
    struct RestoreCache {
        path: std::path::PathBuf,
        previous: Option<Vec<u8>>,
    }
    impl Drop for RestoreCache {
        fn drop(&mut self) {
            if let Some(bytes) = &self.previous {
                let _ = std::fs::write(&self.path, bytes);
            } else {
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }
    let source = "// e32 trusted hit regression\n@deny(io) fn main() -> Int64 { return 246 / 2 }";
    let path =
        tempra_driver::cache::get_cache_path(&tempra_driver::cache::compute_source_hash(source));
    let _restore = RestoreCache {
        previous: std::fs::read(&path).ok(),
        path: path.clone(),
    };
    let compilation = tempra_driver::compile_to_bytecode_with_analysis(source, &path).unwrap();
    assert_eq!(compilation.contracts().count(), 1);
    let before = std::fs::read(&path).unwrap();
    let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
    assert_eq!(
        tempra_driver::run_source_vm(source, &[]).unwrap(),
        tempra_vm::VmValue::Int64(123)
    );
    assert_eq!(
        tempra_driver::run_source_vm_profiled(source, Default::default())
            .unwrap()
            .0,
        tempra_vm::VmValue::Int64(123)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        std::fs::metadata(&path).unwrap().modified().unwrap(),
        modified
    );
}
