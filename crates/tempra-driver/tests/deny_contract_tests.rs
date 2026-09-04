use tempra_driver::{ContractSource, DriverError, VmCompilation};
use tempra_effects::Capability;
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
fn typed_syntax_survives_hir_and_typechecking() {
    let source = "@deny(alloc, panic) fn f() -> String { return \"x\" }";
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
}

#[test]
fn all_public_names_map_canonically_and_alloc_is_not_enforced() {
    let source = "// è\n@deny(nondeterminism, alloc, spawn, process, io, panic, alloc)\n@deny(io)\nfn f() -> String { return \"x\" }\nfn main() -> String { return f() }";
    let compilation = compile(source).unwrap();
    let f = id(&compilation, "f");
    let contract = compilation.contract(f).unwrap();
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
    assert!(!contract.denied.contains(Capability::Unsafe));
    assert_eq!(contract.source, ContractSource::InMemory);
    assert_eq!(
        &source[contract.declaration_span.start as usize..contract.declaration_span.end as usize],
        "fn f() -> String { return \"x\" }"
    );
    assert_eq!(contract.attribute_spans.len(), 2);
    for span in &contract.attribute_spans {
        assert!(source[span.start as usize..span.end as usize].starts_with("@deny("));
    }
    assert!(compilation
        .capabilities()
        .summary(f)
        .unwrap()
        .required
        .contains(Capability::Alloc));
    assert!(compilation.contract(id(&compilation, "main")).is_none());
    assert_eq!(compilation.contracts().count(), 1);
    let result = tempra_vm::run_function(compilation.bytecode(), "main", vec![]).unwrap();
    assert_eq!(result, tempra_vm::VmValue::String("x".into()));
}

#[test]
fn module_io_contract_and_required_io_coexist_without_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.tmpr");
    let source = "import std.io;\n@deny(io)\npub fn main() { std.io.println(\"E3.1\") }";
    std::fs::write(&path, source).unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap();
    let function = id(&compilation, "app.main");
    let contract = compilation.contract(function).unwrap();
    assert!(contract.denied.contains(Capability::Io));
    assert!(compilation
        .capabilities()
        .summary(function)
        .unwrap()
        .required
        .contains(Capability::Io));
    let ContractSource::Module { name, path: origin } = &contract.source else {
        panic!()
    };
    assert_eq!(name, "app");
    assert_eq!(origin.canonicalize().unwrap(), path.canonicalize().unwrap());
    assert_eq!(
        &source
            [contract.attribute_spans[0].start as usize..contract.attribute_spans[0].end as usize],
        "@deny(io)"
    );
    tempra_vm::run_function_with_resolver(
        compilation.bytecode(),
        "app.main",
        vec![],
        Default::default(),
        Some(tempra_native::registry::resolve),
    )
    .unwrap();
}

