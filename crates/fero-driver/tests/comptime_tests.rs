use fero_driver::pipeline::test_source;

#[test]
fn test_comptime_arithmetic() {
    let source = r#"
    fn main() {
        let x = comptime 1 + 2;
        expect(x == 3)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_block() {
    let source = r#"
    fn main() {
        let x = comptime {
            let a = 10;
            let b = 20;
            a + b
        };
        expect(x == 30)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_func_call() {
    let source = r#"
    fn add(a: Int, b: Int) -> Int {
        a + b
    }

    fn main() {
        let x = comptime add(1, 2);
        expect(x == 3)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_forbidden_io() {
    let source = r#"
    fn main() {
        let x = comptime fs.read("Cargo.toml");
    }
    "#;
    let res = test_source(source);
    if res.is_ok() {
        panic!("expected error, got Ok");
    }
    let err = format!("{:?}", res.as_ref().err().unwrap());
    assert!(err.contains("not allowed during comptime evaluation"));
}

#[test]
fn test_comptime_runtime_var_rejection() {
    let source = r#"
    fn f(x: Int) {
        let y = comptime x + 1;
    }
    fn main() {
        f(10)
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("runtime variable 'x' cannot be used in comptime context"));
}

#[test]
fn test_comptime_compile_error_success() {
    let source = r#"
    fn main() {
        comptime {
            compile_error("custom error")
        }
    }
    "#;
    let res = test_source(source);
    if res.is_ok() {
        panic!("expected error, got Ok");
    }
    let err = format!("{:?}", res.as_ref().err().unwrap());
    assert!(err.contains("custom error"));
}

#[test]
fn test_runtime_compile_error_rejection() {
    let source = r#"
    fn main() {
        compile_error("bad")
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("compile_error is only allowed in comptime context"));
}

#[test]
fn test_comptime_materialization() {
    let source = r#"
    fn main() {
        let s = comptime "hello " + "world";
        expect(s == "hello world")
        
        let b = comptime 2 > 1;
        expect(b == true)
        
        let f = comptime 1.5 * 2.0;
        expect(f == 3.0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_non_primitive_materialization_rejection() {
    let source = r#"
    struct User { name: String }

    fn main() {
        let u = comptime User { name: "A" };
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("cannot be materialized from comptime in v1"));
}
