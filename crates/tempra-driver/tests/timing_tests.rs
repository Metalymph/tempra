use tempra_driver::{compile_to_bytecode_with_analysis_timed, CompilationPhase};

#[test]
fn successful_compilation_records_phases_in_pipeline_order() {
    let directory = tempfile::tempdir().unwrap();
    let source = "fn main() -> Int64 { return 42 }";
    let (compilation, report) =
        compile_to_bytecode_with_analysis_timed(source, &directory.path().join("measured.tbc"));
    assert!(compilation.is_ok(), "compilation: {compilation:?}");
    let phases: Vec<_> = report.samples.iter().map(|sample| sample.phase).collect();
    assert_eq!(
        phases,
        vec![
            CompilationPhase::Parse,
            CompilationPhase::HirLower,
            CompilationPhase::Typecheck,
            CompilationPhase::TirLower,
            CompilationPhase::TirVerify,
            CompilationPhase::EffectsAnalysis,
            CompilationPhase::DenyEnforcement,
            CompilationPhase::BytecodeLower,
            CompilationPhase::BytecodeVerify,
            CompilationPhase::CacheWrite,
        ]
    );
    assert!(report.samples.iter().all(|sample| sample.success));
    assert!(report.wall_ns > 0);
}

#[test]
fn parse_failure_records_only_attempted_phase() {
    let directory = tempfile::tempdir().unwrap();
    let (compilation, report) = compile_to_bytecode_with_analysis_timed(
        "fn main( -> Int64 { return 42 }",
        &directory.path().join("invalid.tbc"),
    );
    assert!(compilation.is_err());
    assert_eq!(report.samples.len(), 1);
    assert_eq!(report.samples[0].phase, CompilationPhase::Parse);
    assert!(!report.samples[0].success);
}

#[test]
fn already_present_cache_artifact_does_not_skip_fresh_compilation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("repeated.tbc");
    let source = "fn main() -> Int64 { return 42 }";
    let (first, first_report) = compile_to_bytecode_with_analysis_timed(source, &path);
    let (second, second_report) = compile_to_bytecode_with_analysis_timed(source, &path);
    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(first_report.samples.len(), second_report.samples.len());
    assert_eq!(first_report.samples[0].phase, CompilationPhase::Parse);
    assert_eq!(second_report.samples[0].phase, CompilationPhase::Parse);
}

#[test]
fn cached_hit_skips_all_compiler_phases() {
    use tempra_driver::load_or_compile_cached_source_timed;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("cached.tbc");
    let source = "fn main() -> Int64 { return 42 }";

    let (first, miss) = load_or_compile_cached_source_timed(source, &path);
    assert!(first.is_ok(), "first compile: {first:?}");
    assert_eq!(miss.cache_hit, Some(false));
    assert_eq!(miss.samples[0].phase, CompilationPhase::CacheLookup);
    assert!(miss
        .samples
        .iter()
        .any(|s| s.phase == CompilationPhase::Typecheck));
    let bytes = std::fs::read(&path).unwrap();

    let (second, hit) = load_or_compile_cached_source_timed(source, &path);
    assert!(second.is_ok(), "cache reuse: {second:?}");
    assert_eq!(hit.cache_hit, Some(true));
    assert_eq!(hit.samples.len(), 1);
    assert_eq!(hit.samples[0].phase, CompilationPhase::CacheLookup);
    assert!(hit.samples[0].success);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn corrupted_cache_falls_back_to_one_fresh_compilation() {
    use tempra_driver::load_or_compile_cached_source_timed;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.tbc");
    std::fs::write(&path, b"invalid artifact").unwrap();
    let (result, report) =
        load_or_compile_cached_source_timed("fn main() -> Int64 { return 42 }", &path);
    assert!(result.is_ok());
    assert_eq!(report.cache_hit, Some(false));
    assert_eq!(report.samples[0].phase, CompilationPhase::CacheLookup);
    assert_eq!(
        report
            .samples
            .iter()
            .filter(|s| s.phase == CompilationPhase::Typecheck)
            .count(),
        1
    );
    assert_ne!(std::fs::read(path).unwrap(), b"invalid artifact");
}

#[test]
fn multi_module_timing_uses_existing_graph_and_typechecker() {
    use tempra_driver::{compile_file_to_bytecode_with_analysis_timed, CompilationConfig};
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("math.tmpr"),
        "pub fn doubled(x: Int64) -> Int64 { return x * 2 }",
    )
    .unwrap();
    let entry = directory.path().join("main.tmpr");
    std::fs::write(
        &entry,
        "import math; pub fn main() -> Int64 { return math.doubled(21) }",
    )
    .unwrap();
    let (result, report) =
        compile_file_to_bytecode_with_analysis_timed(&entry, &CompilationConfig::default());
    assert!(result.is_ok(), "module compilation: {result:?}");
    let phases: Vec<_> = report.samples.iter().map(|s| s.phase).collect();
    assert_eq!(
        phases,
        vec![
            CompilationPhase::ModuleGraph,
            CompilationPhase::ModuleTypecheck,
            CompilationPhase::TirLower,
            CompilationPhase::TirVerify,
            CompilationPhase::EffectsAnalysis,
            CompilationPhase::DenyEnforcement,
            CompilationPhase::BytecodeLower,
            CompilationPhase::BytecodeVerify,
        ]
    );
    assert_eq!(report.cache_hit, None);
    assert!(report.samples.iter().all(|s| s.success));
}

#[test]
fn failed_module_load_reports_only_attempted_stage() {
    use tempra_driver::{compile_file_to_bytecode_with_analysis_timed, CompilationConfig};
    let directory = tempfile::tempdir().unwrap();
    let entry = directory.path().join("main.tmpr");
    std::fs::write(
        &entry,
        "import unavailable; pub fn main() -> Int64 { return 42 }",
    )
    .unwrap();
    let (result, report) =
        compile_file_to_bytecode_with_analysis_timed(&entry, &CompilationConfig::default());
    assert!(result.is_err());
    assert_eq!(report.samples.len(), 1);
    assert_eq!(report.samples[0].phase, CompilationPhase::ModuleGraph);
    assert!(!report.samples[0].success);
}