#[test]
fn non_entry_contract_and_extend_method_bind_to_their_actual_ids() {
    let source = "@deny(alloc) fn detached() -> String { return \"x\" }\nextend Int64 { @deny(panic) fn same(self) -> Int64 { return self } }\nfn main() -> Int64 { return 42 }";
    let compilation = compile(source).unwrap();
    assert!(compilation.contract(id(&compilation, "detached")).is_some());
    assert!(compilation
        .contract(id(&compilation, "Int64.same"))
        .unwrap()
        .denied
        .contains(Capability::Panic));
    assert!(compilation.contract(id(&compilation, "main")).is_none());
    let ids: Vec<_> = compilation.contracts().map(|(id, _)| id.0).collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn same_names_and_offsets_in_distinct_modules_do_not_collide() {
    let dir = tempfile::tempdir().unwrap();
    let a = "@deny(alloc) pub fn work() -> Int64 { return 1 }";
    let b = "@deny(panic) pub fn work() -> Int64 { return 2 }";
    std::fs::write(dir.path().join("a.tmpr"), a).unwrap();
    std::fs::write(dir.path().join("b.tmpr"), b).unwrap();
    let main = dir.path().join("main.tmpr");
    std::fs::write(
        &main,
        "import a;\nimport b;\npub fn main() -> Int64 { return a.work() + b.work() }",
    )
    .unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap();
    let aid = id(&compilation, "a.work");
    let bid = id(&compilation, "b.work");
    assert_ne!(aid, bid);
    let ac = compilation.contract(aid).unwrap();
    let bc = compilation.contract(bid).unwrap();
    assert_eq!(ac.declaration_span, bc.declaration_span);
    assert_ne!(ac.source, bc.source);
    assert_eq!(
        ac.denied,
        tempra_effects::CapabilitySet::singleton(Capability::Alloc)
    );
    assert_eq!(
        bc.denied,
        tempra_effects::CapabilitySet::singleton(Capability::Panic)
    );
}

#[test]
fn emitted_callback_specialization_inherits_its_module_contract() {
    let dir = tempfile::tempdir().unwrap();
    // The entry module owns the callback: this uses the existing supported
    // specialization path rather than extending generic call lowering.
    let a = r#"import b;
@deny(alloc)
pub fn work(type T, first: T, second: T) -> T { return first }
pub fn main() -> Int64 {
    let numbers: Array(Int) = [1, 2]
    let n = numbers.reduce(work)
    return 42
}
"#;
    // Align the function span with A while retaining no annotation on B.work.
    let start = a.find("fn work").unwrap();
    let b = format!("{}fn work() -> Int64 {{ return 0 }}", " ".repeat(start));
    std::fs::write(dir.path().join("b.tmpr"), b).unwrap();
    let path = dir.path().join("a.tmpr");
    std::fs::write(&path, a).unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&path).unwrap();
    let specializations: Vec<_> = compilation
        .tir()
        .functions
        .iter()
        .filter(|function| function.name.starts_with("a.work$"))
        .collect();
    assert_eq!(specializations.len(), 1);
    let first = compilation.contract(specializations[0].id).unwrap();
    assert_eq!(
        first.denied,
        tempra_effects::CapabilitySet::singleton(Capability::Alloc)
    );
    assert_eq!(first.declaration_span.start as usize, start);
    assert!(compilation.contract(id(&compilation, "b.work")).is_none());
    assert!(!compilation
        .tir()
        .functions
        .iter()
        .any(|function| function.name == "a.work"));
}

#[test]
fn conflicting_bindings_never_silently_overwrite_a_contract() {
    for source in [
        "@deny(alloc) fn f() {}\nfn f() {}",
        "fn f() {}\n@deny(alloc) fn f() {}",
        "@deny(alloc) fn f() {}\n@deny(alloc) fn f() {}",
    ] {
        let Err(DriverError::ContractBindingError(error)) = compile(source) else {
            panic!("expected binding failure for {source}")
        };
        assert_ne!(error.previous_span, error.incoming_span);
        assert_eq!(error.previous_source, ContractSource::InMemory);
        assert_eq!(error.incoming_source, ContractSource::InMemory);
    }
    assert!(compile("fn f() {}\nfn f() {}").is_ok());
}

#[test]
fn specialization_requests_do_not_leak_into_another_modules_template() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("b.tmpr"),
        "@deny(panic) pub fn work(type T, a: T, b: T) -> T { return a }",
    )
    .unwrap();
    let entry = dir.path().join("a.tmpr");
    std::fs::write(
        &entry,
        r#"import b;
@deny(alloc) pub fn work(type T, a: T, b: T) -> T { return a }
pub fn main() -> Int64 {
    let xs: Array(Int) = [1, 2]
    let result = xs.reduce(work)
    return 42
}
"#,
    )
    .unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&entry).unwrap();
    assert!(compilation
        .tir()
        .functions
        .iter()
        .any(|function| function.name.starts_with("a.work$")));
    assert!(!compilation
        .tir()
        .functions
        .iter()
        .any(|function| function.name.starts_with("b.work")));
    assert_eq!(compilation.contracts().count(), 1);
}

#[test]
fn tir_consumers_and_hir_check_accept_unenforced_contracts() {
    let source = "@deny(panic) fn main() -> Int64 { return 84 / 2 }";
    tempra_driver::check_source(source).unwrap();
    tempra_driver::emit_tir_source(source).unwrap();
    assert_eq!(
        tempra_driver::pipeline::interpret_tir_source(source, "main", vec![]).unwrap(),
        tempra_tir_interp::TirRuntimeValue::Int64(42)
    );
    assert_eq!(
        tempra_driver::run_source_hir(source, &[]).unwrap(),
        tempra_interpreter::value::Value::Int64(42)
    );
}
