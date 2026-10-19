use std::fs;
use std::process::Command;
use tempra_driver::compile_to_bytecode_with_analysis;
use tempra_vm::VmValue;

const CHILD_ENV: &str = "TEMPRA_K2_SPECIALIZATION_CHILD";
const CHILD_ARTIFACT_ENV: &str = "TEMPRA_K2_SPECIALIZATION_ARTIFACT";
const CHILD_RESULT_ENV: &str = "TEMPRA_K2_SPECIALIZATION_RESULT";

const SPECIALIZATION_SOURCE: &str = r#"
fn choose(type T, first: T, second: T) -> T { return first }
fn choose_second(type T, first: T, second: T) -> T { return second }
fn choose_again(type T, first: T, second: T) -> T { return first }

fn main() -> Int64 {
    let values: Array(Int) = [41, 0]
    let chosen = values.reduce(choose)
    let ignored_second = values.reduce(choose_second)
    let ignored_again = values.reduce(choose_again)
    if chosen is Some(value) {
        return value
    }
    return 99
}
"#;

fn compile_and_run(cache_path: &std::path::Path) -> (VmValue, Vec<u8>, Vec<(usize, String)>) {
    let compilation = compile_to_bytecode_with_analysis(SPECIALIZATION_SOURCE, cache_path).unwrap();
    let value = tempra_vm::run_function(compilation.bytecode(), "main", vec![]).unwrap();
    let bytes = fs::read(cache_path).unwrap();
    let functions = compilation
        .tir()
        .functions
        .iter()
        .map(|function| (function.id.0, function.name.clone()))
        .collect();
    (value, bytes, functions)
}

#[test]
fn repeated_fresh_compilation_has_one_result_order_and_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let mut expected_bytes = None;
    let mut expected_functions = None;

    for run in 0..16 {
        let (value, bytes, functions) = compile_and_run(&dir.path().join(format!("run-{run}.tbc")));
        assert_eq!(value, VmValue::Int64(41));
        assert_eq!(expected_bytes.get_or_insert_with(|| bytes.clone()), &bytes);
        assert_eq!(
            expected_functions.get_or_insert_with(|| functions.clone()),
            &functions
        );
    }
}

#[test]
fn specialization_subprocess_child() {
    if std::env::var_os(CHILD_ENV).is_none() {
        return;
    }

    let artifact = std::env::var_os(CHILD_ARTIFACT_ENV).unwrap();
    let result = std::env::var_os(CHILD_RESULT_ENV).unwrap();
    let (value, _, _) = compile_and_run(std::path::Path::new(&artifact));
    fs::write(result, format!("{value:?}")).unwrap();
}

#[test]
fn independent_processes_have_one_result_and_artifact() {
    let dir = tempfile::tempdir().unwrap();
    let current_test = std::env::current_exe().unwrap();
    let mut expected_bytes = None;
    let runs = std::env::var("TEMPRA_K2_SUBPROCESS_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(16);

    for run in 0..runs {
        let artifact = dir.path().join(format!("child-{run}.tbc"));
        let result = dir.path().join(format!("child-{run}.result"));
        let output = Command::new(&current_test)
            .arg("--exact")
            .arg("specialization_subprocess_child")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .env(CHILD_ARTIFACT_ENV, &artifact)
            .env(CHILD_RESULT_ENV, &result)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "child {run} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(&result).unwrap(), "Int64(41)");
        let bytes = fs::read(&artifact).unwrap();
        assert_eq!(expected_bytes.get_or_insert_with(|| bytes.clone()), &bytes);
    }
}
