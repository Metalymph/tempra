use tempra_driver::pipeline::{diagnose_file, extract_symbols};
use std::fs;

#[test]
fn test_diagnose_valid_file() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("valid.tmpr");
    fs::write(&file_path, "fn main() { let x = 123; }").unwrap();

    let diags = diagnose_file(&file_path);
    assert!(diags.is_empty(), "expected no diagnostics but got: {:?}", diags);
    let _ = fs::remove_file(file_path);
}

#[test]
fn test_diagnose_parse_error() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("parse_error.tmpr");
    fs::write(&file_path, "fn main( {").unwrap(); // incomplete parameters list

    let diags = diagnose_file(&file_path);
    assert!(!diags.is_empty(), "expected parse error diagnostic");
    let diag = &diags[0];
    assert!(matches!(diag.severity, tempra_diagnostics::Severity::Error));
    assert!(diag.message.to_lowercase().contains("expected") || diag.message.to_lowercase().contains("parse"));

    // Verify JSON serialization format
    let file_str = file_path.to_string_lossy();
    let json_val = diag.to_json_value(Some(&file_str));
    assert_eq!(json_val["severity"], "error");
    assert_eq!(json_val["labels"][0]["span"]["file"], file_str.as_ref());
    let _ = fs::remove_file(file_path);
}

#[test]
fn test_diagnose_type_error() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("type_error.tmpr");
    fs::write(&file_path, "fn main() { let x: String = 123; }").unwrap(); // type mismatch

    let diags = diagnose_file(&file_path);
    assert!(!diags.is_empty(), "expected type error diagnostic");
    let diag = &diags[0];
    assert!(matches!(diag.severity, tempra_diagnostics::Severity::Error));

    let file_str = file_path.to_string_lossy();
    let json_val = diag.to_json_value(Some(&file_str));
    assert_eq!(json_val["severity"], "error");
    let _ = fs::remove_file(file_path);
}

#[test]
fn test_extract_symbols_valid() {
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("symbols.tmpr");
    fs::write(
        &file_path,
        r#"
        struct Point {
            x: Int64,
            y: Int64,
        }

        enum Color {
            Red,
            Blue,
        }

        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }

        extend Point {
            fn distance(self) -> Int64 {
                return 42
            }
        }
        "#,
    )
    .unwrap();

    let symbols = extract_symbols(&file_path).unwrap();
    assert_eq!(symbols.len(), 5); // Struct Point, Enum Color, fn add, extend Point, method Point.distance

    // Check Struct
    let s_struct = symbols.iter().find(|s| s.name == "Point").unwrap();
    assert!(matches!(s_struct.kind, tempra_diagnostics::SymbolKind::Struct));
    assert_eq!(s_struct.signature.as_deref().unwrap(), "struct Point");

    // Check Enum
    let s_enum = symbols.iter().find(|s| s.name == "Color").unwrap();
    assert!(matches!(s_enum.kind, tempra_diagnostics::SymbolKind::Enum));
    assert_eq!(s_enum.signature.as_deref().unwrap(), "enum Color");

    // Check fn
    let s_fn = symbols.iter().find(|s| s.name == "add").unwrap();
    assert!(matches!(s_fn.kind, tempra_diagnostics::SymbolKind::Function));
    assert_eq!(s_fn.signature.as_deref().unwrap(), "fn add(a: Int64, b: Int64) -> Int64");

    // Check Extension
    let s_ext = symbols.iter().find(|s| s.name == "extend Point").unwrap();
    assert!(matches!(s_ext.kind, tempra_diagnostics::SymbolKind::Extension));

    // Check Method
    let s_method = symbols.iter().find(|s| s.name == "Point.distance").unwrap();
    assert!(matches!(s_method.kind, tempra_diagnostics::SymbolKind::Method));
    assert_eq!(s_method.signature.as_deref().unwrap(), "fn distance(self: Self) -> Int64");

    let _ = fs::remove_file(file_path);
}
