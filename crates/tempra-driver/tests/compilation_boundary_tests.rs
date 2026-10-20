use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use tempra_driver::{
    compile_file_to_bytecode_with_analysis, compile_to_bytecode_with_analysis,
    inspect_effects_file, run_file, run_file_with_experimental_vm_jit, run_source, run_source_hir,
    run_source_vm, run_source_vm_profiled, DriverError, EffectQueryError,
    SpecializedContractViolation, SpecializedMatch, VmCompilation,
};
use tempra_effects::{
    CapabilityAnalysisError, EffectOperation, LocalEffectFact, SpecializedOperationFact,
};
use tempra_vm::profile::VmOptions;
use tempra_vm::VmValue;

const SEMANTICS_SIX_FINGERPRINT: &str =
    concat!("tempra-", env!("CARGO_PKG_VERSION"), "-semantics-6");

struct CacheGuard {
    path: PathBuf,
    previous: Option<Vec<u8>>,
}

impl CacheGuard {
    fn for_source(source: &str) -> Self {
        let path = tempra_driver::cache::get_cache_path(
            &tempra_driver::cache::compute_source_hash(source),
        );
        Self {
            previous: fs::read(&path).ok(),
            path,
        }
    }
}

impl Drop for CacheGuard {
    fn drop(&mut self) {
        if let Some(bytes) = &self.previous {
            let _ = fs::write(&self.path, bytes);
        } else {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn compile_source(source: &str) -> Result<VmCompilation, DriverError> {
    let dir = tempfile::tempdir().unwrap();
    compile_to_bytecode_with_analysis(source, &dir.path().join("fresh.tbc"))
}

fn expect_specialized_violations(error: DriverError) -> Vec<SpecializedContractViolation> {
    match error {
        DriverError::SpecializedContractViolations(violations) => violations,
        other => panic!("expected specialized contract violations, got {other:?}"),
    }
}

fn specialized_fact(violation: &SpecializedContractViolation) -> &SpecializedOperationFact {
    match &violation.fact {
        LocalEffectFact::Specialized(fact) => fact,
        other => panic!("expected a specialized fact, got {other:?}"),
    }
}

fn expect_unclassified_extern(error: DriverError) {
    match error {
        DriverError::CapabilityAnalysisError(CapabilityAnalysisError::UnclassifiedExtern {
            name,
            ..
        }) => assert!(
            name.ends_with("k46b2_unclassified_native"),
            "unexpected extern name: {name}"
        ),
        other => panic!("expected an unclassified extern error, got {other:?}"),
    }
}

fn write_source(denied: &Path, written: &Path) -> String {
    format!(
        r#"
import std.fs;

@deny(fs.write(destination: "{denied}"))
pub fn main() -> Int64 {{
    let result = std.fs.write_text("{written}", "data");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}
"#,
        denied = denied.display(),
        written = written.display(),
    )
}

#[test]
fn fresh_source_compilation_preserves_broad_pure_and_specialized_declarations() {
    let dir = tempfile::tempdir().unwrap();
    let broad_path = dir.path().join("broad.tbc");
    let broad = r#"
// k4.6b.2 fresh source broad contract
@deny(panic)
pub fn main() -> Int64 { return 10 / 2 }
"#;
    assert!(matches!(
        compile_to_bytecode_with_analysis(broad, &broad_path).unwrap_err(),
        DriverError::CapabilityContractViolations(ref violations) if violations.len() == 1
    ));
    assert!(!broad_path.exists());

    let pure = r#"
// k4.6b.2 fresh source pure contract
@pure
pub fn main() -> Int64 { return 10 / 2 }
"#;
    let pure_compilation = compile_source(pure).unwrap();
    let (_, pure_contract) = pure_compilation.contracts().next().unwrap();
    assert!(pure_contract.pure_span.is_some());

    let specialized = r#"
// k4.6b.2 fresh source valid specialized contract
@deny(fs.write(destination: "source-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    let compilation = compile_source(specialized).unwrap();
    let (_, contract) = compilation.contracts().next().unwrap();
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
}

#[test]
fn module_graph_compilation_preserves_specialized_contract_bindings() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("helper.tmpr"),
        r#"
import std.fs;

pub fn write_blocked() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result { Ok(_) => 0 Err(_) => 1 };
}
"#,
    )
    .unwrap();

    let violating_path = dir.path().join("main.tmpr");
    fs::write(
        &violating_path,
        r#"
import helper;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 { return helper.write_blocked() }
"#,
    )
    .unwrap();
    let violations = expect_specialized_violations(
        compile_file_to_bytecode_with_analysis(&violating_path).unwrap_err(),
    );
    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0].classification,
        SpecializedMatch::DefiniteMatch
    );
    assert_eq!(violations[0].witness.calls.len(), 1);
    assert_eq!(
        specialized_fact(&violations[0]).extern_name,
        "std.fs.write_text"
    );

    let valid_path = dir.path().join("valid.tmpr");
    fs::write(
        &valid_path,
        r#"
import helper;

@deny(fs.write(destination: "allowed.txt"))
pub fn main() -> Int64 { return helper.write_blocked() }
"#,
    )
    .unwrap();
    let compilation = compile_file_to_bytecode_with_analysis(&valid_path).unwrap();
    let main = compilation
        .tir()
        .functions
        .iter()
        .find(|function| function.name == "valid.main")
        .unwrap();
    let contract = compilation.contract(main.id).unwrap();
    assert_eq!(contract.specialized_denied.len(), 1);
    assert_eq!(
        contract.specialized_denied[0].operation,
        EffectOperation::FsWrite
    );
}

#[test]
fn vm_and_profiling_never_execute_contract_violations() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("vm-blocked.txt");
    let violating_path = dir.path().join("violating.tmpr");
    fs::write(&violating_path, write_source(&blocked, &blocked)).unwrap();

