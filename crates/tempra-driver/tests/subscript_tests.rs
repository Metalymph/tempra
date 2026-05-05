use tempra_driver::pipeline::test_source;

#[test]
fn test_array_index_read() {
    let source = r#"
        let xs = [10, 20, 30]
        expect(xs[0] == 10)
        expect(xs[1] == 20)
        expect(xs[2] == 30)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_index_write() {
    let source = r#"
        var xs = [10, 20, 30]
        xs[0] = 100
        xs[2] = 300
        expect(xs[0] == 100)
        expect(xs[1] == 20)
        expect(xs[2] == 300)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_slice_read() {
    let source = r#"
        let xs = [10, 20, 30, 40]
        let s = xs[1..3]
        expect(s[0] == 20)
        expect(s[1] == 30)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_slice_write_success() {
    let source = r#"
        var xs = [10, 20, 30, 40]
        xs[1..3] = [200, 300]
        expect(xs[0] == 10)
        expect(xs[1] == 200)
        expect(xs[2] == 300)
        expect(xs[3] == 40)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_slice_write_length_mismatch() {
    let source = r#"
        var xs = [10, 20, 30, 40]
        xs[1..3] = [200]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("slice assignment length mismatch"));
}

#[test]
fn test_slice_as_view_parent_mutation() {
    let source = r#"
        var xs = [1, 2, 3]
        let ys = xs[0..2]
        xs[0] = 9
        expect(ys[0] == 9)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_as_view_slice_mutation() {
    let source = r#"
        var xs = [1, 2, 3]
        var ys = xs[0..2]
        ys[1] = 8
        expect(xs[1] == 8)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_immutable_index_assign_fails() {
    let source = r#"
        let xs = [1, 2, 3]
        xs[0] = 10
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("cannot mutate immutable value 'xs'"));
}

#[test]
fn test_immutable_slice_assign_fails() {
    let source = r#"
        let xs = [1, 2, 3]
        xs[0..2] = [10, 20]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("cannot mutate immutable value 'xs'"));
}

#[test]
fn test_string_indexing_fails() {
    let source = r#"
        let s = "hello"
        let c = s[0]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("string indexing is intentionally unsupported"));
}

#[test]
fn test_nested_subscript_assignment_fails() {
    let source = r#"
        var xs = [[1, 2], [3, 4]]
        xs[0][0] = 10
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("nested subscript assignment is not supported in v1"));
}

#[test]
fn test_user_get() {
    let source = r#"
        struct Box {
            var value: Int
        }

        extend Box {
            fn get(self, index: Int) -> Int {
                return self.value + index
            }
        }

        let b = Box { value: 100 }
        expect(b[10] == 110)
        expect(b[5] == 105)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_user_set() {
    let source = r#"
        struct Box {
            var value: Int
        }

        extend Box {
            fn set(inout self, index: Int, val: Int) {
                self.value = val + index
            }
        }

        var b = Box { value: 100 }
        b[10] = 50
        expect(b.value == 60)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_user_slice() {
    let source = r#"
        struct Range {
            var base: Int
        }

        extend Range {
            fn slice(self, start: Int, end: Int) -> Int {
                return self.base + start + end
            }
        }

        let r = Range { base: 10 }
        expect(r[1..2] == 13)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_user_set_slice() {
    let source = r#"
        struct Range {
            var base: Int
        }

        extend Range {
            fn set_slice(inout self, start: Int, end: Int, val: Int) {
                self.base = start + end + val
            }
        }

        var r = Range { base: 10 }
        r[1..2] = 100
        expect(r.base == 103)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_missing_hooks_fail() {
    let source = r#"
        struct Empty {}
        let e = Empty {}
        let x = e[0]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("missing 'get' method"));

    let source2 = r#"
        struct Empty {}
        var e = Empty {}
        e[0] = 1
    "#;
    let res2 = test_source(source2);
    assert!(res2.is_err());
    // Phase 7: Assignment looks for 'set'
    assert!(format!("{:?}", res2.err().unwrap()).contains("missing 'set' method"));
}
#[test]
fn test_user_set_wrong_return_fails() {
    let source = r#"
        struct Box { var v: Int }
        extend Box {
            fn set(inout self, index: Int, val: Int) -> Int {
                return val
            }
        }
        var b = Box { v: 10 }
        b[0] = 5
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("'set' hook must return Unit"));
}

#[test]
fn test_user_set_slice_wrong_return_fails() {
    let source = r#"
        struct Box { var v: Int }
        extend Box {
            fn set_slice(inout self, start: Int, end: Int, val: Int) -> Int {
                return val
            }
        }
        var b = Box { v: 10 }
        b[0..1] = 5
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("'set_slice' hook must return Unit"));
}

#[test]
fn test_user_set_wrong_value_type_fails() {
    let source = r#"
        struct Box { var v: Int }
        extend Box {
            fn set(inout self, index: Int, val: Int) {}
        }
        var b = Box { v: 10 }
        b[0] = "string"
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("Cannot assign String to Int"));
}
