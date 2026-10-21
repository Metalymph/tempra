use std::fs;
use std::path::Path;
use tempfile::tempdir;
use tempra_driver::DriverError;
use tempra_vm::value::VmValue;

fn run_tempra_vm_file(entry: &Path) -> Result<VmValue, DriverError> {
    let options = tempra_vm::profile::VmOptions::default();
    tempra_driver::run_file_with_experimental_vm_jit(entry, &[], options.jit_enabled)
}

#[test]
fn test_entry_module_unqualified_function_call() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    // Forward reference: `add` is declared after `main` in source order.
    let main_src = r#"
pub fn main() -> Int64 {
    return add(10, 20)
}

fn add(a: Int64, b: Int64) -> Int64 {
    return a + b
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(30));
}

#[test]
fn test_imported_module_unqualified_function_call() {
    let dir = tempdir().unwrap();
    let math_path = dir.path().join("math.tmpr");
    let main_path = dir.path().join("main.tmpr");

    let math_src = r#"
fn double(x: Int64) -> Int64 {
    return x * 2
}

pub fn doubled(x: Int64) -> Int64 {
    return double(x)
}
"#;

    let main_src = r#"
import math;

pub fn main() -> Int64 {
    return math.doubled(21)
}
"#;

    fs::write(&math_path, math_src).unwrap();
    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(42));
}

#[test]
fn test_hir_unqualified_function_call_unchanged() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
pub fn main() {
    let _sum = add(10, 20)
}

fn add(a: Int64, b: Int64) -> Int64 {
    return a + b
}
"#;

    fs::write(&main_path, main_src).unwrap();

    tempra_driver::run_file_with_args(&main_path, &[]).unwrap();
}

#[test]
fn test_local_module_import() {
    let dir = tempdir().unwrap();
    let math_path = dir.path().join("math.tmpr");
    let main_path = dir.path().join("main.tmpr");

    let math_src = r#"
pub fn add(a: Int64, b: Int64) -> Int64 {
    return a + b;
}
"#;

    let main_src = r#"
import math;

pub fn main() -> Int64 {
    let sum = math.add(10, 20);
    if sum != 30 {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&math_path, math_src).unwrap();
    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_path_join() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.path;

pub fn main() -> Int64 {
    let res = std.path.join("a", "b");
    if res != "a/b" {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_strings_contains() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.strings;

pub fn main() -> Int64 {
    let res = std.strings.contains("hello world", "world");
    if !res {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_fs_exists() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");
    let fixture_path = dir.path().join("test_file.txt");

    fs::write(&fixture_path, "hello").unwrap();

    // Pass the tempdir path directly in the script using Rust to inject it
    let main_src = format!(
        r#"
import std.fs;

pub fn main() -> Int64 {{
    let path = "{}";
    let exists = std.fs.exists(path);
    if !exists {{
        return 1;
    }}
    return 0;
}}
"#,
        fixture_path.to_string_lossy()
    );

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_entry_module_resolution_app_main() {
    let dir = tempdir().unwrap();
    let app_path = dir.path().join("app.tmpr");

    let app_src = r#"
pub fn main() -> Int64 {
    return 0;
}
"#;

    fs::write(&app_path, app_src).unwrap();

    let res = run_tempra_vm_file(&app_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_entry_module_missing_main() {
    let dir = tempdir().unwrap();
    let app_path = dir.path().join("app.tmpr");

    let app_src = r#"
pub fn not_main() {
}
"#;

    fs::write(&app_path, app_src).unwrap();

    let res = run_tempra_vm_file(&app_path);
    assert!(res.is_err());
    if let Err(DriverError::UnsupportedExperimentalVm(msg)) = res {
        assert!(msg.contains("experimental VM currently requires main()"));
    } else {
        panic!("Expected UnsupportedExperimentalVm error, got {:?}", res);
    }
}

#[test]
fn test_stdlib_fs_read_text_ok() {
    let dir = tempdir().unwrap();
    let fixture_path = dir.path().join("hello.txt");
    let main_path = dir.path().join("main.tmpr");

    fs::write(&fixture_path, "hello from tempra").unwrap();

    let main_src = format!(
        r#"
import std.fs;

pub fn main() -> Int64 {{
    let result = std.fs.read_text("{}");
    return match result {{
        Ok(text) => 0
        Err(_) => 1
    }};
}}
"#,
        fixture_path.to_string_lossy()
    );

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_fs_read_text_missing() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.fs;

pub fn main() -> Int64 {
    let result = std.fs.read_text("nonexistent_file_xyz.txt");
    return match result {
        Ok(_) => 1
        Err(_) => 0
    };
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_fs_write_text_ok() {
    let dir = tempdir().unwrap();
    let out_path = dir.path().join("out.txt");
    let main_path = dir.path().join("main.tmpr");

    let main_src = format!(
        r#"
import std.fs;

pub fn main() -> Int64 {{
    let result = std.fs.write_text("{}", "written by tempra");
    return match result {{
        Ok(_) => 0
        Err(_) => 1
    }};
}}
"#,
        out_path.to_string_lossy()
    );

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
    // Verify the file was actually written
    assert_eq!(fs::read_to_string(&out_path).unwrap(), "written by tempra");
}

#[test]
fn test_stdlib_fs_write_text_invalid_path() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    // Use a path whose parent doesn't exist so write will fail
    let bad_path = dir.path().join("nonexistent_dir").join("out.txt");
    let main_src = format!(
        r#"
import std.fs;

pub fn main() -> Int64 {{
    let result = std.fs.write_text("{}", "data");
    return match result {{
        Ok(_) => 1
        Err(_) => 0
    }};
}}
"#,
        bad_path.to_string_lossy()
    );

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_strings_trim() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.strings;

pub fn main() -> Int64 {
    let trimmed = std.strings.trim("  hello  ");
    if trimmed != "hello" {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_strings_replace() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.strings;

pub fn main() -> Int64 {
    let result = std.strings.replace("hello world", "world", "Tempra");
    if result != "hello Tempra" {
        return 1;
    }
    return 0;
}
"#;

    fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();
    assert_eq!(res, VmValue::Int64(0));
}

#[test]
fn test_stdlib_io_println_statement() {
    let dir = tempfile::tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.io;

pub fn main() -> Int64 {
    std.io.println("hello")
    return 0
}
"#;

    std::fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();

    assert_eq!(res, tempra_vm::value::VmValue::Int64(0));
}

#[test]
fn test_stdlib_io_println_if_branch() {
    let dir = tempfile::tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");

    let main_src = r#"
import std.io;

pub fn main() -> Int64 {
    if true {
        std.io.println("hello")
    }

    return 0
}
"#;

    std::fs::write(&main_path, main_src).unwrap();

    let res = run_tempra_vm_file(&main_path).unwrap();

    assert_eq!(res, tempra_vm::value::VmValue::Int64(0));
}
