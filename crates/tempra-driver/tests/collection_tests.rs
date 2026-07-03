use tempra_driver::pipeline::{check_source, test_source};

#[test]
fn test_infer_array_literal() {
    let source = r#"
        let xs = [1, 2, 3]
        expect(xs.len() == 3)
        expect(xs[0] == 1)
        expect(!xs.is_empty())
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_explicit_array_type() {
    let source = r#"
        let xs: Array(Int) = [1, 2, 3]
        expect(xs[1] == 2)
    "#;
    assert!(check_source(source).is_ok());
}

#[test]
fn test_mixed_array_fails() {
    let source = r#"
        let xs = [1, "x"]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap())
        .contains("array literal elements must have the same type"));
}

#[test]
fn test_empty_array_requires_type() {
    let source = r#"
        let xs = []
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap())
        .contains("cannot infer element type for empty array literal"));
}

#[test]
fn test_empty_array_with_type() {
    let source = r#"
        let xs: Array(Int) = []
        expect(xs.len() == 0)
        expect(xs.is_empty())
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_type_annotation() {
    let source = r#"
        let xs: Array(Int) = [1, 2, 3]
        let ys: Slice(Int) = xs[0..2]
        expect(ys.len() == 2)
        expect(ys[1] == 2)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_assignment_array() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3]
        xs[1..3] = [10, 20]
        expect(xs[1] == 10)
        expect(xs[2] == 20)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_assignment_slice() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3, 4]
        let ys: Slice(Int) = xs[0..2]
        xs[2..4] = ys
        expect(xs[2] == 1)
        expect(xs[3] == 2)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_assignment_mismatch() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3]
        xs[0..2] = [9]
    "#;
    let res = test_source(source);
    // This fails at RUNTIME (Crash)
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("slice assignment length mismatch"));
}

#[test]
fn test_array_element_assignment() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3]
        xs[0] = 9
        expect(xs[0] == 9)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_immutable_array_assignment_fails() {
    let source = r#"
        let xs: Array(Int) = [1, 2, 3]
        xs[0] = 9
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("cannot mutate immutable value 'xs'"));
}

#[test]
fn test_string_indexing_still_fails() {
    let source = r#"
        let s = "abc"
        let x = s[0]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap())
        .contains("string indexing is intentionally unsupported"));
}

#[test]
fn test_never_unification_failure() {
    let source = r#"
        let xs = [crash("error")]
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap())
        .contains("cannot infer element type for array literal with only Never elements"));
}

#[test]
fn test_never_unification_success() {
    let source = r#"
        let xs: Array(Int) = [crash("error")]
    "#;
    // Typechecks successfully even if it will crash at runtime
    // I'll check the error message.
    let err_msg = format!("{:?}", test_source(source).err().unwrap());
    assert!(!err_msg.contains("TypeError"));
}

#[test]
fn test_mixed_never_unification() {
    let source = r#"
        let xs = [1, crash("error")]
        expect(xs.len() == 2)
        expect(xs[0] == 1)
    "#;
    // Should be inferred as Array(Int)
    let err_msg = format!("{:?}", test_source(source).err().unwrap());
    assert!(!err_msg.contains("TypeError"));
}

#[test]
fn test_slice_view_read() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3]
        let ys: Slice(Int) = xs[0..2]
        xs[0] = 9
        expect(ys[0] == 9)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_slice_view_write() {
    let source = r#"
        var xs: Array(Int) = [1, 2, 3]
        var ys: Slice(Int) = xs[0..2]
        ys[0] = 8
        expect(xs[0] == 8)
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_first_non_empty() {
    let source = r#"
        let xs = [10, 20, 30]
        match xs.first() {
            Some(v) => expect(v == 10)
            None => crash("expected Some")
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_last_non_empty() {
    let source = r#"
        let xs = [10, 20, 30]
        match xs.last() {
            Some(v) => expect(v == 30)
            None => crash("expected Some")
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_first_empty() {
    let source = r#"
        let xs: Array(Int) = []
        match xs.first() {
            Some(_) => crash("expected None")
            None => {}
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_array_last_empty() {
    let source = r#"
        let xs: Array(Int) = []
        match xs.last() {
            Some(_) => crash("expected None")
            None => {}
        }
    "#;
    assert!(test_source(source).is_ok());
}

#[test]
fn test_functional_collection_helpers_typecheck() {
    let source = r#"
        fn double(x: Int) -> Int { x * 2 }
        fn keep(x: Int) -> Bool { x > 1 }
        fn sink(x: Int) {}

        let xs: Array(Int) = [1, 2, 3]
        let sl: Slice(Int) = xs[0..3]

        let mapped: Array(Int) = xs.map(double)
        let filtered: Array(Int) = xs.filter(keep)
        let found: Option(Int) = xs.find(keep)
        let any_match: Bool = xs.any(keep)
        let all_match: Bool = xs.all(keep)
        let count: Int = xs.count()
        xs.for_each(sink)

        let slice_mapped: Array(Int) = sl.map(double)
        let slice_filtered: Array(Int) = sl.filter(keep)
        let slice_found: Option(Int) = sl.find(keep)
        let slice_any: Bool = sl.any(keep)
        let slice_all: Bool = sl.all(keep)
        let slice_count: Int = sl.count()
        sl.for_each(sink)
    "#;
    assert!(check_source(source).is_ok());
}

#[test]
fn test_functional_collection_helpers_reject_inline_closure() {
    let source = r#"
        let xs: Array(Int) = [1, 2, 3]
        let mapped = xs.map(fn(x: Int) -> Int { x * 2 })
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap())
        .contains("expects a direct top-level function reference"));
}

#[test]
fn test_collection_reductions_typecheck_success() {
    let source = r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        fn choose(type T, a: T, b: T) -> T { a }

        let xs: Array(Int) = [1, 2, 3]
        let sl: Slice(Int) = xs[0..3]

        let reduced: Option(Int) = xs.reduce(add)
        let folded: Int = xs.fold(0, add)
        let slice_reduced: Option(Int) = sl.reduce(add)
        let slice_folded: Int = sl.fold(0, add)

        let generic_reduced: Option(Int) = xs.reduce(choose)
        let generic_folded: Int = xs.fold(0, choose)
    "#;
    assert!(check_source(source).is_ok());
}

#[test]
fn test_collection_reductions_string_concat_typecheck() {
    let source = r#"
        fn concat(a: String, b: String) -> String { a + b }
        let xs: Array(String) = ["a", "b", "c"]
        let reduced: Option(String) = xs.reduce(concat)
        let folded: String = xs.fold("", concat)
    "#;
    assert!(check_source(source).is_ok());
}

#[test]
fn test_reduce_rejects_wrong_callback_arity() {
    let source = r#"
        fn bad(a: Int) -> Int { a }
        let xs: Array(Int) = [1, 2, 3]
        let value = xs.reduce(bad)
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("must take exactly 2 arguments"));
}

#[test]
fn test_reduce_rejects_wrong_callback_return_type() {
    let source = r#"
        fn bad(a: Int, b: Int) -> Bool { true }
        let xs: Array(Int) = [1, 2, 3]
        let value = xs.reduce(bad)
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("return type mismatch"));
}

#[test]
fn test_fold_rejects_wrong_accumulator_type() {
    let source = r#"
        fn add(a: Int, b: Int) -> Int { a + b }
        let xs: Array(Int) = [1, 2, 3]
        let value = xs.fold("", add)
    "#;
    let res = test_source(source);
    assert!(res.is_err());
    assert!(format!("{:?}", res.err().unwrap()).contains("parameter type mismatch"));
}
