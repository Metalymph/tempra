use tempra_driver::pipeline::{check_file, emit_tir_source, test_source};

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
fn test_comptime_arithmetic_is_resolved_before_tir() {
    let source = r#"
    fn main() -> Int64 {
        return comptime 1 + 2
    }
    "#;

    let tir = emit_tir_source(source).unwrap();

    assert!(tir.contains("const 3i64"), "unexpected TIR:\n{tir}");
    assert!(!tir.contains("comptime"), "unexpected TIR:\n{tir}");
}

#[test]
fn comptime_arithmetic_failure_is_controlled_and_does_not_poison_later_compilation() {
    let invalid = r#"
    fn main() -> Int64 {
        return comptime 1 / 0
    }
    "#;
    let error = emit_tir_source(invalid).expect_err("division by zero must fail");
    assert!(
        error.to_string().contains("integer division by zero"),
        "unexpected error: {error}"
    );

    let valid = r#"
    fn main() -> Int64 {
        return comptime 40 + 2
    }
    "#;
    let tir = emit_tir_source(valid).expect("later compilation must remain usable");
    assert!(tir.contains("const 42i64"), "unexpected TIR:\n{tir}");
    assert!(!tir.contains("comptime"), "unexpected TIR:\n{tir}");
}

#[test]
fn comptime_bodyless_extern_fails_before_tir_and_does_not_poison_later_compilation() {
    let invalid = r#"
    pub extern fn missing() -> Unit
    fn main() {
        comptime missing()
    }
    "#;
    let error = emit_tir_source(invalid).expect_err("bodyless comptime call must fail");
    assert!(
        error
            .to_string()
            .contains("function 'missing' has no executable HIR body"),
        "unexpected error: {error}"
    );

    let valid = r#"
    fn main() -> Int64 {
        return comptime 1 + 2
    }
    "#;
    let tir = emit_tir_source(valid).expect("later compilation must remain usable");
    assert!(tir.contains("const 3i64"), "unexpected TIR:\n{tir}");
    assert!(!tir.contains("comptime"), "unexpected TIR:\n{tir}");
}

#[test]
fn comptime_effect_policy_is_enforced_transitively_by_the_interpreter() {
    let source = r#"
    fn inner() { dbg("hidden") }
    fn outer() { inner() }
    fn main() { comptime outer() }
    "#;

    let error = emit_tir_source(source).expect_err("transitive dbg must fail");
    assert!(
        error
            .to_string()
            .contains("operation 'dbg' is not allowed during comptime evaluation"),
        "unexpected error: {error}"
    );
}

#[test]
fn file_check_uses_canonical_comptime_authority() {
    let directory = tempfile::tempdir().unwrap();
    let helper = directory.path().join("helper.tmpr");
    let main = directory.path().join("main.tmpr");
    std::fs::write(
        helper,
        r#"
fn local_value() -> Int64 { return 40 }
pub fn value() -> Int64 { return local_value() }
"#,
    )
    .unwrap();
    std::fs::write(
        &main,
        r#"
import helper
fn main() -> Int64 { return comptime helper.value() + 2 }
"#,
    )
    .unwrap();

    let result = check_file(&main);

    assert!(result.is_ok(), "unexpected file-check result: {result:?}");
}

#[test]
fn multi_module_vm_resolves_comptime_before_tir() {
    let directory = tempfile::tempdir().unwrap();
    let helper = directory.path().join("helper.tmpr");
    let main = directory.path().join("main.tmpr");
    std::fs::write(helper, "pub fn value() -> Int64 { return 40 }").unwrap();
    std::fs::write(
        &main,
        r#"
import helper
fn main() -> Int64 { return comptime helper.value() + 2 }
"#,
    )
    .unwrap();

    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap();
    let tir = tempra_tir::pretty::format_tir_module(compilation.tir());

    assert!(tir.contains("fn main.main"), "unexpected TIR:\n{tir}");
    assert!(tir.contains("const 42i64"), "unexpected TIR:\n{tir}");
    assert!(!tir.contains("comptime"), "unexpected TIR:\n{tir}");
}

#[test]
fn duplicate_module_function_names_keep_their_qualified_identity() {
    let directory = tempfile::tempdir().unwrap();
    let main = directory.path().join("main.tmpr");
    std::fs::write(
        directory.path().join("a.tmpr"),
        "pub fn helper() -> Int64 { return 4 }",
    )
    .unwrap();
    std::fs::write(
        directory.path().join("b.tmpr"),
        "pub fn helper() -> Int64 { return 2 }",
    )
    .unwrap();
    std::fs::write(
        &main,
        r#"
import a
import b
fn main() -> Int64 { return comptime a.helper() * 10 + b.helper() }
"#,
    )
    .unwrap();

    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap();
    let tir = tempra_tir::pretty::format_tir_module(compilation.tir());

    assert!(tir.contains("const 42i64"), "unexpected TIR:\n{tir}");
}

