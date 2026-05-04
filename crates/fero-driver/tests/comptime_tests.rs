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
#[test]
fn test_comptime_param_basic() {
    let source = r#"
    fn add_n(comptime n: Int, x: Int) -> Int {
        x + n
    }

    fn main() {
        let x = add_n(3, 4);
        expect(x == 7)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_param_rejection_runtime_arg() {
    let source = r#"
    fn add_n(comptime n: Int, x: Int) -> Int {
        x + n
    }

    fn main() {
        let n = 3;
        let x = add_n(n, 4);
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("runtime variable 'n' cannot be used in comptime context"));
}

#[test]
fn test_comptime_expr_as_comptime_arg() {
    let source = r#"
    fn add_n(comptime n: Int, x: Int) -> Int {
        x + n
    }

    fn main() {
        let x = add_n(comptime 1 + 2, 4);
        expect(x == 7)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_multiple_specializations() {
    let source = r#"
    fn multiply_n(comptime n: Int, x: Int) -> Int {
        x * n
    }

    fn main() {
        let a = multiply_n(2, 5);
        let b = multiply_n(3, 5);
        expect(a == 10)
        expect(b == 15)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_param_generic() {
    let source = r#"
    fn f(type T, comptime n: Int, x: T) -> T {
        x
    }

    fn main() {
        let x = f[Int](10, 42);
        expect(x == 42)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_param_immutable() {
    let source = r#"
    fn f(comptime n: Int) {
        n = 10;
    }
    fn main() { f(5) }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("cannot assign to immutable value 'n'"));
}

#[test]
fn test_comptime_param_in_closure_rejected() {
    let source = r#"
    fn main() {
        let f = fn(comptime n: Int) -> Int { n };
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("comptime parameters are only supported on named functions in this phase"));
}

#[test]
fn test_comptime_type_equality() {
    let source = r#"
    fn is_int(type T) -> Bool {
        T == Int
    }

    fn main() {
        expect(is_int[Int]())
        expect(!is_int[String]())
    }
    "#;
    assert!(test_source(source).is_ok());
}

// ── Phase 2+3 Hardening regression tests ──

#[test]
fn test_namespace_shadowing_comptime() {
    // Lexical scope must win over builtin namespace names
    let source = r#"
    fn main() {
        let fs = 123
        expect(fs == 123)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_type_equality_false() {
    let source = r#"
    fn is_string(type T) -> Bool {
        T == String
    }
    fn main() {
        expect(is_string[Int]() == false)
        expect(is_string[String]() == true)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_type_equality_two_generic_params() {
    let source = r#"
    fn same(type A, type B) -> Bool {
        A == B
    }
    fn main() {
        expect(same[Int, Int]())
        expect(!same[Int, String]())
        expect(same[Bool, Bool]())
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_type_not_equal() {
    let source = r#"
    fn different(type A, type B) -> Bool {
        A != B
    }
    fn main() {
        expect(different[Int, String]())
        expect(!different[Int, Int]())
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_normal_values_not_confused_with_types() {
    // Normal values should still compare normally; no coercion with type values
    let source = r#"
    fn main() {
        let a = 42
        let b = 42
        expect(a == b)
        expect(a != 99)
        let s = "hello"
        expect(s == "hello")
        expect(s != "world")
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_param_literal_let_still_rejected() {
    // Even if n is initialized from a literal, it's still a runtime local
    let source = r#"
    fn f(comptime n: Int) -> Int { n }
    fn main() {
        let n = 42
        let x = f(n)
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("runtime variable 'n' cannot be used in comptime context"));
}
