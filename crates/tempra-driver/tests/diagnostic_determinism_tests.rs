use std::fs;
use std::process::Command;

use tempra_driver::check_source;

const CHILD_MARKER: &str = "TEMPRA_K2_DIAGNOSTIC_CHILD";
const CHILD_OUTPUT: &str = "TEMPRA_K2_DIAGNOSTIC_OUTPUT";

const INTERFACE_ERRORS_SOURCE: &str = r#"
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

const ASSOCIATED_TYPE_SOURCE: &str = r#"
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

fn interface_error_text() -> String {
    check_source(INTERFACE_ERRORS_SOURCE)
        .expect_err("the fixture must fail interface conformance")
        .to_string()
}

fn assert_in_order(text: &str, needles: &[&str]) {
    let mut previous = None;
    for needle in needles {
        let position = text
            .find(needle)
            .unwrap_or_else(|| panic!("missing {needle:?} in:\n{text}"));
        if let Some(previous) = previous {
            assert!(
                previous < position,
                "{needle:?} appeared out of source order in:\n{text}"
            );
        }
        previous = Some(position);
    }
}

#[test]
fn missing_interface_requirements_follow_source_order() {
    let text = interface_error_text();
    assert_in_order(
        &text,
        &["missing requirement 'first'", "missing requirement 'third'"],
    );
    assert!(!text.contains("missing requirement 'second'"));
}

#[test]
fn repeated_fresh_contexts_have_identical_interface_diagnostics() {
    let expected = interface_error_text();
    for _ in 0..16 {
        assert_eq!(interface_error_text(), expected);
    }
}

#[test]
fn associated_type_first_match_uses_interface_declaration_order() {
    for _ in 0..16 {
        check_source(ASSOCIATED_TYPE_SOURCE).unwrap();
    }
}

#[test]
fn diagnostic_subprocess_child() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }
    fs::write(
        std::env::var_os(CHILD_OUTPUT).unwrap(),
        interface_error_text(),
    )
    .unwrap();
}

#[test]
fn independent_processes_have_identical_interface_diagnostics() {
    let dir = tempfile::tempdir().unwrap();
    let current_test = std::env::current_exe().unwrap();
    let expected = interface_error_text();

    for run in 0..16 {
        let output_path = dir.path().join(format!("diagnostic-{run}.txt"));
        let output = Command::new(&current_test)
            .arg("--exact")
            .arg("diagnostic_subprocess_child")
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env(CHILD_OUTPUT, &output_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child {run} failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read_to_string(output_path).unwrap(), expected);
    }
}

#[test]
fn missing_struct_fields_follow_declaration_order() {
    let source = r#"
struct Record {
    zebra: Int
    alpha: Int
    middle: Int
}
fn main() { let record = Record {} }
"#;
    let text = check_source(source).unwrap_err().to_string();
    assert_in_order(
        &text,
        &[
            "Missing required field 'zebra'",
            "Missing required field 'alpha'",
            "Missing required field 'middle'",
        ],
    );
}

#[test]
fn unknown_and_duplicate_literal_fields_keep_literal_order() {
    let source = r#"
struct Record { known: Int }
fn main() {
    let record = Record { first: 1, known: 2, second: 3, known: 4 }
}
"#;
    let text = check_source(source).unwrap_err().to_string();
    assert_in_order(
        &text,
        &[
            "has no field 'first'",
            "has no field 'second'",
            "Duplicate field 'known'",
        ],
    );
}
