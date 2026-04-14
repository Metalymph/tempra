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

#[test]
fn test_print() {
    let source = r#"
    fn main() {
        print("hello")
    }
    "#;
    let val = assert_runs(source);
    assert!(matches!(val, Value::Unit));
}

// Limitations: `read_line` is not directly tested here automatically since
// it blocks waiting for stdin in the default environment. Option B was chosen
// to test it manually via CLI (smoke test).
// #[test]
// fn test_read_line() {
//      // test logic
// }

#[test]
fn test_cmd_run() {
    let source = r#"
    fn main() {
        let r = cmd("echo", ["hello"]).run()
        print(r.stdout)
        r.stdout
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s.trim(), "hello");
    } else {
        panic!("Expected String(\"hello\"), found {:?}", val);
    }
}

#[test]
fn test_command_pipe() {
    let source = r#"
    fn main() {
        let r = cmd("echo", ["hello"]) | cmd("grep", ["h"])
        print(r.stdout)
        r.stdout
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s.trim(), "hello");
    } else {
        panic!("Expected String(\"hello\"), found {:?}", val);
    }
}

#[test]
fn test_command_and() {
    let source = r#"
    fn main() {
        let r = cmd("true", []) && cmd("echo", ["ok"])
        print(r.success)
        r.success
    }
    "#;
    let val = assert_runs(source);
    if let Value::Bool(b) = val {
        assert!(b);
    } else {
        panic!("Expected Bool(true), found {:?}", val);
    }
}

#[test]
fn test_command_or() {
    let source = r#"
    fn main() {
        let r = cmd("false", []) || cmd("echo", ["fallback"])
        print(r.success)
        r.success
    }
    "#;
    let val = assert_runs(source);
    if let Value::Bool(b) = val {
        assert!(b);
    } else {
        panic!("Expected Bool(true), found {:?}", val);
    }
}

#[test]
fn test_read_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_read.txt");
    std::fs::write(&path, "test_hello_world").unwrap();
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(r#"
    fn main() {{
        read("{}")
    }}
    "#, path_str);
    
    let val = assert_runs(&source);
    if let Value::String(s) = val {
        assert_eq!(s, "test_hello_world");
    } else {
        panic!("Expected String(\"test_hello_world\"), found {:?}", val);
    }
}

#[test]
fn test_write_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_write.txt");
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(r#"
    fn main() {{
        write("{}", "hello_fero")
    }}
    "#, path_str);
    
    let val = assert_runs(&source);
    assert!(matches!(val, Value::Unit));
    
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello_fero");
}

#[test]
fn test_append_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_append.txt");
    std::fs::write(&path, "hello").unwrap();
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(r#"
    fn main() {{
        append("{}", " world")
    }}
    "#, path_str);
    
    let val = assert_runs(&source);
    assert!(matches!(val, Value::Unit));
    
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello world");
}

#[test]
fn test_exit_propagates() {
    let source = r#"
    fn main() {
        exit(5)
    }
    "#;
    
    let mut ast = fero_parser::parse_source(source).unwrap();
    let hir = fero_hir::lower_file(source, &ast);
    let result = fero_interpreter::run(&hir);
    
    match result {
        Err(fero_interpreter::RuntimeError::Exit(5)) => {}
        other => panic!("Expected Exit(5) signal, got {:?}", other),
    }
}
