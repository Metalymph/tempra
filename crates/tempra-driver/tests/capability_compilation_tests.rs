use tempra_driver::{compile_to_bytecode_with_analysis, DriverError, VmCompilation};
use tempra_effects::{Capability, CapabilityAnalysisError};
use tempra_tir::ir::TirFunctionId;
use tempra_vm::VmValue;

fn compile(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    compile_to_bytecode_with_analysis(source, &dir.path().join("fresh.tbc"))
}

fn id(compilation: &VmCompilation, name: &str) -> TirFunctionId {
    compilation
        .tir()
        .functions
        .iter()
        .find(|f| f.name == name)
        .unwrap()
        .id
}

fn execute(compilation: &VmCompilation, name: &str) -> VmValue {
    tempra_vm::run_function_with_resolver(
        compilation.bytecode(),
        name,
        vec![],
        Default::default(),
        Some(tempra_native::registry::resolve),
    )
    .unwrap()
}

#[test]
fn fresh_product_writes_the_existing_bytecode_only_cache() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fresh.tbc");
    let source = "fn main() -> Int64 { return 42 }";
    let compilation = compile_to_bytecode_with_analysis(source, &path).unwrap();
    let artifact = tempra_driver::cache::read_cache_artifact(&path).unwrap();
    assert_eq!(
        artifact.header.source_hash,
        tempra_driver::cache::compute_source_hash(source)
    );
    assert_eq!(
        tempra_bytecode::serialize::encode_module(&artifact.module).unwrap(),
        tempra_bytecode::serialize::encode_module(compilation.bytecode()).unwrap(),
    );
    let projected = compilation.into_bytecode();
    assert_eq!(
        tempra_bytecode::serialize::encode_module(&projected).unwrap(),
        tempra_bytecode::serialize::encode_module(&artifact.module).unwrap(),
    );
}

#[test]
fn retains_direct_transitive_and_non_entry_facts_without_enforcement() {
    let compilation = compile(
        r#"
fn c() -> Int64 { let text = "allocation"; return 84 / 2 }
fn b() -> Int64 { return c() }
fn a() -> Int64 { return b() }
fn detached() -> String { return "not reachable from main" }
fn main() -> Int64 { return a() }
"#,
    )
    .unwrap();
    let analysis = compilation.capabilities();
    let a = id(&compilation, "a");
    let c = id(&compilation, "c");
    assert_eq!(
        analysis.summary(a).unwrap().direct,
        tempra_effects::CapabilitySet::empty()
    );
    for capability in [Capability::Alloc, Capability::Panic] {
        assert!(analysis.summary(c).unwrap().direct.contains(capability));
        assert!(analysis.summary(a).unwrap().required.contains(capability));
        let witness = analysis.witness(a, capability).unwrap();
        assert_eq!(witness.calls.len(), 2);
        assert_eq!(witness.origin.location.function, c);
    }
    assert!(analysis
        .summary(id(&compilation, "detached"))
        .unwrap()
        .direct
        .contains(Capability::Alloc));
    assert_eq!(execute(&compilation, "main"), VmValue::Int64(42));

    let direct = tempra_bytecode_lower::lower_tir_module(compilation.tir()).unwrap();
    assert_eq!(
        tempra_bytecode::serialize::encode_module(compilation.bytecode()).unwrap(),
        tempra_bytecode::serialize::encode_module(&direct).unwrap(),
    );
    assert_eq!(
        compilation.capabilities(),
        &tempra_effects::analyze(compilation.tir()).unwrap()
    );
}

const UNKNOWN_CALLED: &str = r#"
pub extern fn e2_unclassified_native() -> Int64
fn detached() -> Int64 { return e2_unclassified_native() }
fn main() -> Int64 { return 42 }
"#;

