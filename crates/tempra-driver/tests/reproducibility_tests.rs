use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempra_driver::{
    check_source, compile_file_to_bytecode_with_analysis_with_config,
    compile_to_bytecode_with_analysis, CompilationConfig,
};
use tempra_vm::VmValue;

const CHILD_MARKER: &str = "TEMPRA_K2_REPRODUCIBILITY_CHILD";
const CHILD_ROOT: &str = "TEMPRA_K2_REPRODUCIBILITY_ROOT";

const SIMPLE_SOURCE: &str = "fn main() -> Int64 { return 41 }";

const MULTI_FUNCTION_SOURCE: &str = r#"
fn forty() -> Int64 { return 40 }
fn one() -> Int64 { return 1 }
fn main() -> Int64 { return forty() + one() }
"#;

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

const COMPTIME_SOURCE: &str = r#"
fn main() -> Int64 {
    let forty: Int = comptime 20 + 20
    let one: Int = comptime 1
    return forty + one
}
"#;

const EFFECT_SOURCE: &str = r#"
@deny(process)
fn answer() -> Int64 {
    let values: Array(Int) = [40, 1]
    return values[0] + values[1]
}
fn main() -> Int64 { return answer() }
"#;

const INTERFACE_SOURCE: &str = r#"
interface A {
    type Item
    fn a(self) -> Item
}
interface B {
    type Item
    fn b(self) -> Item
}
struct Both {}
extend Both {
    fn a(self) -> Int { 41 }
    fn b(self) -> String { "forty-one" }
}
fn select(type T: A, value: T) -> T.Item { value.a() }
fn main() { let selected: Int = select[Both](Both {}) }
"#;

const DIAGNOSTIC_SOURCE: &str = r#"
interface Example {
    fn first(self) -> Int
    fn second(self) -> Int
    fn third(self) -> Int
}
struct Partial {}
extend Partial {
    fn second(self) -> Int { 2 }
}
fn accept(type T: Example, value: T) {}
fn main() { accept[Partial](Partial {}) }
"#;

const BYTECODE_FIXTURES: &[&str] = &[
    "simple",
    "multi-function",
    "multi-module",
    "specialization",
    "comptime",
    "effects",
    "stdlib",
];

#[derive(Debug, PartialEq, Eq)]
struct MatrixObservation {
    artifacts: BTreeMap<String, Vec<u8>>,
    results: String,
    diagnostics: String,
}

const EXPECTED_RESULTS: &str = "simple=Int64(41)\
\nmulti-function=Int64(41)\
\nspecialization=Int64(41)\
\ncomptime=Int64(41)\
\neffects=Int64(41)\
\nmulti-module=Int64(41)\
\nstdlib=Int64(41)\
\ninterface=accepted";

fn repository_stdlib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std")
}

fn find_main(module: &tempra_bytecode::BytecodeModule) -> String {
    module
        .functions
        .iter()
        .find(|function| function.name == "main" || function.name.ends_with(".main"))
        .expect("fixture bytecode must contain main")
        .name
        .clone()
}

fn run_module(module: &tempra_bytecode::BytecodeModule) -> VmValue {
    let main = find_main(module);
    tempra_vm::run_function_with_resolver(
        module,
        &main,
        vec![],
        tempra_vm::profile::VmOptions::default(),
        Some(tempra_native::registry::resolve),
    )
    .expect("fixture bytecode must execute")
}

fn compile_source_fixture(source: &str, artifact: &Path) -> VmValue {
    let compilation = compile_to_bytecode_with_analysis(source, artifact)
        .expect("source fixture must compile to bytecode");
    run_module(compilation.bytecode())
}

fn compile_file_fixture(entry: &Path, config: &CompilationConfig, artifact: &Path) -> VmValue {
    let compilation = compile_file_to_bytecode_with_analysis_with_config(entry, config)
        .expect("file fixture must compile to bytecode");
    let bytes = tempra_bytecode::serialize::encode_module(compilation.bytecode())
        .expect("fixture bytecode must serialize");
    fs::write(artifact, bytes).expect("fixture artifact must be writable");
    run_module(compilation.bytecode())
}

fn write_multi_module_fixture(root: &Path) -> PathBuf {
    fs::write(
        root.join("helper.tmpr"),
        "pub fn answer() -> Int64 { return 41 }",
    )
    .unwrap();
    let entry = root.join("main.tmpr");
    fs::write(
        &entry,
        r#"
import helper;
pub fn main() -> Int64 { return helper.answer() }
"#,
    )
    .unwrap();
    entry
}

fn write_stdlib_fixture(root: &Path) -> (PathBuf, PathBuf) {
    let stdlib = root.join("toolchain-std");
    fs::create_dir_all(&stdlib).unwrap();
    fs::copy(
        repository_stdlib().join("path.tmpr"),
        stdlib.join("path.tmpr"),
    )
    .unwrap();

    let entry = root.join("stdlib-main.tmpr");
    fs::write(
        &entry,
        r#"
import std.path;
pub fn main() -> Int64 {
    let joined = std.path.join("a", "b");
    if std.path.basename(joined) == "b" {
        return 41
    }
    return 0
}
"#,
    )
    .unwrap();
    (entry, stdlib)
}