    let error = run_file_with_experimental_vm_jit(&violating_path, &[], false).unwrap_err();
    assert!(matches!(
        error,
        DriverError::SpecializedContractViolations(ref violations) if violations.len() == 1
    ));
    assert!(
        !blocked.exists(),
        "a violating program must fail before native execution"
    );

    let allowed = dir.path().join("vm-allowed.txt");
    let valid_path = dir.path().join("valid.tmpr");
    fs::write(&valid_path, write_source(&blocked, &allowed)).unwrap();
    let value = run_file_with_experimental_vm_jit(&valid_path, &[], false).unwrap();
    assert_eq!(value, VmValue::Int64(0));
    assert!(
        allowed.exists(),
        "a valid specialized contract must execute normally"
    );

    // The source-only VM entry point cannot resolve module imports, so its
    // contract parity is checked with source-local facts (division/panic).
    let profiled_violating = r#"
// k4.6b.2 ordinary and profiled VM contract parity
@deny(panic)
pub fn main() -> Int64 { return 10 / 2 }
"#;
    let _guard = CacheGuard::for_source(profiled_violating);
    for error in [
        run_source_vm(profiled_violating, &[]).unwrap_err(),
        run_source_vm_profiled(profiled_violating, VmOptions::default()).unwrap_err(),
    ] {
        assert!(matches!(
            error,
            DriverError::CapabilityContractViolations(ref violations) if violations.len() == 1
        ));
    }

