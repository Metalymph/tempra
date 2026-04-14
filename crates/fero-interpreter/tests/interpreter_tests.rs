use fero_hir::lower_file;
use fero_interpreter::{run, value::Value};
use fero_parser::parse_source;

fn assert_runs(source: &str) -> Value {
    let ast = parse_source(source).expect("Failed to parse source");
    let hir = lower_file(source, &ast);
    run(&hir).expect("Failed to run interpreter")
}

#[test]
fn test_1() {
    let source = "
    fn main() {
        1
    }
    ";
    let val = assert_runs(source);
    if let Value::Int64(i) = val {
        assert_eq!(i, 1);
    } else {
        panic!("Expected Int64(1), found {:?}", val);
    }
}

#[test]
fn test_2() {
    let source = "
    fn main() {
        1 + 2
    }
    ";
    let val = assert_runs(source);
    if let Value::Int64(i) = val {
        assert_eq!(i, 3);
    } else {
        panic!("Expected Int64(3), found {:?}", val);
    }
}

#[test]
fn test_3() {
    let source = "
    fn main() {
        if true { 1 } else { 2 }
    }
    ";
    let val = assert_runs(source);
    if let Value::Int64(i) = val {
        assert_eq!(i, 1);
    } else {
        panic!("Expected Int64(1), found {:?}", val);
    }
}

#[test]
fn test_4() {
    let source = "
    fn add(a: Int64, b: Int64) -> Int64 {
        a + b
    }

    fn main() {
        add(1, 2)
    }
    ";
    let val = assert_runs(source);
    if let Value::Int64(i) = val {
        assert_eq!(i, 3);
    } else {
        panic!("Expected Int64(3), found {:?}", val);
    }
}

#[test]
fn test_5() {
    let source = "
    struct User {
        name: String
    }

    fn main() {
        let u = User { name: \"a\" }
        u.name
    }
    ";
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s, "a");
    } else {
        panic!("Expected String(\"a\"), found {:?}", val);
    }
}