#[test]
fn called_unknown_extern_is_typed_but_unused_extern_is_accepted() {
    match compile(UNKNOWN_CALLED).unwrap_err() {
        DriverError::CapabilityAnalysisError(CapabilityAnalysisError::UnclassifiedExtern {
            caller,
            extern_id,
            name,
            callsite,
        }) => {
            assert_eq!(name, "e2_unclassified_native");
            assert_eq!(caller, callsite.function);
            assert_ne!(caller, extern_id);
        }
        error => panic!("unexpected error: {error:?}"),
    }
    let unused = compile(
        "pub extern fn e2_unclassified_native() -> Int64\nfn main() -> Int64 { return 42 }",
    )
    .unwrap();
    assert_eq!(execute(&unused, "main"), VmValue::Int64(42));
}

#[test]
fn emit_and_interpret_tir_do_not_require_capability_coverage() {
    let text = tempra_driver::emit_tir_source(UNKNOWN_CALLED).unwrap();
    assert!(text.contains("e2_unclassified_native"));
    assert_eq!(
        tempra_driver::pipeline::interpret_tir_source(UNKNOWN_CALLED, "main", vec![]).unwrap(),
        tempra_tir_interp::TirRuntimeValue::Int64(42),
    );
}

#[test]
fn multi_module_known_extern_facts_are_retained_without_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.tmpr");
    std::fs::write(
        dir.path().join("helper.tmpr"),
        r#"
import std.fs;
pub fn probe() -> Bool { return std.fs.exists(".") }
"#,
    )
    .unwrap();
    std::fs::write(
        &main,
        r#"
import helper;
pub fn main() -> Int64 { if helper.probe() { return 84 / 2 }; return 0 }
"#,
    )
    .unwrap();
    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap();
    let main_id = id(&compilation, "main.main");
    for capability in [Capability::Alloc, Capability::Panic, Capability::Io] {
        assert!(compilation
            .capabilities()
            .summary(main_id)
            .unwrap()
            .required
            .contains(capability));
    }
    assert_eq!(execute(&compilation, "main.main"), VmValue::Int64(42));
}

#[test]
fn cache_hit_executes_verified_bytecode_without_requiring_analysis() {
    // Model a valid bytecode-only artifact produced before E2. The non-entry
    // extern is deliberately unclassified, but never executed.
    let ast = tempra_parser::parse_source(UNKNOWN_CALLED).unwrap();
    let mut hir = tempra_hir::lower_file(UNKNOWN_CALLED, &ast);
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
    let module = tempra_bytecode_lower::lower_tir_module(&builder.module).unwrap();
    let hash = tempra_driver::cache::compute_source_hash(UNKNOWN_CALLED);
    let path = tempra_driver::cache::get_cache_path(&hash);
    // Restore any pre-existing artifact rather than deleting user data.
    let previous = std::fs::read(&path).ok();
    let artifact = tempra_bytecode::BytecodeCacheArtifact {
        header: tempra_bytecode::BytecodeCacheHeader {
            compiler_fingerprint: format!("tempra-{}", env!("CARGO_PKG_VERSION")),
            source_hash: hash,
            ..Default::default()
        },
        module,
    };
    tempra_driver::cache::write_cache_artifact(&path, &artifact).unwrap();
    let result = tempra_driver::run_source_vm(UNKNOWN_CALLED, &[]);
    let profiled = tempra_driver::run_source_vm_profiled(UNKNOWN_CALLED, Default::default());
    // Analysis-producing APIs must compile fresh even when this valid artifact
    // exists, and the legacy bytecode-only wrapper is capability-aware too.
    let fresh = compile_to_bytecode_with_analysis(UNKNOWN_CALLED, &path);
    let legacy = tempra_driver::compile_to_bytecode(UNKNOWN_CALLED, &path);
    if let Some(bytes) = previous {
        std::fs::write(&path, bytes).unwrap();
    } else {
        std::fs::remove_file(&path).unwrap();
    }
    assert_eq!(result.unwrap(), VmValue::Int64(42));
    assert_eq!(profiled.unwrap().0, VmValue::Int64(42));
    assert!(matches!(
        fresh,
        Err(DriverError::CapabilityAnalysisError(_))
    ));
    assert!(matches!(
        legacy,
        Err(DriverError::CapabilityAnalysisError(_))
    ));
    assert!(matches!(
        compile(UNKNOWN_CALLED),
        Err(DriverError::CapabilityAnalysisError(_))
    ));
}