#[test]
fn non_entry_module_comptime_is_materialized_independently() {
    let directory = tempfile::tempdir().unwrap();
    let dependency = directory.path().join("dependency.tmpr");
    let main = directory.path().join("main.tmpr");
    std::fs::write(
        dependency,
        "pub fn value() -> Int64 { return comptime 20 + 1 }",
    )
    .unwrap();
    std::fs::write(
        &main,
        r#"
import dependency
fn main() -> Int64 { return dependency.value() + 21 }
"#,
    )
    .unwrap();

    let compilation = tempra_driver::compile_file_to_bytecode_with_analysis(&main).unwrap();
    let tir = tempra_tir::pretty::format_tir_module(compilation.tir());

    let dependency_tir = tir
        .split("fn dependency.value")
        .nth(1)
        .expect("dependency function missing from TIR");
    assert!(
        dependency_tir.contains("const 21i64"),
        "unexpected TIR:\n{tir}"
    );
    assert!(!tir.contains("comptime"), "unexpected TIR:\n{tir}");
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
    assert!(err.contains("bad"));
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

#[test]
fn test_comptime_if_true_branch() {
    let source = r#"
    fn main() {
        let x = if comptime 1 + 1 == 2 {
            10
        } else {
            compile_error("wrong branch")
        }
        expect(x == 10)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_false_branch() {
    let source = r#"
    fn main() {
        let x = if comptime 1 + 1 == 3 {
            compile_error("wrong branch")
        } else {
            20
        }
        expect(x == 20)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_selected_branch_typechecked() {
    let source = r#"
    fn main() {
        let x = if comptime true {
            1
        } else {
            "not an int"
        }
        expect(x == 1)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_unselected_branch_compile_error() {
    let source = r#"
    fn main() {
        let x = if comptime true {
            1
        } else {
            compile_error("must not run")
        }
        expect(x == 1)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_selected_branch_compile_error_fails() {
    let source = r#"
    fn main() {
        let x = if comptime false {
            1
        } else {
            compile_error("selected failure")
        }
    }
    "#;
    let res = test_source(source);
    let err = format!("{:?}", res.err().unwrap());
    assert!(
        err.contains("selected failure"),
        "Actual error was: {}",
        err
    );
}

#[test]
fn test_comptime_if_type_directed_default() {
    let source = r#"
    fn default_value(type T) -> T {
        if comptime T == Int {
            0
        } else if comptime T == Bool {
            false
        } else {
            compile_error("unsupported type")
        }
    }
    fn main() {
        expect(default_value[Int]() == 0)
        expect(default_value[Bool]() == false)
    }
    "#;
    let _res = test_source(source);
    assert!(test_source(source).is_ok());
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_unsupported_type_fails() {
    let source = r#"
    fn default_value(type T) -> T {
        if comptime T == Int {
            0
        } else {
            compile_error("unsupported type")
        }
    }
    fn main() {
        let unused = default_value[String]();
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("unsupported type"));
}

#[test]
fn test_comptime_if_runtime_value_rejected() {
    let source = r#"
    fn f(x: Int) -> Int {
        if comptime x == 1 {
            1
        } else {
            2
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("cannot be used in comptime"));
}

#[test]
fn test_comptime_if_non_bool_condition_rejected() {
    let source = r#"
    fn main() {
        let x = if comptime 1 {
            1
        } else {
            2
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("comptime if condition must be Bool"));
}

#[test]
fn test_comptime_match_type_int() {
    let source = r#"
    fn default_value(type T) -> T {
        comptime match T {
            Int => 0
            Bool => false
            String => ""
            else => compile_error("unsupported type")
        }
    }
    fn main() {
        expect(default_value[Int]() == 0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_type_bool() {
    let source = r#"
    fn default_value(type T) -> T {
        comptime match T {
            Int => 0
            Bool => false
            String => ""
            else => compile_error("unsupported type")
        }
    }
    fn main() {
        expect(default_value[Bool]() == false)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_type_string() {
    let source = r#"
    fn default_value(type T) -> T {
        comptime match T {
            Int => 0
            Bool => false
            String => ""
            else => compile_error("unsupported type")
        }
    }
    fn main() {
        expect(default_value[String]() == "")
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_unsupported_type_fails() {
    let source = r#"
    fn default_value(type T) -> T {
        comptime match T {
            Int => 0
            else => compile_error("unsupported type")
        }
    }
    fn main() {
        let unused = default_value[String]();
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("unsupported type"));
}

#[test]
fn test_comptime_match_non_selected_not_typechecked() {
    let source = r#"
    fn main() {
        let x = comptime match 1 {
            1 => 10
            2 => "wrong type but not selected"
            else => compile_error("not selected")
        }
        expect(x == 10)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_non_selected_compile_error_ignored() {
    let source = r#"
    fn main() {
        let x = comptime match 1 {
            1 => 10
            else => compile_error("must not run")
        }
        expect(x == 10)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_selected_compile_error_fails() {
    let source = r#"
    fn main() {
        let x = comptime match 2 {
            1 => 10
            else => compile_error("selected failure")
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("selected failure"));
}

#[test]
fn test_comptime_match_literal_int() {
    let source = r#"
    fn pick(comptime n: Int) -> String {
        comptime match n {
            0 => "zero"
            1 => "one"
            else => "many"
        }
    }
    fn main() {
        expect(pick(0) == "zero")
        expect(pick(1) == "one")
        expect(pick(2) == "many")
    }
    "#;
    let res = test_source(source);
    assert!(res.is_ok());
}

#[test]
fn test_comptime_match_literal_bool() {
    let source = r#"
    fn pick(comptime b: Bool) -> Int {
        comptime match b {
            true => 1
            false => 0
        }
    }
    fn main() {
        expect(pick(true) == 1)
        expect(pick(false) == 0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_literal_string() {
    let source = r#"
    fn mode(comptime name: String) -> Int {
        comptime match name {
            "debug" => 1
            "release" => 2
            else => 0
        }
    }
    fn main() {
        expect(mode("debug") == 1)
        expect(mode("release") == 2)
        expect(mode("other") == 0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_runtime_scrutinee_rejected() {
    let source = r#"
    fn f(x: Int) -> Int {
        comptime match x {
            1 => 1
            else => 0
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("cannot be used in comptime"));
}

#[test]
fn test_comptime_match_non_exhaustive() {
    let source = r#"
    fn main() {
        let x = comptime match 2 {
            1 => 10
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("no matching arm in comptime match"));
}

#[test]
fn test_comptime_match_unsupported_pattern_rejected() {
    let source = r#"
    fn main() {
        let x = comptime match 1 {
            Some(v) => 1
            else => 0
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    let err = format!("{:?}", res.err().unwrap());
    assert!(err.contains("unsupported pattern in comptime match"));
}

#[test]
fn test_comptime_if_unselected_branch_invalid_code_ignored() {
    let source = r#"
    fn main() {
        if comptime true {
            let x = 1;
        } else {
            let x = 1 + "string"; // Type error
            non_existent_function(); // Name error
        }
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_unselected_arm_invalid_code_ignored() {
    let source = r#"
    fn main() {
        comptime match 1 {
            1 => { let x = 1; }
            2 => { let x = 1 + "string"; } // Type error
            else => { non_existent_function(); } // Name error
        }
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_if_type_equality_generic() {
    let source = r#"
    fn f(type T) -> T {
        if comptime T == Int {
            0
        } else {
            compile_error("unsupported")
        }
    }
    fn main() {
        let x: Int = f[Int]();
        expect(x == 0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_type_generic_fallback() {
    let source = r#"
    fn f(type T) -> T {
        comptime match T {
            Int => 0
            else => compile_error("unsupported")
        }
    }
    fn main() {
        let x: Int = f[Int]();
        expect(x == 0)
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_local_variable_usage() {
    let source = r#"
    fn main() {
        comptime {
            let x = 1;
            if x == 1 { // inside comptime block, this is already comptime-evaluated
                let y = x + 1;
                expect(y == 2)
            }
        }
    }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_comptime_match_unsupported_pattern_rejected_more() {
    let source = r#"
    fn main() {
        comptime match 1 {
            (1, 2) => 1
            else => 2
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("unsupported pattern in comptime match"));

    let source = r#"
    fn main() {
        comptime match 1 {
            [1, 2] => 1
            else => 2
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    // Note: slice patterns might be rejected by parser or typechecker depending on implementation
}

#[test]
fn test_comptime_match_binding_pattern_rejected() {
    let source = r#"
    fn main() {
        comptime match 1 {
            x => 1
            else => 2
        }
    }
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("unsupported pattern in comptime match"));
}