    let profiled_valid = r#"
// k4.6b.2 profiled valid specialized contract
@deny(fs.write(destination: "profiled-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    let _guard = CacheGuard::for_source(profiled_valid);
    let mut options = VmOptions::default();
    options.profiling = true;
    let (value, profile) = run_source_vm_profiled(profiled_valid, options).unwrap();
    assert_eq!(value, VmValue::Int64(42));
    assert!(profile
        .function_stats
        .iter()
        .any(|stats| stats.function_name == "main"));
}

#[test]
fn jit_options_do_not_bypass_specialized_compile_enforcement() {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("jit-blocked.txt");
    let violating_path = dir.path().join("jit-violating.tmpr");
    fs::write(&violating_path, write_source(&blocked, &blocked)).unwrap();

    let error = run_file_with_experimental_vm_jit(&violating_path, &[], true).unwrap_err();
    assert!(matches!(
        error,
        DriverError::SpecializedContractViolations(ref violations) if violations.len() == 1
    ));
    assert!(!blocked.exists());

    let allowed = dir.path().join("jit-allowed.txt");
    let valid_path = dir.path().join("jit-valid.tmpr");
    fs::write(&valid_path, write_source(&blocked, &allowed)).unwrap();
    let value = run_file_with_experimental_vm_jit(&valid_path, &[], true).unwrap();
    assert_eq!(value, VmValue::Int64(0));
    assert!(allowed.exists());
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_compiled_programs_retain_specialized_contracts() {
    let source = r#"
import std.fs;

// k4.6b.2 actual jit valid specialized contract
fn add(a: Int64, b: Int64) -> Int64 { return a + b }

@deny(fs.write(destination: "jit-never.txt"))
pub fn main() -> Int64 {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    var sum = 0
    for i in xs {
        for j in xs {
            sum = add(sum, j)
        }
    }
    return sum - 550
}
"#;
    let _guard = CacheGuard::for_source(source);
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 10;
    options.hot_backedge_threshold = 10;

    let (value, profile) = run_source_vm_profiled(source, options).unwrap();
    assert_eq!(value, VmValue::Int64(0));
    assert!(
        profile
            .function_stats
            .iter()
            .any(|stats| stats.jit_compile_successes > 0),
        "the JIT-enabled path must actually compile at least one function"
    );
}

#[test]
fn cache_miss_enforces_and_compatible_hit_is_reused() {
    let violating = r#"
// k4.6b.2 cache miss contract enforcement
@deny(fs.write(destination: "cache-miss-never.txt"), panic)
pub fn main() -> Int64 { return 10 / 2 }
"#;
    let _guard = CacheGuard::for_source(violating);
    let violating_path =
        tempra_driver::cache::get_cache_path(&tempra_driver::cache::compute_source_hash(violating));

    let error = run_source_vm(violating, &[]).unwrap_err();
    assert!(matches!(
        error,
        DriverError::CapabilityContractViolations(ref violations) if violations.len() == 1
    ));
    assert!(
        !violating_path.exists(),
        "a failed cache miss must not write an artifact"
    );

    let valid = r#"
import std.fs;

// k4.6b.2 compatible specialized cache hit
@deny(fs.write(destination: "cache-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    let _guard = CacheGuard::for_source(valid);
    let valid_path =
        tempra_driver::cache::get_cache_path(&tempra_driver::cache::compute_source_hash(valid));
    assert_eq!(run_source_vm(valid, &[]).unwrap(), VmValue::Int64(42));
    let artifact = tempra_driver::cache::read_cache_artifact(&valid_path).unwrap();
    assert_eq!(
        artifact.header.compiler_fingerprint,
        SEMANTICS_SIX_FINGERPRINT
    );
    let bytes = fs::read(&valid_path).unwrap();
    let modified = fs::metadata(&valid_path).unwrap().modified().unwrap();

    thread::sleep(Duration::from_millis(50));
    assert_eq!(run_source_vm(valid, &[]).unwrap(), VmValue::Int64(42));
    assert_eq!(fs::read(&valid_path).unwrap(), bytes);
    assert_eq!(
        fs::metadata(&valid_path).unwrap().modified().unwrap(),
        modified,
        "a compatible hit must reuse verified bytecode without rewriting it"
    );
}

#[test]
fn stale_semantic_revision_is_rejected_before_execution() {
    let source = r#"
// k4.6b.2 stale semantic revision enforcement
@deny(fs.write(destination: "stale-never.txt"), panic)
pub fn main() -> Int64 { return 10 / 2 }
"#;
    let _guard = CacheGuard::for_source(source);
    let cache_path =
        tempra_driver::cache::get_cache_path(&tempra_driver::cache::compute_source_hash(source));
    let source_hash = tempra_driver::cache::compute_source_hash(source);

    let seed_dir = tempfile::tempdir().unwrap();
    let seed = compile_to_bytecode_with_analysis(
        "fn main() -> Int64 { return 7 }",
        &seed_dir.path().join("seed.tbc"),
    )
    .unwrap();
    let stale = tempra_bytecode::BytecodeCacheArtifact {
        header: tempra_bytecode::BytecodeCacheHeader {
            compiler_fingerprint: format!("tempra-{}-semantics-5", env!("CARGO_PKG_VERSION")),
            source_hash,
            ..Default::default()
        },
        module: seed.bytecode().clone(),
    };
    tempra_driver::cache::write_cache_artifact(&cache_path, &stale).unwrap();
    let stale_bytes = fs::read(&cache_path).unwrap();

    let error = run_source_vm(source, &[]).unwrap_err();
    assert!(matches!(
        error,
        DriverError::CapabilityContractViolations(ref violations) if violations.len() == 1
    ));
    assert_eq!(fs::read(&cache_path).unwrap(), stale_bytes);
    assert_eq!(
        tempra_driver::cache::read_cache_artifact(&cache_path)
            .unwrap()
            .header
            .compiler_fingerprint,
        format!("tempra-{}-semantics-5", env!("CARGO_PKG_VERSION")),
        "a failed fresh compilation must not promote a stale artifact"
    );
}

#[test]
fn source_changes_select_a_new_verified_cache_identity() {
    const VALID: &str = r#"
import std.fs;

// k4.6b.2 source identity specialized artifact
@deny(fs.write(destination: "identity-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    let changed = VALID.replace("return 42", "return 43");
    let valid_hash = tempra_driver::cache::compute_source_hash(VALID);
    let changed_hash = tempra_driver::cache::compute_source_hash(&changed);
    assert_ne!(changed_hash, valid_hash);
    assert_ne!(
        tempra_driver::cache::get_cache_path(&valid_hash),
        tempra_driver::cache::get_cache_path(&changed_hash),
        "source changes must not share a cache identity"
    );

    let _valid_guard = CacheGuard::for_source(VALID);
    let _changed_guard = CacheGuard::for_source(&changed);
    assert_eq!(run_source_vm(VALID, &[]).unwrap(), VmValue::Int64(42));
    assert_eq!(run_source_vm(&changed, &[]).unwrap(), VmValue::Int64(43));
    assert_eq!(
        tempra_driver::cache::read_cache_artifact(&tempra_driver::cache::get_cache_path(
            &valid_hash
        ))
        .unwrap()
        .header
        .source_hash,
        valid_hash
    );
    assert_eq!(
        tempra_driver::cache::read_cache_artifact(&tempra_driver::cache::get_cache_path(
            &changed_hash
        ))
        .unwrap()
        .header
        .source_hash,
        changed_hash
    );
}

#[test]
fn failed_compilation_does_not_replace_a_valid_specialized_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source-identity.tbc");
    let valid = r#"
import std.fs;

// k4.6b.2 failed compilation preservation
@deny(fs.write(destination: "preserve-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    compile_to_bytecode_with_analysis(valid, &path).unwrap();
    let original_bytes = fs::read(&path).unwrap();
    let original_header = tempra_driver::cache::read_cache_artifact(&path)
        .unwrap()
        .header;

    let violating = r#"
// k4.6b.2 failed compilation preservation
@deny(panic)
pub fn main() -> Int64 { return 10 / 2 }
"#;
    let error = compile_to_bytecode_with_analysis(violating, &path).unwrap_err();
    assert!(matches!(
        error,
        DriverError::CapabilityContractViolations(ref violations) if violations.len() == 1
    ));
    assert_eq!(fs::read(&path).unwrap(), original_bytes);
    let preserved_header = tempra_driver::cache::read_cache_artifact(&path)
        .unwrap()
        .header;
    assert_eq!(preserved_header.source_hash, original_header.source_hash);
    assert_eq!(
        preserved_header.compiler_fingerprint,
        original_header.compiler_fingerprint
    );
}

#[test]
fn hir_and_source_tir_entry_points_reject_specialized_annotations() {
    const SOURCE: &str = r#"
import std.fs;

// k4.6b.2 unsupported specialized annotation path
@deny(fs.write(destination: "unsupported-never.txt"))
pub fn main() -> Int64 { return 42 }
"#;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unsupported.tmpr");
    fs::write(&path, SOURCE).unwrap();

    let results = [
        run_source(SOURCE),
        run_source_hir(SOURCE, &[]).map(|_| ()),
        run_file(&path),
        tempra_driver::pipeline::test_source(SOURCE),
        tempra_driver::pipeline::interpret_tir_source(SOURCE, "main", vec![]).map(|_| ()),
    ];
    for result in results {
        match result {
            Err(DriverError::RuntimeError(message)) => assert!(
                message.contains("Capability-contract enforcement is not supported"),
                "unexpected rejection message: {message}"
            ),
            other => panic!("expected explicit annotation rejection, got {other:?}"),
        }
    }
}

#[test]
fn unknown_externs_fail_closed_in_fresh_compilation_and_inspection_paths() {
    const UNKNOWN: &str = r#"
// k4.6b.2 unknown extern fresh boundary

pub extern fn k46b2_unclassified_native() -> Int64
fn main() -> Int64 { return k46b2_unclassified_native() }
"#;
    let dir = tempfile::tempdir().unwrap();
    let source_path = dir.path().join("unknown.tmpr");
    fs::write(&source_path, UNKNOWN).unwrap();
    let fresh_path = dir.path().join("fresh.tbc");
    expect_unclassified_extern(
        compile_to_bytecode_with_analysis(UNKNOWN, &fresh_path).unwrap_err(),
    );
    assert!(!fresh_path.exists());
    expect_unclassified_extern(compile_file_to_bytecode_with_analysis(&source_path).unwrap_err());

    let _guard = CacheGuard::for_source(UNKNOWN);
    expect_unclassified_extern(run_source_vm(UNKNOWN, &[]).unwrap_err());
    expect_unclassified_extern(run_source_vm_profiled(UNKNOWN, VmOptions::default()).unwrap_err());
    expect_unclassified_extern(
        run_file_with_experimental_vm_jit(&source_path, &[], true).unwrap_err(),
    );
    match inspect_effects_file(&source_path).unwrap_err() {
        EffectQueryError::Compiler(error) => expect_unclassified_extern(error),
        other => panic!("expected inspection to fail closed, got {other:?}"),
    }
}
