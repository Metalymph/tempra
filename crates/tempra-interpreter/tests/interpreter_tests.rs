use tempra_hir::lower_file;
use tempra_interpreter::{
    run, run_with_args, value::OptionValue, value::Value, value::ValueResult,
};

use tempra_parser::parse_source;

fn assert_runs(source: &str) -> Value {
    assert_runs_with_args(source, vec![])
}

fn assert_runs_with_args(source: &str, args: Vec<String>) -> Value {
    let ast = parse_source(source).expect("Failed to parse source");
    let hir = lower_file(source, &ast);
    run_with_args(&hir, args).expect("Failed to run interpreter")
}

#[test]
fn test_as_optional_interpreter() {
    let source = r#"
    fn main() {
        expect(("hello" as? String) == Some("hello"))
        expect((123 as? String) == None)
        expect(((123 as! Int) as? String) == None)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_as_optional_option_casts() {
    let source = r#"
    fn main() {
        let x = Some("hello")
        let y = x as? String?
        expect(y == Some(Some("hello"))) // No unwrap, correctly wrapped Option

        let a = Some(123)
        let b = a as? String?
        expect(b == None)

        let i = None
        let j = i as? String?
        expect(j == Some(None))
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_as_optional_result_casts() {
    let source = r#"
    fn main() {
        let x: String! = Ok("hello")
        let y = x as? String!;
        let ok = y is (String!)?
        expect(ok)
        expect(y == Some(Ok("hello")))

        let a: Int! = Ok(123)
        let b = a as? String!;
        expect(b == None)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_is_pattern_some_success() {
    let source = r#"
    fn main() {
        let x = Some("hello")
        let hit = false
        if x is Some(v) {
            expect(v == "hello")
            hit = true
        }
        expect(hit)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_is_pattern_some_failure() {
    let source = r#"
    fn main() {
        let x = None
        if x is Some(v) {
            crash("should not happen")
        }
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_is_pattern_none_success() {
    let source = r#"
    fn main() {
        let x = None
        let hit = false
        if x is None {
            hit = true
        }
        expect(hit)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_is_pattern_ok() {
    let source = r#"
    fn main() {
        let r = Ok("hello")
        if r is Ok(v) {
            expect(v == "hello")
        }
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_is_pattern_isolation() {
    let source = r#"
    fn main() {
        let x = 1
        let y = Some(2)
        if y is Some(x) {
            expect(x == 2)
        }
        expect(x == 1)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_as_forced_interpreter_success() {
    let source = r#"
    fn main() {
        let x = "hello" as! String
        expect(x == "hello")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_as_forced_interpreter_crash() {
    let source = r#"
    fn main() {
        let x = 1 as! String
    }
    "#;
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    let result = tempra_interpreter::run(&hir);
    match result {
        Err(tempra_interpreter::RuntimeError::Crash(msg)) => {
            assert!(msg.contains("invalid cast: expected String"));
        }
        other => panic!("Expected Crash for invalid cast, got {:?}", other),
    }
}

#[test]
fn test_os_args() {
    let source = r#"
    fn main() {
        os.args()
    }
    "#;
    let val = assert_runs_with_args(source, vec!["hello".to_string(), "world".to_string()]);
    if let Value::Array(arr) = val {
        let b = arr.borrow();
        assert_eq!(b.len(), 2);
        assert_eq!(b[0].to_string(), "hello");
        assert_eq!(b[1].to_string(), "world");
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_os_cwd_chdir() {
    let source = r#"
    fn main() {
        let initial = os.cwd()!
        os.chdir("..")!
        let second = os.cwd()!
        // Try to go back
        os.chdir(initial)!
        second
    }
    "#;
    let initial_cwd = std::env::current_dir().unwrap();
    let val = assert_runs(source);
    // Restoration is handled by the script itself above, but let's be double safe
    std::env::set_current_dir(&initial_cwd).unwrap();

    if let Value::String(s) = val {
        assert!(s.len() < initial_cwd.to_string_lossy().len());
    } else {
        panic!("Expected string, got {:?}", val);
    }
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
    fn add(a: Int, b: Int) -> Int {
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
fn test_print_variadic() {
    let source = r#"
    fn main() {
        print("hello ", "world")
    }
    "#;
    let val = assert_runs(source);
    assert!(matches!(val, Value::Unit));
}

#[test]
fn test_fmt_variadic() {
    let source = r#"
    fn main() {
        let s = fmt("hello ", "world")
        print(s)
        s
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s, "hello world");
    } else {
        panic!("Expected String(\"hello world\"), found {:?}", val);
    }
}

#[test]
fn test_mixed_types() {
    let source = r#"
    fn main() {
        print("val=", 5, " ok=", true)
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
        let r = cmd("echo", "hello").run()!
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
        let r = (cmd("echo", "hello") | cmd("grep", "h")) // pipe returns CommandResult directly currently
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
        let r = (cmd("true") && cmd("echo", "ok"))!
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
        let r = (cmd("false") || cmd("echo", "fallback"))!
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

    let source = format!(
        r#"
    fn main() {{
        fs.read("{}")!
    }}
    "#,
        path_str
    );

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

    let source = format!(
        r#"
    fn main() {{
        fs.write("{}", "hello_tempra")!
    }}
    "#,
        path_str
    );

    let val = assert_runs(&source);
    assert!(matches!(val, Value::Unit));

    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello_tempra");
}

#[test]
fn test_append_file() {
    let dir = std::env::temp_dir();
    let path = dir.join("test_append.txt");
    std::fs::write(&path, "hello").unwrap();
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(
        r#"
    fn main() {{
        fs.append("{}", " world")!
    }}
    "#,
        path_str
    );

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

    let ast = tempra_parser::parse_source(source).unwrap();
    let hir = tempra_hir::lower_file(source, &ast);
    let result = tempra_interpreter::run(&hir);

    match result {
        Err(tempra_interpreter::RuntimeError::Exit(5)) => {}
        other => panic!("Expected Exit(5) signal, got {:?}", other),
    }
}

#[test]
fn test_expect_pass() {
    let source = r#"
    fn main() {
        expect(true)
        "ok"
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s, "ok");
    } else {
        panic!("Expected String(\"ok\"), got {:?}", val);
    }
}

#[test]
fn test_expect_fail() {
    let source = r#"
    fn main() {
        expect(false)
    }
    "#;
    let ast = tempra_parser::parse_source(source).unwrap();
    let hir = tempra_hir::lower_file(source, &ast);
    let result = tempra_interpreter::run(&hir);
    match result {
        Err(tempra_interpreter::RuntimeError::Crash(msg)) => {
            assert_eq!(msg, "expectation failed");
        }
        other => panic!("Expected Crash signal, got {:?}", other),
    }
}

#[test]
fn test_expect_fail_message() {
    let source = r#"
    fn main() {
        expect(false, "my custom message")
    }
    "#;
    let ast = tempra_parser::parse_source(source).unwrap();
    let hir = tempra_hir::lower_file(source, &ast);
    let result = tempra_interpreter::run(&hir);
    match result {
        Err(tempra_interpreter::RuntimeError::Crash(msg)) => {
            assert_eq!(msg, "my custom message");
        }
        other => panic!("Expected Crash signal, got {:?}", other),
    }
}

#[test]
fn test_dbg_eval() {
    let source = r#"
    fn main() {
        let x = dbg(123)
        x
    }
    "#;
    let val = assert_runs(source);
    if let Value::Int64(i) = val {
        assert_eq!(i, 123);
    } else {
        panic!("Expected Int64(123), got {:?}", val);
    }
}

#[test]
fn test_crash() {
    let source = r#"
    fn main() {
        crash("boom")
    }
    "#;
    let ast = tempra_parser::parse_source(source).unwrap();
    let hir = tempra_hir::lower_file(source, &ast);
    let result = tempra_interpreter::run(&hir);
    match result {
        Err(tempra_interpreter::RuntimeError::Crash(msg)) => {
            assert_eq!(msg, "boom");
        }
        other => panic!("Expected Crash signal, got {:?}", other),
    }
}

#[test]
fn test_env_get_existing() {
    std::env::set_var("TEMPRA_TEST_VAR", "tempra_is_cool");
    let source = r#"
    fn main() {
        env.get("TEMPRA_TEST_VAR")!
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(s, "tempra_is_cool");
    } else {
        panic!("Expected String(tempra_is_cool), got {:?}", val);
    }
}

#[test]
fn test_env_get_missing() {
    let source = r#"
    fn main() {
        env.get("NON_EXISTENT_VAR_XYZ")!
    }
    "#;
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    let res = run(&hir);
    assert!(res.is_err(), "Expected error for missing env var");
}

#[test]
fn test_fs_exists() {
    let dir = std::env::temp_dir();
    let path = dir.join("tempra_exists.txt");
    std::fs::write(&path, "exists").unwrap();
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(
        r#"
    fn main() {{
        let e = fs.exists("{}")!
        expect(e)
        let non_existent = fs.exists("non_existent_file_123.txt")!
        expect(!non_existent)
        "ok"
    }}
    "#,
        path_str
    );

    let val = assert_runs(&source);
    assert_eq!(val, Value::String("ok".to_string()));
}

#[test]
fn test_fs_remove() {
    let dir = std::env::temp_dir();
    let path = dir.join("tempra_remove.txt");
    std::fs::write(&path, "remove me").unwrap();
    let path_str = path.to_str().unwrap().replace("\\", "\\\\");

    let source = format!(
        r#"
    fn main() {{
        expect(fs.exists("{}")!)
        fs.remove("{}")!
        expect(!fs.exists("{}")!)
        "removed"
    }}
    "#,
        path_str, path_str, path_str
    );

    let val = assert_runs(&source);
    assert_eq!(val, Value::String("removed".to_string()));
}
#[test]
fn test_named_arguments_execution() {
    let source = r#"
    fn greet(name: String, age: Int) -> String {
        fmt("Hello ", name, ", you are ", age)
    }

    fn main() {
        let s1 = greet(name="Lorenzo", age=25)
        let s2 = greet(age=30, name="Antigravity")
        let s3 = greet("Pair", age=100)
        fmt(s1, " | ", s2, " | ", s3)
    }
    "#;
    let val = assert_runs(source);
    if let Value::String(s) = val {
        assert_eq!(
            s,
            "Hello Lorenzo, you are 25 | Hello Antigravity, you are 30 | Hello Pair, you are 100"
        );
    } else {
        panic!("Expected String, got {:?}", val);
    }
}

#[test]
fn test_ergonomic_builtins_execution() {
    // fmt with separator
    let source = r#"
    fn main() {
        fmt("a", "b", "c", separator=", ")
    }
    "#;
    let val = assert_runs(source);
    assert_eq!(val, Value::String("a, b, c".to_string()));

    // cmd variadic
    let source = r#"
    fn main() {
        let c = cmd("echo", "hello", "world")
        c.args[0] + " " + c.args[1]
    }
    "#;
    let val = assert_runs(source);
    assert_eq!(val, Value::String("hello world".to_string()));

    // cmd with check=true (success)
    let source = r#"
    fn main() {
        cmd("true", check=true).run()!
        "ok"
    }
    "#;
    let val = assert_runs(source);
    assert_eq!(val, Value::String("ok".to_string()));
}

#[test]
fn test_cmd_check_failure() {
    let source = r#"
    fn main() {
        cmd("false", check=true).run()
    }
    "#;
    let val = assert_runs(source);
    // Should return Err
    if let Value::Result(res) = val {
        assert!(matches!(res.as_ref(), ValueResult::Err(_)));
    } else {
        panic!("Expected Result::Err, got {:?}", val);
    }
}

#[test]
fn test_string_methods() {
    let source = r#"
    fn main() {
        expect("abc".len() == 3)
        expect("  hi  ".trim() == "hi")
        expect("hello".contains("ell"))
        expect(!"hello".contains("zzz"))
        expect("hello".starts_with("he"))
        expect("hello".ends_with("lo"))
        expect("a,b,c".split(",").len() == 3)
        expect("abc".replace("a", "z") == "zbc")
        expect("AbC".to_lower() == "abc")
        expect("AbC".to_upper() == "ABC")
    }
    "#;
    assert_runs(source);
}

#[test]
fn test_fs_v0_2() {
    let _ = std::fs::remove_dir_all("tmp_fs_test");
    let source = r#"
    fn main() ! {
        let dir = "tmp_fs_test"
        fs.mkdir(dir)!
        expect(fs.exists(dir)!)
        
        let f1 = "tmp_fs_test/a.txt"
        fs.write(f1, "hello")!
        
        let entries = fs.list_dir(dir)!
        expect(entries.len() == 1)
        expect(entries[0] == "a.txt")
        
        let f2 = "tmp_fs_test/b.txt"
        fs.rename(f1, f2)!
        expect(!fs.exists(f1)!)
        expect(fs.exists(f2)!)
        
        let f3 = "tmp_fs_test/c.txt"
        fs.copy(f2, f3)!
        expect(fs.exists(f2)!)
        expect(fs.exists(f3)!)
        expect(fs.read(f3)! == "hello")
        
        fs.remove(f3)!
        fs.remove(f2)!
        // std::fs::remove_dir is not in Tempra yet
    }
    "#;
    assert_runs(source);
    let _ = std::fs::remove_dir("tmp_fs_test");
}

#[test]
fn test_array_slices() {
    let source = r#"
    fn main() {
        let xs = [1, 2, 3, 4]
        let p1 = xs[1..3]
        expect(p1.len() == 2)
        expect(p1[0] == 2)
        expect(p1[1] == 3)
        
        let p2 = xs[0..4]
        expect(p2.len() == 4)
        
        let p3 = xs[2..2]
        expect(p3.len() == 0)
    }
    "#;
    assert_runs(source);
}

#[test]
fn test_array_slices_oob() {
    let source = r#"
    fn main() {
        let xs = [1, 2, 3]
        let p = xs[1..5]
    }
    "#;
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    let result = run(&hir);
    match result {
        Err(tempra_interpreter::RuntimeError::Crash(msg)) => {
            assert!(msg.contains("Slice bounds out of range"));
        }
        other => panic!("Expected Crash for OOB slice, got {:?}", other),
    }
}

#[test]
fn test_struct_defaults_interpreter() {
    let source = r#"
    struct User {
        name: String = "anon"
        age: Int = 0
    }
    fn main() {
        let u1 = User {}
        let u2 = User { name: "me" }
        fmt(u1.name, " ", u1.age, " | ", u2.name, " ", u2.age)
    }
    "#;
    let val = assert_runs(source);
    assert_eq!(val, Value::String("anon 0 | me 0".to_string()));
}

#[test]
fn test_struct_field_mutation() {
    let source = r#"
    struct User {
        name: String
        var age: Int = 0
    }
    fn main() {
        let u = User { name: "me" }
        u.age = 29
        u.age
    }
    "#;
    let val = assert_runs(source);
    assert_eq!(val, Value::Int64(29));
}

#[test]
fn test_struct_formatting() {
    let source = r#"
    struct User { name: String, age: Int }
    fn main() {
        let u = User { name: "Lorenzo", age: 29 }
        fmt(u)
    }
    "#;
    let val = assert_runs(source);
    // Fields should be sorted for consistency
    assert_eq!(
        val,
        Value::String("User { age: 29, name: \"Lorenzo\" }".to_string())
    );
}

#[test]
fn test_string_unicode_interpreter() {
    // ASCII
    assert_eq!(assert_runs(r#"fn main() { "abc".len() }"#), Value::Int64(3));
    assert_eq!(
        assert_runs(r#"fn main() { "abc".size() }"#),
        Value::Int64(3)
    );

    // Unicode (h + é)
    // "é" is U+00E9, which is 2 bytes in UTF-8 (C3 A9)
    assert_eq!(assert_runs(r#"fn main() { "hé".len() }"#), Value::Int64(2));
    assert_eq!(assert_runs(r#"fn main() { "hé".size() }"#), Value::Int64(3));

    // Iteration
    let source = r#"
    fn main() {
        var count = 0
        for ch in "hé" {
            count = count + 1
        }
        count
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(2));

    // Values
    let source = r#"
    fn main() {
        var last = ""
        for ch in "hé" {
            last = ch
        }
        last
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("é".to_string()));
}

#[test]
fn test_slice_model_interpreter() {
    // Array to Slice
    let source = r#"
    fn main() {
        let xs = [10, 20, 30, 40]
        let ys = xs[1..3]
        ys[0]
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(20));

    // Slice len
    let source = r#"
    fn main() {
        let xs = [10, 20, 30, 40]
        let ys = xs[1..3]
        ys.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(2));

    // Nested Slice
    let source = r#"
    fn main() {
        let xs = [1, 2, 3, 4, 5]
        let ys = xs[1..5]
        let zs = ys[1..3]
        zs[0]
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(3));

    // Nested Slice len
    let source = r#"
    fn main() {
        let xs = [1, 2, 3, 4, 5]
        let ys = xs[1..5]
        let zs = ys[1..3]
        zs.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(2));

    // Full range
    let source = r#"
    fn main() {
        let xs = [1, 2]
        let ys = xs[0..2]
        ys.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(2));

    // Empty range
    let source = r#"
    fn main() {
        let xs = [1, 2]
        let ys = xs[1..1]
        ys.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(0));
}

#[test]
fn test_string_bytes() {
    // bytes().len() == size()
    let source = r#"
    fn main() {
        let s = "hé"
        let bs = s.bytes()
        bs.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(3)); // "hé" = 3 UTF-8 bytes

    // ASCII: bytes() indexing
    let source = r#"
    fn main() {
        let bs = "A".bytes()
        bs[0]
    }
    "#;
    assert_eq!(assert_runs(source), Value::UInt8(65)); // 'A' = 65

    // bytes() len for ASCII matches size()
    let source = r#"
    fn main() {
        "hello".bytes().len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(5));

    // bytes() slicing
    let source = r#"
    fn main() {
        let bs = "hé".bytes()
        let part = bs[0..1]
        part.len()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(1));

    // bytes() iteration count
    let source = r#"
    fn main() {
        var count = 0
        for b in "hé".bytes() {
            count = count + 1
        }
        count
    }
    "#;
    assert_eq!(assert_runs(source), Value::Int64(3)); // 3 bytes in "hé"
}

#[test]
fn test_regex_text_match() {
    let source = r#"
    fn main() {
        let re = Regex.text("abc")
        expect(re.is_match("zabcx"))
        expect(!re.is_match("zzz"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_seq() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.text("a"), Regex.text("b"))
        expect(re.is_match("xabcy"))
        expect(!re.is_match("xacy"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_alt() {
    let source = r#"
    fn main() {
        let re = Regex.alt(Regex.text("cat"), Regex.text("dog"))
        expect(re.is_match("hotdog"))
        expect(re.is_match("catnap"))
        expect(!re.is_match("bird"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_optional() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.text("a"), Regex.optional(Regex.text("b")), Regex.text("c"))
        expect(re.is_match("abc"))
        expect(re.is_match("ac"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_zero_or_more() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.text("a"), Regex.zero_or_more(Regex.text("b")), Regex.text("c"))
        expect(re.is_match("ac"))
        expect(re.is_match("abbbc"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_one_or_more() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.text("a"), Regex.one_or_more(Regex.text("b")), Regex.text("c"))
        expect(!re.is_match("ac"))
        expect(re.is_match("abc"))
        expect(re.is_match("abbbc"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_start_end() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.start(), Regex.text("abc"), Regex.end())
        expect(re.is_match("abc"))
        expect(!re.is_match("zabc"))
        expect(!re.is_match("abcz"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_start_anchor() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.start(), Regex.text("abc"))
        expect(re.is_match("abc"))
        expect(!re.is_match("zabc"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_end_anchor() {
    let source = r#"
    fn main() {
        let re = Regex.seq(Regex.text("abc"), Regex.end())
        expect(re.is_match("abc"))
        expect(!re.is_match("abcz"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_digit_word_space() {
    let source = r#"
    fn main() {
        expect(Regex.digit().is_match("a1"))
        expect(!Regex.digit().is_match("ab"))
        expect(Regex.word().is_match("!a"))
        expect(!Regex.word().is_match("!?"))
        expect(Regex.space().is_match("x y"))
        expect(!Regex.space().is_match("xy"))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_regex_replace() {
    let source = r#"
    fn main() {
        let re = Regex.text("abc")
        expect(re.replace("abc abc", "X") == "X X")
        re.replace("abc xyz abc", "X")
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("X xyz X".to_string()));
}

#[test]
fn test_regex_zero_width_optional_replace_terminates() {
    let source = r#"
    fn main() {
        let re = Regex.optional(Regex.text("x"))
        re.replace("abc", "_")
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("_a_b_c_".to_string()));
}

#[test]
fn test_regex_zero_width_zero_or_more_replace_terminates() {
    let source = r#"
    fn main() {
        let re = Regex.zero_or_more(Regex.text("a"))
        re.replace("bbb", "_")
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("_b_b_b_".to_string()));
}

#[test]
fn test_regex_zero_width_end_anchor_replace() {
    let source = r#"
    fn main() {
        Regex.end().replace("abc", "_")
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("abc_".to_string()));
}

#[test]
fn test_regex_unicode_scalar_sanity() {
    let source = r#"
    fn main() {
        let re = Regex.text("é")
        expect(re.is_match("héllo"))
        expect(!re.is_match("hello"))
        expect(re.replace("héhé", "X") == "hXhX")
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_option_some_formatting() {
    let source = r#"
    fn main() {
        let x = Some(1)
        print(x)
        x
    }
    "#;
    assert_eq!(
        assert_runs(source),
        Value::Option(OptionValue::Some(Box::new(Value::Int64(1))))
    );
}

#[test]
fn test_option_none_formatting() {
    let source = r#"
    fn main() {
        print(None)
        None
    }
    "#;
    assert_eq!(assert_runs(source), Value::Option(OptionValue::None));
}

#[test]
fn test_option_function_return_some() {
    let source = r#"
    fn f() -> Int? {
        Some(1)
    }

    fn main() {
        print(f())
        f()
    }
    "#;
    assert_eq!(
        assert_runs(source),
        Value::Option(OptionValue::Some(Box::new(Value::Int64(1))))
    );
}

#[test]
fn test_option_function_return_none() {
    let source = r#"
    fn f() -> Int? {
        None
    }

    fn main() {
        print(f())
        f()
    }
    "#;
    assert_eq!(assert_runs(source), Value::Option(OptionValue::None));
}

#[test]
fn test_is_operator_primitives_runtime() {
    let source = r#"
    fn main() {
        expect("abc" is String)
        expect(!(1 is String))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_is_operator_option_runtime() {
    let source = r#"
    fn main() {
        expect(Some(1) is Int?)
        expect(None is Int?)
        expect(!(Some(1) is String?))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_is_operator_result_runtime() {
    let source = r#"
    struct IoError {}

    fn main() {
        expect(Ok("x") is String!)
        expect(Err(IoError {}) is String!IoError)
        expect(!(Ok("x") is Int!))
        true
    }
    "#;
    assert_eq!(assert_runs(source), Value::Bool(true));
}

#[test]
fn test_is_operator_option_string_runtime() {
    let source = r#"
    let x = Some("hello")
    expect(x is String?)
    expect(!(x is Int?))
    "#;
    assert_eq!(
        assert_runs(source),
        Value::Result(Box::new(ValueResult::Ok(Value::Unit)))
    );
}

#[test]
fn test_match_runtime_some_none() {
    let source = r#"
    fn main() {
        let x = Some("hello")
        let y = match x {
            Some(v) => v,
            None => "default",
        }
        expect(y == "hello")

        let x2 = None
        let y2 = match x2 {
            Some(v) => v,
            None => "default",
        }
        expect(y2 == "default")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_ok_err() {
    let source = r#"
    struct MyError {}
    fn main() {
        let r = Ok("hello")
        let y = match r {
            Ok(v) => v,
            Err(e) => "fallback",
        }
        expect(y == "hello")

        let r2 = Err(MyError {})
        let y2 = match r2 {
            Ok(v) => "ok",
            Err(e) => "fallback",
        }
        expect(y2 == "fallback")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_wildcard() {
    let source = r#"
    fn main() {
        let y = match Some("hello") {
            _ => "x",
        }
        expect(y == "x")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_binding() {
    let source = r#"
    fn main() {
        let y = match Some("hello") {
            v => v,
        }
        expect(y == Some("hello"))
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_precedence() {
    let source = r#"
    fn main() {
        let y = match Some("hello") {
            Some(v) => "first",
            _ => "second",
        }
        expect(y == "first")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

// ── Literal patterns & Generalized constructors ────────────────────────────

#[test]
fn test_match_runtime_literal_int() {
    let source = r#"
    fn main() {
        let x = 1
        let res = match x {
            0 => "zero",
            1 => "one",
            _ => "many",
        }
        expect(res == "one")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_literal_string() {
    let source = r#"
    fn main() {
        let s = "no"
        let res = match s {
            "yes" => true,
            "no" => false,
            _ => false,
        }
        expect(res == false)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_literal_bool() {
    let source = r#"
    fn main() {
        let b = false
        let res = match b {
            true => 1,
            false => 0,
        }
        expect(res == 0)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_precedence_with_literals() {
    let source = r#"
    fn main() {
        let x = 1
        let res = match x {
            1 => "first",
            i => "second",
        }
        expect(res == "first")

        let x2 = 2
        let res2 = match x2 {
            1 => "first",
            i => "second",
        }
        expect(res2 == "second")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_struct_destructuring() {
    let source = r#"
    struct User { name: String, age: Int }
    fn main() {
        let u = User { name: "Antigravity", age: 1 }
        let res = match u {
            User { name, age } => name,
        }
        expect(res == "Antigravity")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_tuple_destructuring() {
    let source = r#"
    fn main() {
        let t = ("Antigravity", 1)
        let res = match t {
            (name, age) => name,
        }
        expect(res == "Antigravity")
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_tuple_wildcard() {
    let source = r#"
    fn main() {
        let t = ("Antigravity", 1)
        let res = match t {
            (_, age) => age,
        }
        expect(res == 1)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_bool_exact() {
    let source = r#"
    fn main() {
        let b = false
        let x = match b {
            true => 1,
            false => 0,
        }
        expect(x == 0)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}

#[test]
fn test_match_runtime_bool_wildcard() {
    let source = r#"
    fn main() {
        let b = false
        let x = match b {
            true => 1,
            _ => 0,
        }
        expect(x == 0)
        "ok"
    }
    "#;
    assert_eq!(assert_runs(source), Value::String("ok".to_string()));
}
