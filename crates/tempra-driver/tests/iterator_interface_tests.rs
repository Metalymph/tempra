use tempra_driver::{check_source, run_source};

fn test_source(source: &str) -> Result<(), String> {
    check_source(source).map_err(|e| format!("{:?}", e))
}

fn test_source_run(source: &str) -> Result<(), String> {
    run_source(source).map_err(|e| format!("{:?}", e))
}

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
            let _res = consume_one[MyIter](it);
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
            // Missing inout self
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
    let res = test_source(source);
    assert!(res.is_err());
}

#[test]
fn test_wrong_return_type_fails() {
    let source = r#"
        struct MyIter {}
        extend MyIter {
            // Returns Int instead of Int?
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

#[test]
fn test_iterator_projection() {
    let source = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }

        fn main() {
            type T = MyIter;
            let t = T.Item;
            assert(t == Int);
        }
    "#;
    // Projection on non-generic T might need T to be bound to Iterator
    // Or we use a generic function:
    let source_gen = r#"
        struct MyIter {}
        extend MyIter {
            fn next(inout self) -> Int? { return None; }
        }
        fn check_it(type T: Iterator, x: T.Item) -> T.Item {
            return x;
        }
        fn main() {
            // If this passes type checking, projection is working
            let dummy = check_it[MyIter](10);
        }
    "#;
    assert!(test_source(source_gen).is_ok());
}

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