fn produce_matrix(root: &Path) {
    fs::create_dir_all(root).unwrap();
    let artifacts = root.join("artifacts");
    fs::create_dir_all(&artifacts).unwrap();
    let mut results = Vec::new();

    for (name, source) in [
        ("simple", SIMPLE_SOURCE),
        ("multi-function", MULTI_FUNCTION_SOURCE),
        ("specialization", SPECIALIZATION_SOURCE),
        ("comptime", COMPTIME_SOURCE),
        ("effects", EFFECT_SOURCE),
    ] {
        let value = compile_source_fixture(source, &artifacts.join(format!("{name}.tbc")));
        results.push(format!("{name}={value:?}"));
    }

    let multi_entry = write_multi_module_fixture(root);
    let value = compile_file_fixture(
        &multi_entry,
        &CompilationConfig::without_stdlib(),
        &artifacts.join("multi-module.tbc"),
    );
    results.push(format!("multi-module={value:?}"));

    let (stdlib_entry, stdlib_root) = write_stdlib_fixture(root);
    let value = compile_file_fixture(
        &stdlib_entry,
        &CompilationConfig::with_stdlib_root(stdlib_root).unwrap(),
        &artifacts.join("stdlib.tbc"),
    );
    results.push(format!("stdlib={value:?}"));

    check_source(INTERFACE_SOURCE).expect("canonical interface fixture must typecheck");
    results.push("interface=accepted".to_string());
    fs::write(root.join("results.txt"), results.join("\n")).unwrap();

    let diagnostics = check_source(DIAGNOSTIC_SOURCE)
        .expect_err("diagnostic fixture must fail")
        .to_string();
    fs::write(root.join("diagnostics.txt"), diagnostics).unwrap();
}

fn read_matrix(root: &Path) -> MatrixObservation {
    let artifacts = BYTECODE_FIXTURES
        .iter()
        .map(|name| {
            (
                (*name).to_string(),
                fs::read(root.join("artifacts").join(format!("{name}.tbc"))).unwrap(),
            )
        })
        .collect();
    MatrixObservation {
        artifacts,
        results: fs::read_to_string(root.join("results.txt")).unwrap(),
        diagnostics: fs::read_to_string(root.join("diagnostics.txt")).unwrap(),
    }
}

fn assert_expected_semantics(observation: &MatrixObservation) {
    assert_eq!(observation.results, EXPECTED_RESULTS);
    for required in ["missing requirement 'first'", "missing requirement 'third'"] {
        assert!(
            observation.diagnostics.contains(required),
            "missing deterministic diagnostic {required:?}:\n{}",
            observation.diagnostics
        );
    }
    let first = observation
        .diagnostics
        .find("missing requirement 'first'")
        .unwrap();
    let third = observation
        .diagnostics
        .find("missing requirement 'third'")
        .unwrap();
    assert!(first < third, "requirements must follow source order");
}

fn spawn_matrix(root: &Path, cwd: &Path, run: usize) -> MatrixObservation {
    let output = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("reproducibility_subprocess_entrypoint")
        .arg("--nocapture")
        .current_dir(cwd)
        .env(CHILD_MARKER, "1")
        .env(CHILD_ROOT, root)
        .env("TEMPRA_K2_INCIDENTAL_RUN", run.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child {run} failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    read_matrix(root)
}

#[test]
fn reproducibility_subprocess_entrypoint() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }
    produce_matrix(Path::new(&std::env::var_os(CHILD_ROOT).unwrap()));
}

#[test]
fn repeated_fresh_compilation_has_one_matrix() {
    let fixture = tempfile::tempdir().unwrap();
    let mut expected = None;

    for run in 0..8 {
        let root = fixture.path().join(format!("same-process-{run}"));
        produce_matrix(&root);
        let observation = read_matrix(&root);
        assert_expected_semantics(&observation);
        assert_eq!(expected.get_or_insert(observation), &read_matrix(&root));
    }
}

#[test]
fn independent_processes_have_one_supported_bytecode_and_diagnostic_matrix() {
    let fixture = tempfile::tempdir().unwrap();
    let cwd_a = tempfile::tempdir().unwrap();
    let cwd_b = tempfile::tempdir().unwrap();
    fs::create_dir_all(cwd_b.path().join("nested/work")).unwrap();
    let cwd_b = cwd_b.path().join("nested/work");
    let runs = std::env::var("TEMPRA_K2_REPRO_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(16);
    let mut expected = None;

    for run in 0..runs {
        let root = fixture.path().join(format!("process-{run}"));
        let cwd = if run % 2 == 0 { cwd_a.path() } else { &cwd_b };
        let observation = spawn_matrix(&root, cwd, run);
        assert_expected_semantics(&observation);
        assert_eq!(expected.get_or_insert(observation), &read_matrix(&root));
    }
}

#[test]
fn equivalent_absolute_roots_and_file_metadata_produce_identical_artifacts() {
    let fixture = tempfile::tempdir().unwrap();
    let root_a = fixture.path().join("absolute-a");
    let root_b = fixture.path().join("absolute-b");
    produce_matrix(&root_a);

    std::thread::sleep(std::time::Duration::from_millis(20));
    produce_matrix(&root_b);
    let before = read_matrix(&root_b);
    assert_expected_semantics(&before);

    std::thread::sleep(std::time::Duration::from_millis(20));
    let entry = root_b.join("main.tmpr");
    let source = fs::read_to_string(&entry).unwrap();
    fs::write(&entry, source).unwrap();
    produce_matrix(&root_b);

    assert_eq!(read_matrix(&root_a), before);
    assert_eq!(read_matrix(&root_a), read_matrix(&root_b));
}
