use tempra_driver::check_source;

fn test_source(source: &str) -> Result<(), String> {
    check_source(source).map_err(|e| format!("{:?}", e))
}

// ─── Satisfaction Tests ───────────────────────────────────────────────────────

#[test]
fn test_custom_iterator_success() {
    let source = r#"
        struct MyIter {
            var count: Int
        }
        extend MyIter {
            fn next(inout self) -> Int? {
                if self.count > 0 {
                    self.count = self.count - 1;
                    return Some(self.count);
                }
                return None;
            }
        }

        fn consume_one(type T: Iterator, inout it: T) -> T.Item? {
            it.next()
        }

        fn main() {
            var it = MyIter { count: 10 };
            let dummy = consume_one[MyIter](it);
        }
    "#;
    let res = test_source(source);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

#[test]
fn test_missing_inout_fails() {
    let source = r#"
        struct MyIter {
            var count: Int
        }
        extend MyIter {
            // Missing inout self — must fail Iterator satisfaction
            fn next(self) -> Int? {
                return None;
            }
        }

        fn consume_one(type T: Iterator, it: T) {}

        fn main() {
            let it = MyIter { count: 10 };
            consume_one[MyIter](it);
        }
    "#;
    assert!(test_source(source).is_err());
}

#[test]
fn test_wrong_return_type_fails() {
    let source = r#"
        struct MyIter {}
        extend MyIter {
            // Returns Int instead of Int? — must fail
            fn next(inout self) -> Int {
                return 0;
            }
        }

        fn consume_one(type T: Iterator, it: T) {}

        fn main() {
            var it = MyIter {};
            consume_one[MyIter](it);
        }
    "#;
    assert!(test_source(source).is_err());
}

// ─── Projection Type-Position Tests ──────────────────────────────────────────

#[test]
fn test_projection_in_return_type() {
    // T.Item is valid in return type position
    let source = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }
        fn get_item(type T: Iterator, inout it: T) -> T.Item? {
            it.next()
        }
        fn main() {
            var it = MyIter {};
            let dummy = get_item[MyIter](it);
        }
    "#;
    let res = test_source(source);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

#[test]
fn test_projection_in_param_type() {
    // T.Item is valid as a parameter type
    let source = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }
        fn accept_item(type T: Iterator, value: T.Item) -> T.Item {
            return value;
        }
        fn main() {
            let dummy = accept_item[MyIter](42);
        }
    "#;
    let res = test_source(source);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

#[test]
fn test_projection_in_nested_type() {
    // T.Item is valid inside Array(T.Item)
    let source = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }
        fn collect(type T: Iterator) -> Array(T.Item) {
            return [];
        }
        fn main() {
            let dummy: Array(Int) = collect[MyIter]();
        }
    "#;
    let res = test_source(source);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Projection Expression-Position Rejection Tests ──────────────────────────

#[test]
fn test_projection_in_expression_rejected_generic() {
    // let x = T.Item inside a generic function must be rejected
    let source = r#"
        fn check_it(type T: Iterator) {
            let x = T.Item;
        }
        fn main() {}
    "#;
    let res = test_source(source);
    assert!(
        res.is_err(),
        "Expected error for projection in expression position"
    );
}

#[test]
fn test_projection_in_expression_rejected_concrete() {
    // MyIter.Item as expression must be rejected
    let source = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }
        fn main() {
            let x = MyIter.Item;
        }
    "#;
    let res = test_source(source);
    assert!(
        res.is_err(),
        "Expected error for concrete projection in expression position"
    );
}

#[test]
fn test_projection_in_call_arg_rejected() {
    // Projection used as a function argument must be rejected
    let source = r#"
        fn take_it(x: Int) {}
        fn check_it(type T: Iterator) {
            take_it(T.Item);
        }
        fn main() {}
    "#;
    let res = test_source(source);
    assert!(
        res.is_err(),
        "Expected error for projection passed as call argument"
    );
}

// ─── Interface-as-Runtime-Type Rejection Tests ───────────────────────────────

#[test]
fn test_iterator_rejection_as_runtime_type() {
    let source = r#"
        fn main() {
            let x: Iterator = 1;
        }
    "#;
    assert!(test_source(source).is_err());

    let source2 = r#"
        fn f(x: Iterator) {}
    "#;
    assert!(test_source(source2).is_err());

    let source3 = r#"
        fn f() -> Iterator { return 1; }
    "#;
    assert!(test_source(source3).is_err());

    let source4 = r#"
        struct Box {
            value: Iterator
        }
    "#;
    assert!(test_source(source4).is_err());
}
