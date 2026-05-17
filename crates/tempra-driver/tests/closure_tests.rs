use tempra_driver::pipeline::test_source;

fn expect_ok(source: &str) {
    let result = test_source(source);
    assert!(
        result.is_ok(),
        "expected success but got: {:?}",
        result.err()
    );
}

fn expect_err_containing(source: &str, needle: &str) {
    let result = test_source(source);
    assert!(result.is_err(), "expected compile error for: {}", needle);
    let err = format!("{:?}", result.err().unwrap());
    assert!(
        err.to_lowercase().contains(&needle.to_lowercase()),
        "expected error containing {:?} but got:\n{}",
        needle,
        err
    );
}

#[test]
fn test_no_capture_closure() {
    let input = r#"
        fn main() {
            let add = fn(x: Int) -> Int {
                x + 1
            }
            expect(add(10) == 11, "no-capture closure failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_explicit_capture_ref() {
    let input = r#"
        fn main() {
            let base = 10;
            let add_base = fn[ref base](x: Int) -> Int {
                base + x
            }
            expect(add_base(5) == 15, "ref capture failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_explicit_capture_inout() {
    let input = r#"
        fn main() {
            var count = 0;
            let inc = fn[inout count]() -> Int {
                count = count + 1
                count
            }
            expect(inc() == 1, "inout capture first call failed");
            expect(inc() == 2, "inout capture second call failed");
            expect(count == 2, "outer variable not updated by inout capture");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_explicit_capture_consume() {
    let input = r#"
        struct Box { value: Int }
        fn main() {
            let b = Box { value: 42 };
            let get_val = fn[consume b]() -> Int {
                b.value
            }
            expect(get_val() == 42, "consume capture failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_closure_type_annotation() {
    let input = r#"
        fn main() {
            let f: fn(Int) -> Int = fn(x: Int) -> Int { x * 2 };
            expect(f(21) == 42, "closure with type annotation failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_implicit_capture_fails() {
    let input = r#"
        fn main() {
            let x = 10;
            let f = fn() -> Int { x };
        }
    "#;
    expect_err_containing(input, "use of external value 'x' requires explicit capture");
}

#[test]
fn test_empty_capture_list_fails() {
    let input = r#"
        fn main() {
            let f = fn[]() { 1 };
        }
    "#;
    expect_err_containing(
        input,
        "empty capture list is not allowed; use `fn()` for no captures",
    );
}

#[test]
fn test_closure_in_struct() {
    let input = r#"
        struct Ops {
            callback: fn(Int) -> Int
        }
        fn main() {
            let o = Ops {
                callback: fn(x: Int) -> Int { x * x }
            };
            expect(o.callback(4) == 16, "closure in struct field failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_higher_order_function() {
    let input = r#"
        fn apply(f: fn(Int) -> Int, x: Int) -> Int {
            f(x)
        }
        fn main() {
            let res = apply(fn(x: Int) -> Int { x + 10 }, 5);
            expect(res == 15, "higher order function failed");
        }
    "#;
    expect_ok(input);
}
#[test]
fn test_ref_assignment_rejected() {
    let input = r#"
        fn main() {
            let x = 1;
            let f = fn[ref x]() {
                x = 2
            }
        }
    "#;
    expect_err_containing(input, "cannot assign to ref capture 'x'");
}

#[test]
fn test_inout_immutable_rejected() {
    let input = r#"
        fn main() {
            let x = 0;
            let f = fn[inout x]() { }
        }
    "#;
    expect_err_containing(input, "cannot capture immutable value 'x' as inout");
}

#[test]
fn test_consume_invalidation() {
    let input = r#"
        fn main() {
            let x = 10;
            let f = fn[consume x]() { x };
            let y = x;
        }
    "#;
    expect_err_containing(input, "use of consumed value 'x'");
}

#[test]
fn test_duplicate_capture_rejected() {
    let input = r#"
        fn main() {
            let x = 1;
            let f = fn[ref x, ref x]() { }
        }
    "#;
    expect_err_containing(input, "duplicate capture: 'x'");
}

#[test]
fn test_unknown_capture_rejected() {
    let input = r#"
        fn main() {
            let f = fn[ref missing]() { }
        }
    "#;
    expect_err_containing(input, "cannot capture unknown value 'missing'");
}

#[test]
fn test_parameter_shadows_capture_rejected() {
    let input = r#"
        fn main() {
            let x = 1;
            let f = fn[ref x](x: Int) { }
        }
    "#;
    expect_err_containing(input, "cannot shadow captured value 'x'");
}

#[test]
fn test_local_shadows_capture_rejected() {
    let input = r#"
        fn main() {
            let x = 1;
            let f = fn[ref x]() {
                let x = 2;
            }
        }
    "#;
    expect_err_containing(input, "cannot shadow captured value 'x'");
}

#[test]
fn test_closure_equality_rejected() {
    let input = r#"
        fn main() {
            let a = fn() { };
            let b = fn() { };
            let eq = a == b;
        }
    "#;
    expect_err_containing(input, "closures are not comparable");
}
#[test]
fn test_recursive_closure_rejected() {
    let input = r#"
        fn main() {
            var fact = fn(n: Int) -> Int { 0 };
            fact = fn(n: Int) -> Int {
                if n == 0 {
                    1
                } else {
                    n * fact(n - 1)
                }
            };
        }
    "#;
    expect_err_containing(input, "requires explicit capture");
}

#[test]
fn test_nested_closure_capture_isolation() {
    let input = r#"
        fn main() {
            let x = 1;
            let outer = fn[ref x]() -> Int {
                let inner = fn() -> Int {
                    x
                };
                inner()
            };
        }
    "#;
    expect_err_containing(input, "requires explicit capture");
}

#[test]
fn test_nested_closure_explicit_capture_success() {
    let input = r#"
        fn main() {
            let x = 42;
            let outer = fn[ref x]() -> Int {
                let inner = fn[ref x]() -> Int {
                    x
                };
                inner()
            };
            expect(outer() == 42, "nested explicit capture failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_consume_then_capture_rejected() {
    let input = r#"
        fn main() {
            let x = 10;
            let a = fn[consume x]() -> Int { x };
            let b = fn[ref x]() -> Int { x };
        }
    "#;
    expect_err_containing(input, "use of consumed value 'x'");
}

#[test]
fn test_recursive_factorial() {
    let input = r#"
        fn main() {
            let fact = fn[self](n: Int) -> Int {
                if n == 0 {
                    1
                } else {
                    n * self(n - 1)
                }
            };
            expect(fact(3) == 6, "recursive factorial failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_recursive_fibonacci() {
    let input = r#"
        fn main() {
            let fib = fn[self](n: Int) -> Int {
                if n < 2 {
                    n
                } else {
                    self(n - 1) + self(n - 2)
                }
            };
            expect(fib(4) == 3, "recursive fibonacci failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_self_shadowing_parameter_rejected() {
    let input = r#"
        fn main() {
            let f = fn[self](self: Int) { };
        }
    "#;
    expect_err_containing(input, "cannot shadow captured value 'self'");
}

#[test]
fn test_self_shadowing_local_rejected() {
    let input = r#"
        fn main() {
            let f = fn[self]() {
                let self = 1;
            };
        }
    "#;
    expect_err_containing(input, "cannot shadow captured value 'self'");
}

#[test]
fn test_self_capture_mode_rejected() {
    let input = r#"
        fn main() {
            let f = fn[ref self]() { };
        }
    "#;
    expect_err_containing(input, "cannot capture unknown value 'self'");
}

#[test]
fn test_nested_closure_isolation_self() {
    let input = r#"
        fn main() {
            let fact = fn[self](n: Int) -> Int {
                let inner = fn() -> Int {
                    self(n - 1)
                };
                inner()
            }
        }
    "#;
    expect_err_containing(input, "requires explicit capture");
}

#[test]
fn test_nested_explicit_self_propagation() {
    let input = r#"
        fn main() {
            let outer = fn[self](n: Int) -> Int {
                if n == 0 { return 1; }
                let inner = fn[ref self](m: Int) -> Int {
                    self(m - 1)
                };
                n * inner(n)
            };
            expect(outer(3) == 6, "nested explicit self propagation failed");
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_self_assignment_rejected() {
    let input = r#"
        fn main() {
            let f = fn[self]() {
                self = fn() { };
            };
        }
    "#;
    expect_err_containing(input, "cannot assign to ref capture 'self'");
}

#[test]
fn test_self_recursion_typing_stability() {
    let input = r#"
        fn main() {
            let f = fn[self]() -> Int {
                self()
            };
        }
    "#;
    expect_ok(input);
}

#[test]
fn test_recursive_self_minimal() {
    let input = r#"
        fn main() {
            let f = fn[self](n: Int) -> Int {
                if n == 0 { 0 } else { self(n - 1) }
            };

            expect(f(3) == 0, "minimal recursion failed");
        }
    "#;

    expect_ok(input);
}
