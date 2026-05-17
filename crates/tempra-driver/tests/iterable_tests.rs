use tempra_driver::{check_source, run_source};

fn ok(source: &str) -> Result<(), String> {
    run_source(source).map_err(|e| format!("{:?}", e))
}

fn typecheck_ok(source: &str) -> Result<(), String> {
    check_source(source).map_err(|e| format!("{:?}", e))
}

fn typecheck_err(source: &str) -> bool {
    check_source(source).is_err()
}

// ─── Built-in Array Iteration ─────────────────────────────────────────────────

#[test]
fn test_for_array_sum() {
    let res = ok(r#"
        fn expect(cond: Bool) {
            if !cond { crash("expectation failed") }
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            var sum = 0;
            for x in xs {
                sum = sum + x;
            }
            expect(sum == 6);
        }
    "#);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

#[test]
fn test_for_array_expression() {
    let res = ok(r#"
        fn expect(cond: Bool) {
            if !cond { crash("expectation failed") }
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            let ys = for x in xs { x + 1 };
            expect(ys[0] == 2);
            expect(ys[2] == 4);
        }
    "#);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Built-in Slice Iteration ─────────────────────────────────────────────────

#[test]
fn test_for_slice_sum() {
    let res = ok(r#"
        fn expect(cond: Bool) {
            if !cond { crash("expectation failed") }
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            let ys: Slice(Int) = xs[1..3];
            var sum = 0;
            for x in ys {
                sum = sum + x;
            }
            expect(sum == 5);
        }
    "#);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Filtered For ─────────────────────────────────────────────────────────────

#[test]
fn test_for_filtered() {
    let res = ok(r#"
        fn expect(cond: Bool) {
            if !cond { crash("expectation failed") }
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3, 4];
            let ys = for x in xs if x > 2 { x };
            expect(ys.len() == 2);
            expect(ys[0] == 3);
            expect(ys[1] == 4);
        }
    "#);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Custom Iterable (Struct) ─────────────────────────────────────────────────

#[test]
fn test_custom_iterable() {
    let res = ok(r#"
        struct CounterIter {
            current: Int
            end: Int
        }
        extend CounterIter {
            fn next(inout self) -> Int? {
                if self.current >= self.end {
                    return None;
                }
                let out = self.current;
                self.current = self.current + 1;
                return Some(out);
            }
        }

        struct CounterRange {
            end: Int
        }
        extend CounterRange {
            fn iter(self) -> CounterIter {
                CounterIter { current: 0, end: self.end }
            }
        }

        fn main() {
            let r = CounterRange { end: 3 };
            var sum = 0;
            for x in r {
                sum = sum + x;
            }
            if sum != 3 { crash("fail") }
        }
    "#);
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Generic Iterable Constraint ─────────────────────────────────────────────

#[test]
fn test_generic_iterable_count() {
    let res = typecheck_ok(
        r#"
        fn count(type T: Iterable, value: T) -> Int {
            var n = 0;
            for dummy in value {
                n = n + 1;
            }
            return n;
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            let dummy = count[Array(Int)](xs);
        }
    "#,
    );
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Projection via Iterable ──────────────────────────────────────────────────

#[test]
fn test_iterable_item_projection_in_return_type() {
    // T.Item is valid as a return type for Iterable-constrained generics
    let res = typecheck_ok(
        r#"
        fn first(type T: Iterable, value: T) -> T.Item? {
            for x in value {
                return Some(x);
            }
            return None;
        }
        fn main() {
            let xs: Array(Int) = [1, 2, 3];
            let dummy = first[Array(Int)](xs);
        }
    "#,
    );
    if let Err(e) = &res {
        println!("FAIL: {}", e);
    }
    assert!(res.is_ok());
}

// ─── Rejection Tests ──────────────────────────────────────────────────────────

#[test]
fn test_non_iterable_struct_rejected() {
    // A plain struct without iter() should fail in a for-in
    assert!(typecheck_err(
        r#"
        struct Foo { x: Int }
        fn main() {
            let f = Foo { x: 1 };
            for item in f { }
        }
    "#
    ));
}

#[test]
fn test_iterable_rejected_as_runtime_type() {
    assert!(typecheck_err(
        r#"
        fn main() { let x: Iterable = 1; }
    "#
    ));
    assert!(typecheck_err(
        r#"
        fn f(x: Iterable) {}
    "#
    ));
    assert!(typecheck_err(
        r#"
        fn f() -> Iterable { return 1; }
    "#
    ));
    assert!(typecheck_err(
        r#"
        struct Box { value: Iterable }
    "#
    ));
}

#[test]
fn test_iterable_projection_in_expression_rejected() {
    // T.Iter and T.Item in expression position must be rejected
    assert!(typecheck_err(
        r#"
        fn check(type T: Iterable) {
            let x = T.Item;
        }
        fn main() {}
    "#
    ));
    assert!(typecheck_err(
        r#"
        fn check(type T: Iterable) {
            let x = T.Iter;
        }
        fn main() {}
    "#
    ));
}
