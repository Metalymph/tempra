use tempra_hir::lower_file;
use tempra_parser::parse_source;
use tempra_typeck::check_file;

fn expect_ok(source: &str) {
    let ast = parse_source(source).expect("Failed to parse source");
    let mut hir = lower_file(source, &ast);
    if let Err(errors) = check_file(&mut hir) {
        panic!(
            "Type checking failed for source:\n{}\nErrors: {:?}",
            source, errors
        );
    }
}

fn expect_err(source: &str) {
    let ast = match parse_source(source) {
        Ok(ast) => ast,
        Err(_) => return, // Parse error is also a form of error
    };
    let mut hir = lower_file(source, &ast);
    if let Err(errs) = check_file(&mut hir) {
        if errs.is_empty() {
            panic!(
                "check_file returned Err but with no errors? for source:\n{}",
                source
            );
        }
        return;
    }
    panic!("Expected error, but succeeded for source:\n{}", source);
}

// --- PART 2: Generic Parameter Identity ---

#[test]
fn test_identity_same_name_different_functions() {
    expect_ok(
        r#"
        fn id_int(type T, x: T) -> T { return x }
        fn id_string(type T, x: T) -> T { return x }
        fn main() {
            let a = id_int(1);
            let b = id_string("x");
        }
    "#,
    );
}

#[test]
fn test_identity_nested_calls_same_name() {
    expect_ok(
        r#"
        fn outer(type T, x: T) -> T { return x }
        fn inner(type T, x: T) -> T { return x }
        fn main() {
            let x = outer(inner(1));
        }
    "#,
    );
}

// --- PART 3: Explicit Specialization ---

#[test]
fn test_explicit_specialization_simple() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[Int](1);
        }
    "#,
    );
}

#[test]
fn test_explicit_specialization_tuple() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[(Int, String)]((1, "a"));
        }
    "#,
    );
}

#[test]
fn test_explicit_specialization_option() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[Int?](Some(1));
        }
    "#,
    );
}

#[test]
fn test_explicit_specialization_mismatch() {
    expect_err(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[String](42);
        }
    "#,
    );
}

#[test]
fn test_explicit_specialization_arity_mismatch() {
    expect_err(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[Int, String](1);
        }
    "#,
    );
}

#[test]
fn test_explicit_specialization_empty_args() {
    // Should fail because it's not a valid specialization call
    expect_err(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[](1);
        }
    "#,
    );
}

// --- PART 4: Inference ---

#[test]
fn test_inference_single_arg() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity(1);
        }
    "#,
    );
}

#[test]
fn test_inference_multiple_args_success() {
    expect_ok(
        r#"
        fn choose(type T, a: T, b: T) -> T { return a }
        fn main() {
            let x = choose(1, 2);
        }
    "#,
    );
}

#[test]
fn test_inference_conflict() {
    expect_err(
        r#"
        fn choose(type T, a: T, b: T) -> T { return a }
        fn main() {
            let x = choose(1, "x");
        }
    "#,
    );
}

#[test]
fn test_inference_no_implicit_coercion() {
    expect_err(
        r#"
        fn choose(type T, a: T, b: T) -> T { return a }
        fn main() {
            let x = choose(1, 2.0);
        }
    "#,
    );
}

#[test]
fn test_inference_return_only_fails() {
    expect_err(
        r#"
        fn make(type T) -> T { crash("err") }
        fn main() {
            let x = make();
        }
    "#,
    );
}

#[test]
fn test_inference_return_only_explicit_success() {
    expect_ok(
        r#"
        fn make(type T) -> T { crash("ok") }
        fn main() {
            let x = make[Int]();
        }
    "#,
    );
}

// --- PART 5: Recursive Substitution ---

#[test]
fn test_recursive_sub_tuple() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[(Int, String)]((1, "a"));
            let y = match x {
                (a, b) => a,
                _ => crash("unreachable"),
            };
        }
    "#,
    );
}

#[test]
fn test_recursive_sub_nested_tuple() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[((Int, String), Bool)](((1, "a"), true));
            let y = match x {
                ((a, b), c) => a,
                _ => crash("unreachable"),
            };
        }
    "#,
    );
}

#[test]
fn test_recursive_sub_option() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[Int?](Some(1));
        }
    "#,
    );
}

#[test]
fn test_recursive_sub_result() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let x = identity[Int!](Ok(1));
        }
    "#,
    );
}

// --- PART 6: Nested Generic Calls ---

#[test]
fn test_nested_inference() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn wrap(type T, x: T) -> T { return x }
        fn main() {
            let x = wrap(identity(1));
        }
    "#,
    );
}

#[test]
fn test_nested_explicit_outer_inferred_inner() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn wrap(type T, x: T) -> T { return x }
        fn main() {
            let x = wrap[Int](identity(1));
        }
    "#,
    );
}

#[test]
fn test_nested_explicit_inner_inferred_outer() {
    expect_ok(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn wrap(type T, x: T) -> T { return x }
        fn main() {
            let x = wrap(identity[Int](1));
        }
    "#,
    );
}

// --- PART 7: Interaction with Never ---

#[test]
fn test_never_return_t() {
    expect_ok(
        r#"
        fn fail_as(type T) -> T { crash("fail") }
        fn main() {
            let x = fail_as[Int]();
        }
    "#,
    );
}

#[test]
fn test_never_match_arm() {
    expect_ok(
        r#"
        fn unwrap_or_fail(type T, value: T?) -> T {
            match value {
                Some(v) => v,
                None => crash("missing"),
            }
        }
        fn main() {
            let x = unwrap_or_fail[Int](Some(1));
        }
    "#,
    );
}

// --- PART 8: Interaction with is / narrowing ---

#[test]
fn test_narrowing_option() {
    expect_ok(
        r#"
        fn has_value(type T, value: T?) -> Bool {
            if value is Some(v) {
                return true
            }
            return false
        }
        fn main() {
            let res = has_value[Int](Some(1));
        }
    "#,
    );
}

#[test]
fn test_narrowing_result() {
    expect_ok(
        r#"
        fn is_ok(type T, value: T!) -> Bool {
            if value is Ok(v) {
                return true
            }
            return false
        }
        fn main() {
            let res = is_ok[Int](Ok(1));
        }
    "#,
    );
}

// --- PART 9: Parser Ambiguity ---

#[test]
fn test_parser_specialization_without_call_fails() {
    expect_err(
        r#"
        fn identity(type T, x: T) -> T { return x }
        fn main() {
            let f = identity[Int];
        }
    "#,
    );
}

#[test]
fn test_parser_indexing_still_works() {
    expect_ok(
        r#"
        fn main() {
            let xs = [1, 2, 3];
            let x = xs[0];
        }
    "#,
    );
}

// --- PART 10: Non-generic regression ---

#[test]
fn test_regression_plain_function() {
    expect_ok(
        r#"
        fn add(a: Int, b: Int) -> Int { return a + b }
        fn main() {
            let res = add(1, 2);
        }
    "#,
    );
}
