use fero_parser::parse_source;
use fero_hir::lower_file;
use fero_typeck::check_file;

fn expect_ok(source: &str) {
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    if let Err(errors) = check_file(&hir) {
        panic!("Type checking failed for source:\n{}\nErrors: {:?}", source, errors);
    }
}

fn expect_err(source: &str) {
    let ast = parse_source(source).unwrap();
    let hir = lower_file(source, &ast);
    if check_file(&hir).is_ok() {
        panic!("Expected type error, but succeeded for source:\n{}", source);
    }
}

#[test]
fn test_literal_typing() {
    expect_ok(r#"
        fn main() {
            let x = 1;
            let y = 1.5;
            let z = "hello";
            let ok = true;
        }
    "#);
}

#[test]
fn test_arithmetic_success() {
    expect_ok(r#"
        fn main() {
            let x = 1 + 2;
        }
    "#);
}

#[test]
fn test_arithmetic_failure() {
    expect_err(r#"
        fn main() {
            let x = "a" + 1;
        }
    "#);
}

#[test]
fn test_if_success() {
    expect_ok(r#"
        fn main() {
            let x = if true { 1 } else { 2 };
        }
    "#);
}

#[test]
fn test_if_failure() {
    expect_err(r#"
        fn main() {
            let x = if true { 1 } else { "a" };
        }
    "#);
}

#[test]
fn test_function_call_success() {
    expect_ok(r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            a + b
        }

        fn main() {
            let x = add(1, 2);
        }
    "#);
}

#[test]
fn test_function_call_arity_failure() {
    expect_err(r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            a + b
        }

        fn main() {
            let x = add(1);
        }
    "#);
}

#[test]
fn test_struct_literal_success() {
    expect_ok(r#"
        struct User {
            name: String,
            bank_value: Int64
        }

        fn main() {
            let u = User {
                name: "Lorenzo",
                bank_value: 50000
            };
        }
    "#);
}

#[test]
fn test_struct_literal_failure() {
    expect_err(r#"
        struct User {
            name: String,
            bank_value: Int64
        }

        fn main() {
            let u = User {
                name: 123,
                bank_value: 50000
            };
        }
    "#);
}

#[test]
fn test_array_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
        }
    "#);
}

#[test]
fn test_array_failure() {
    expect_err(r#"
        fn main() {
            let xs = [1, "a", 3];
        }
    "#);
}

#[test]
fn test_for_statement_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
            for x in xs {
                x;
            }
        }
    "#);
}

#[test]
fn test_for_expression_success() {
    expect_ok(r#"
        fn main() {
            let xs = [1, 2, 3];
            let ys = for x in xs {
                x + 1
            };
        }
    "#);
}
