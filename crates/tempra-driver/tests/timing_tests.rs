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
