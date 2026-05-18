use std::fs;
use std::process::Command;

#[test]
fn test_cli_check_json_valid() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("cli_valid.tmpr");
    fs::write(&file_path, "fn main() { let x = 123; }").unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "tempra",
            "--",
            "check",
            "--json",
            &file_path.to_string_lossy(),
        ])
        .output()
        .expect("failed to execute tempra");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json_val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json_val.get("diagnostics").is_some());
    assert!(json_val["diagnostics"].as_array().unwrap().is_empty());

    let _ = fs::remove_file(file_path);
}

#[test]
fn test_cli_diagnostics_json_invalid() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("cli_invalid.tmpr");
    fs::write(&file_path, "fn main() { let x: String = 123; }").unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "tempra",
            "--",
            "diagnostics",
            "--json",
            &file_path.to_string_lossy(),
        ])
        .output()
        .expect("failed to execute tempra");

    // Should return non-zero exit status on type errors
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json_val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json_val.get("diagnostics").is_some());
    let diags = json_val["diagnostics"].as_array().unwrap();
    assert!(!diags.is_empty());
    assert_eq!(diags[0]["severity"], "error");

    let _ = fs::remove_file(file_path);
}

#[test]
fn test_cli_symbols_json() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("cli_symbols.tmpr");
    fs::write(&file_path, "struct Point { x: Int64 } fn add(a: Int64) {}").unwrap();

    let output = Command::new("cargo")
        .args(&[
            "run",
            "--bin",
            "tempra",
            "--",
            "symbols",
            "--json",
            &file_path.to_string_lossy(),
        ])
        .output()
        .expect("failed to execute tempra");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json_val: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(json_val.get("symbols").is_some());
    let symbols = json_val["symbols"].as_array().unwrap();
    assert_eq!(symbols.len(), 2);
    assert_eq!(symbols[0]["name"], "Point");
    assert_eq!(symbols[0]["kind"], "struct");
    assert_eq!(symbols[1]["name"], "add");
    assert_eq!(symbols[1]["kind"], "function");

    let _ = fs::remove_file(file_path);
}
