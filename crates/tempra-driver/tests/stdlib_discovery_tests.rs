use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::tempdir;
use tempra_driver::{
    check_file_with_config, compile_file_to_bytecode_with_analysis_with_config, CompilationConfig,
    DriverError,
};
use tempra_vm::value::VmValue;

const CHILD_MARKER: &str = "TEMPRA_K2_STDLIB_CHILD";
const CHILD_ENTRY: &str = "TEMPRA_K2_STDLIB_ENTRY";
const CHILD_OUTPUT: &str = "TEMPRA_K2_STDLIB_OUTPUT";
const CHILD_ROOT: &str = "TEMPRA_K2_STDLIB_ROOT";

fn repository_stdlib() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../std")
}

fn write_path_program(path: &Path) {
    fs::write(
        path,
        r#"
import std.path;

pub fn main() -> Int64 {
    if std.path.join("a", "b") == "a/b" {
        return 41;
    }
    return 0;
}
"#,
    )
    .unwrap();
}

fn copy_std_module(root: &Path, name: &str) {
    fs::create_dir_all(root).unwrap();
    fs::copy(
        repository_stdlib().join(format!("{name}.tmpr")),
        root.join(format!("{name}.tmpr")),
    )
    .unwrap();
}

fn compile_and_run(path: &Path, config: &CompilationConfig) -> (VmValue, Vec<u8>) {
    let compilation = compile_file_to_bytecode_with_analysis_with_config(path, config).unwrap();
    let bytes = tempra_bytecode::serialize::encode_module(compilation.bytecode()).unwrap();
    let main = compilation
        .bytecode()
        .functions
        .iter()
        .find(|function| function.name == "main" || function.name.ends_with(".main"))
        .unwrap()
        .name
        .clone();
    let value = tempra_vm::run_function_with_resolver(
        compilation.bytecode(),
        &main,
        vec![],
        tempra_vm::profile::VmOptions::default(),
        Some(tempra_native::registry::resolve),
    )
    .unwrap();
    (value, bytes)
}

fn child_command(entry: &Path, output: &Path, cwd: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("subprocess_compilation_entrypoint")
        .arg("--nocapture")
        .current_dir(cwd)
        .env(CHILD_MARKER, "1")
        .env(CHILD_ENTRY, entry)
        .env(CHILD_OUTPUT, output);
    command
}

#[test]
fn subprocess_compilation_entrypoint() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }

    let entry = PathBuf::from(std::env::var_os(CHILD_ENTRY).unwrap());
    let output = PathBuf::from(std::env::var_os(CHILD_OUTPUT).unwrap());
    let config = match std::env::var_os(CHILD_ROOT) {
        Some(root) => CompilationConfig::with_stdlib_root(root).unwrap(),
        None => CompilationConfig::default(),
    };
    let (value, bytes) = compile_and_run(&entry, &config);
    assert_eq!(value, VmValue::Int64(41));
    fs::write(output, bytes).unwrap();
}

#[test]
fn default_discovery_is_independent_of_current_working_directory() {
    let fixture = tempdir().unwrap();
    let entry = fixture.path().join("main.tmpr");
    write_path_program(&entry);

    let cwd_a = tempdir().unwrap();
    let cwd_b = tempdir().unwrap();
    fs::create_dir_all(cwd_b.path().join("nested/work")).unwrap();
    let output_a = fixture.path().join("a.tbc");
    let output_b = fixture.path().join("b.tbc");

    let first = child_command(&entry, &output_a, cwd_a.path())
        .output()
        .unwrap();
    assert!(
        first.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = child_command(&entry, &output_b, &cwd_b.path().join("nested/work"))
        .output()
        .unwrap();
    assert!(
        second.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );

    assert_eq!(fs::read(output_a).unwrap(), fs::read(output_b).unwrap());
}

#[test]
fn explicit_root_wins_over_a_cwd_std_directory() {
    let fixture = tempdir().unwrap();
    let entry = fixture.path().join("main.tmpr");
    write_path_program(&entry);

    let selected = fixture.path().join("selected-std");
    copy_std_module(&selected, "path");
    let hostile_cwd = fixture.path().join("hostile");
    fs::create_dir_all(hostile_cwd.join("std")).unwrap();
    fs::write(hostile_cwd.join("std/path.tmpr"), "not valid Tempra").unwrap();
    let output = fixture.path().join("explicit.tbc");

    let child = child_command(&entry, &output, &hostile_cwd)
        .env(CHILD_ROOT, &selected)
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "child failed: {}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(!fs::read(output).unwrap().is_empty());
}

#[test]
fn missing_root_is_allowed_until_std_is_imported() {
    let fixture = tempdir().unwrap();
    let plain = fixture.path().join("plain.tmpr");
    fs::write(&plain, "pub fn main() -> Int64 { return 41 }").unwrap();

    check_file_with_config(&plain, &CompilationConfig::without_stdlib()).unwrap();

    let importing = fixture.path().join("importing.tmpr");
    write_path_program(&importing);
    let error =
        check_file_with_config(&importing, &CompilationConfig::without_stdlib()).unwrap_err();
    assert!(matches!(error, DriverError::TypeError(_)));
    assert!(error
        .to_string()
        .contains("standard library could not be located for import 'std.path'"));
}

#[test]
fn invalid_explicit_root_does_not_fall_back() {
    let fixture = tempdir().unwrap();
    let entry = fixture.path().join("main.tmpr");
    write_path_program(&entry);
    let missing = fixture.path().join("missing-std");
    let config = CompilationConfig::with_stdlib_root(&missing).unwrap();

    let error = check_file_with_config(&entry, &config).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("could not read module file 'std.path'"));
    assert!(message.contains(&missing.join("path.tmpr").display().to_string()));
}

#[test]
fn equivalent_explicit_roots_produce_identical_bytecode() {
    let fixture = tempdir().unwrap();
    let entry = fixture.path().join("main.tmpr");
    write_path_program(&entry);
    let root_a = fixture.path().join("std-a");
    let root_b = fixture.path().join("std-b");
    copy_std_module(&root_a, "path");
    copy_std_module(&root_b, "path");

    let (_, bytes_a) = compile_and_run(
        &entry,
        &CompilationConfig::with_stdlib_root(root_a).unwrap(),
    );
    let (_, bytes_b) = compile_and_run(
        &entry,
        &CompilationConfig::with_stdlib_root(root_b).unwrap(),
    );

    assert_eq!(bytes_a, bytes_b);
}

#[test]
fn one_explicit_root_is_used_across_the_module_graph() {
    let fixture = tempdir().unwrap();
    let stdlib = fixture.path().join("std");
    copy_std_module(&stdlib, "path");
    copy_std_module(&stdlib, "strings");

    fs::write(
        fixture.path().join("helper.tmpr"),
        r#"
import std.strings;

pub fn has_b(value: String) -> Bool {
    return std.strings.contains(value, "b");
}
"#,
    )
    .unwrap();
    let entry = fixture.path().join("main.tmpr");
    fs::write(
        &entry,
        r#"
import helper;
import std.path;

pub fn main() -> Int64 {
    let value = std.path.join("a", "b");
    if helper.has_b(value) {
        return 41;
    }
    return 0;
}
"#,
    )
    .unwrap();

    let (value, _) = compile_and_run(
        &entry,
        &CompilationConfig::with_stdlib_root(stdlib).unwrap(),
    );
    assert_eq!(value, VmValue::Int64(41));
}
