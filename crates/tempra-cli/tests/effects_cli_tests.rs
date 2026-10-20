use serde_json::Value;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP_DIR: AtomicUsize = AtomicUsize::new(0);

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> Self {
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "tempra-effects-cli-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(path: &std::path::Path, json: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_tempra"));
    command.arg("effects");
    if json {
        command.arg("--json");
    }
    command.arg(path).output().unwrap()
}

fn write(source: &str) -> (TestDir, std::path::PathBuf) {
    let dir = TestDir::new();
    let path = dir.path().join("app.tmpr");
    std::fs::write(&path, source).unwrap();
    (dir, path)
}

fn json_stdout(output: &Output) -> Value {
    assert!(
        output.stderr.is_empty(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("stdout must contain exactly one JSON document")
}

#[test]
fn json_success_is_one_deterministic_machine_document() {
    let (_dir, path) = write(
        "import std.io; @deny(process) pub fn helper() { std.io.println(\"x\") } @pure fn local() -> String { return \"x\" } pub fn main() {}",
    );
    let first = run(&path, true);
    assert!(first.status.success());
    let document = json_stdout(&first);
    assert!(document["diagnostics"].as_array().unwrap().is_empty());
    let functions = document["functions"].as_array().unwrap();
    assert!(functions.len() >= 3);
    let helper = functions
        .iter()
        .find(|function| function["function"]["name"] == "app.helper")
        .unwrap();
    assert_eq!(helper["direct"], serde_json::json!(["alloc", "io"]));
    assert_eq!(
        helper["contract"]["explicit_denied"],
        serde_json::json!(["process"])
    );
    let second = run(&path, true);
    assert!(second.status.success());
    json_stdout(&second);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn json_capability_failure_is_single_document_with_empty_stderr_and_exit_one() {
    let (_dir, path) = write(
        "import std.io; @pure @deny(io) pub fn f() { std.io.println(\"x\") } pub fn main() {}",
    );
    let first = run(&path, true);
    assert_eq!(first.status.code(), Some(1));
    let document = json_stdout(&first);
    assert!(document["functions"].as_array().unwrap().is_empty());
    let diagnostic = &document["diagnostics"][0];
    assert_eq!(diagnostic["code"], "capability-denied");
    assert_eq!(diagnostic["severity"], "error");
    assert_eq!(diagnostic["capability"], "io");
    assert_eq!(diagnostic["function"]["name"], "app.f");
    assert_eq!(
        diagnostic["contract_origins"],
        serde_json::json!(["deny", "pure"])
    );
    assert_eq!(diagnostic["witness"]["origin"]["kind"], "extern_call");
    assert_eq!(
        diagnostic["witness"]["origin"]["external"]["name"],
        "std.io.println"
    );
    let second = run(&path, true);
    assert_eq!(second.status.code(), Some(1));
    json_stdout(&second);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn json_non_capability_failure_uses_existing_diagnostic_shape_only() {
    let (_dir, path) = write("fn main( {");
    let output = run(&path, true);
    assert_eq!(output.status.code(), Some(1));
    let document = json_stdout(&output);
    assert!(document["functions"].as_array().unwrap().is_empty());
    let diagnostic = &document["diagnostics"][0];
    assert_eq!(diagnostic["severity"], "error");
    assert!(diagnostic.get("code").is_some());
    assert!(diagnostic["message"]
        .as_str()
        .unwrap()
        .contains("Type error"));
    assert!(diagnostic["labels"].as_array().unwrap().is_empty());
    assert!(diagnostic["notes"].as_array().unwrap().is_empty());
}

#[test]
fn human_violation_uses_real_source_and_named_witness() {
    let (_dir, path) =
        write("import std.io;\n@deny(io)\npub fn f() { std.io.println(\"x\") }\npub fn main() {}");
    let output = run(&path, false);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error[capability-denied]"));
    assert!(stderr.contains("app.f -> std.io.println -> io"));
    assert!(stderr.contains("@deny(io)"));
    assert!(!stderr.contains("CapabilityContractViolation"));
}

#[test]
fn json_specialized_violation_is_not_reported_as_successful_inspection() {
    let (_dir, path) = write(
        r#"
import std.fs;

@deny(fs.write(destination: "blocked.txt"))
pub fn main() -> Int64 {
    let result = std.fs.write_text("blocked.txt", "data");
    return match result { Ok(_) => 0 Err(_) => 1 };
}
"#,
    );
    let output = run(&path, true);
    assert_eq!(output.status.code(), Some(1));
    let document = json_stdout(&output);
    assert!(document["functions"].as_array().unwrap().is_empty());
    let diagnostic = &document["diagnostics"][0];
    assert_eq!(diagnostic["severity"], "error");
    assert!(diagnostic["message"]
        .as_str()
        .unwrap()
        .contains("Specialized capability contract violation"));
}

#[test]
fn json_transitive_and_multiple_violations_remain_structured_and_stable() {
    let dir = TestDir::new();
    std::fs::write(
        dir.path().join("inner.tmpr"),
        "import std.io; pub fn work() { std.io.println(\"x\") }",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("middle.tmpr"),
        "import inner; pub fn work() { inner.work() }",
    )
    .unwrap();
    let path = dir.path().join("outer.tmpr");
    std::fs::write(
        &path,
        "import middle; @deny(alloc, io) pub fn outer() { let text = \"x\"; middle.work() } pub fn main() {}",
    )
    .unwrap();

    let first = run(&path, true);
    assert_eq!(first.status.code(), Some(1));
    let document = json_stdout(&first);
    let diagnostics = document["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["capability"], "alloc");
    assert_eq!(diagnostics[1]["capability"], "io");
    let calls = diagnostics[1]["witness"]["calls"].as_array().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["caller"]["name"], "outer.outer");
    assert_eq!(calls[0]["callee"]["name"], "middle.work");
    assert_eq!(calls[1]["callee"]["name"], "inner.work");

    let second = run(&path, true);
    assert_eq!(second.status.code(), Some(1));
    json_stdout(&second);
    assert_eq!(first.stdout, second.stdout);
}
