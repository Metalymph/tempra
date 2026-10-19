use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tempra-run-entry-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_source(source: &str, experimental_vm: bool) -> Output {
    run_source_with_flags(source, experimental_vm, false)
}

fn run_source_with_flags(source: &str, experimental_vm: bool, experimental_jit: bool) -> Output {
    let dir = TestDir::new();
    let path = dir.path().join("app.tmpr");
    std::fs::write(&path, source).unwrap();

    let mut command = Command::new(env!("CARGO_BIN_EXE_tempra"));
    command.arg("run");
    if experimental_vm {
        command.arg("--experimental-vm");
    }
    if experimental_jit {
        command.arg("--experimental-jit");
    }
    command.arg(&path);
    command.output().unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(output));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(output.stderr.is_empty(), "stderr: {}", stderr(output));
}

#[test]
fn main_unit_exits_zero_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source("fn main() {}", experimental_vm);
        assert_success(&output);
    }
}

#[test]
fn main_int_zero_exits_zero_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source("pub fn main() -> Int64 { return 0 }", experimental_vm);
        assert_success(&output);
    }
}

#[test]
fn main_int_seven_exits_seven_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source("pub fn main() -> Int64 { return 7 }", experimental_vm);
        assert_eq!(output.status.code(), Some(7), "stderr: {}", stderr(&output));
        assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
        assert!(output.stderr.is_empty(), "stderr: {}", stderr(&output));
    }
}

#[test]
fn main_result_ok_exits_zero_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source(
            "pub fn main() -> Result(Int64, String) { return Ok(42) }",
            experimental_vm,
        );
        assert_success(&output);
    }
}

#[test]
fn main_result_err_reports_uncaught_error_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source(
            "pub fn main() -> Result(Int64, String) { return Err(\"boom\") }",
            experimental_vm,
        );
        assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
        assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
        assert_eq!(stderr(&output), "error: Uncaught error: boom\n");
    }
}

#[test]
fn explicit_exit_preserves_hir_exit_code() {
    let output = run_source("fn main() { exit(7) }", false);
    assert_eq!(output.status.code(), Some(7), "stderr: {}", stderr(&output));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(output.stderr.is_empty(), "stderr: {}", stderr(&output));
}

#[test]
fn explicit_exit_remains_unsupported_in_experimental_vm() {
    let output = run_source("fn main() { exit(7) }", true);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("Unsupported by experimental VM"),
        "{stderr}"
    );
    assert!(stderr.contains("unknown function: exit"), "{stderr}");
}

#[test]
fn main_int_outside_portable_range_is_rejected_without_truncation() {
    for experimental_vm in [false, true] {
        for (source, code) in [
            ("pub fn main() -> Int64 { return 256 }", "256"),
            ("pub fn main() -> Int64 { return -1 }", "-1"),
        ] {
            let output = run_source(source, experimental_vm);
            assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
            let stderr = stderr(&output);
            assert!(
                stderr.contains(&format!(
                    "main returned exit code {code}; portable exit codes are 0..=255"
                )),
                "{stderr}"
            );
        }
    }
}

#[test]
fn main_unsupported_return_type_is_rejected_in_hir_and_vm() {
    for experimental_vm in [false, true] {
        let output = run_source("pub fn main() -> Bool { return true }", experimental_vm);
        assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
        let stderr = stderr(&output);
        assert!(
            stderr.contains(
                "main returned unsupported entry-point value of type Bool; expected Unit, Int, or Result"
            ),
            "{stderr}"
        );
    }
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn entry_results_are_applied_in_experimental_jit_mode() {
    let output = run_source_with_flags("pub fn main() -> Int64 { return 7 }", true, true);
    assert_eq!(output.status.code(), Some(7), "stderr: {}", stderr(&output));

    let output = run_source_with_flags(
        "pub fn main() -> Result(Int64, String) { return Err(\"boom\") }",
        true,
        true,
    );
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stderr(&output), "error: Uncaught error: boom\n");
}
