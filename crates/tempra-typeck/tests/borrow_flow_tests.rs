//! Memory & Ownership v0.4 §14 / §16 regression cases.
//! Negative cases must parse successfully and fail specifically during type checking.

use tempra_hir::lower_file;
use tempra_parser::parse_source;
use tempra_typeck::{check_file, check_file_with_context};

fn check(source: &str) -> Result<(), String> {
    let ast = parse_source(source).expect("borrow test must parse successfully");
    let mut file = lower_file(source, &ast);
    check_file(&mut file).map_err(|errors| {
        errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn accepted(source: &str) {
    if let Err(err) = check(source) {
        panic!("expected accepted borrow flow, got: {err}\n{source}");
    }
}

fn rejected(source: &str, expected: &str) {
    let err = check(source).expect_err("invalid borrow flow must be rejected");
    assert!(err.contains(expected), "expected '{expected}', got '{err}'");
}

#[test]
fn borrow_nll_after_last_use_ok() {
    accepted(
        r#"
        fn main() {
            var data = [1, 2, 3]
            let view = data[0..2]
            let first = view[0]
            data[0] = 9
        }
    "#,
    );
}

#[test]
fn borrow_nll_before_last_use_err() {
    rejected(
        r#"
        fn main() {
            var data = [1, 2, 3]
            let view = data[0..2]
            data[0] = 9
            let first = view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrow_branch_separate_ok() {
    accepted(
        r#"
        fn main(flag: Bool) {
            var data = [1, 2, 3]
            if flag {
                let view = data[0..2]
                let x = view[0]
            } else {
                data[0] = 9
            }
        }
    "#,
    );
}

#[test]
fn borrow_join_same_owner_ok() {
    accepted(
        r#"
        fn main(flag: Bool) {
            var data = [1, 2, 3]
            let view = if flag { data[0..1] } else { data[1..2] }
            let first = view[0]
            data[0] = 8
        }
    "#,
    );
}

#[test]
fn borrow_join_different_owner_err() {
    rejected(
        r#"
        fn main(flag: Bool) {
            var first = [1, 2]
            var second = [3, 4]
            let view = if flag { first[0..1] } else { second[0..1] }
            let value = view[0]
        }
    "#,
        "borrow provenance conflict",
    );
}

#[test]
fn borrow_loop_iteration_ok() {
    accepted(
        r#"
        fn main() {
            var data = [1, 2, 3]
            for i in [0, 1, 2] {
                let view = data[i..i+1]
                let x = view[0]
                data[i] = 0
            }
        }
    "#,
    );
}

#[test]
fn borrow_loop_carried_err() {
    rejected(
        r#"
        fn main() {
            var data = [1, 2, 3]
            let view = data[0..2]
            for i in [0, 1, 2] {
                data[i] = 0
                let x = view[0]
            }
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrow_return_local_err() {
    rejected(
        r#"
        fn bad() -> Slice(Int) {
            let data = [1, 2, 3]
            data[0..2]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn borrow_closure_ref_nll_ok() {
    accepted(
        r#"
        fn main() {
            var x = 10
            let get = fn[ref x]() -> Int { x }
            let first = get()
            x = 20
        }
    "#,
    );
}

#[test]
fn borrow_closure_ref_mutate_err() {
    rejected(
        r#"
        fn main() {
            var x = 10
            let get = fn[ref x]() -> Int { x }
            x = 20
            let first = get()
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrow_closure_ref_escape_err() {
    rejected(
        r#"
        fn bad() -> fn() -> Int {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            get
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn borrow_shadowed_owner_not_confused() {
    accepted(
        r#"
        fn main(flag: Bool) {
            var data = [1, 2, 3]
            let view = data[0..2]
            if flag {
                var data = [4, 5, 6]
                data[0] = 9
            }
            let x = view[0]
        }
    "#,
    );
}

#[test]
fn borrow_diagnostics_are_deterministic() {
    let source = r#"
        fn main() {
            var data = [1, 2, 3]
            let view = data[0..2]
            data[0] = 5
            let x = view[0]
        }
    "#;
    assert_eq!(check(source), check(source));
}

#[test]
fn borrow_facts_expose_declaration_identity() {
    let source = r#"
        fn main() {
            let data = [1, 2, 3]
            let view = data[0..2]
            let x = view[0]
        }
    "#;
    let ast = parse_source(source).unwrap();
    let mut file = lower_file(source, &ast);
    let ctx = check_file_with_context(&mut file).unwrap();
    assert!(!ctx.facts.borrow_facts.is_empty());
    assert_ne!(
        ctx.facts.borrow_facts[0].borrower,
        ctx.facts.borrow_facts[0].owner
    );
}

#[test]
fn borrow_callback_without_nonescape_contract_err() {
    rejected(
        r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let value = invoke(get)
        }
    "#,
        "borrow callback escape",
    );
}

#[test]
fn borrow_inout_alias_named_arguments_err() {
    rejected(
        r#"
        fn update(inout first: Slice(Int), second: Slice(Int)) {
            first[0] = second[0]
        }
        fn main() {
            var data = [1, 2, 3]
            update(first=data[0..1], second=data[1..2])
        }
    "#,
        "borrow alias",
    );
}

#[test]
fn borrow_sequential_slice_calls_ok() {
    accepted(
        r#"
        fn read(data: Slice(Int)) -> Int { data[0] }
        fn update(inout data: Slice(Int)) { data[0] = 9 }
        fn main() {
            var data = [1, 2, 3]
            let x = read(data[0..1])
            update(data[0..1])
        }
    "#,
    );
}

#[test]
fn borrow_return_parameter_requires_summary_until_callers_can_verify() {
    rejected(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) {
            data[0..1]
        }
    "#,
        "borrow return contract unavailable",
    );
}

#[test]
fn borrow_return_by_value_array_parameter_err() {
    rejected(
        r#"
        fn invalid(data: Array(Int)) -> Slice(Int) {
            data[0..1]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn borrow_return_ref_closure_capturing_parameter_err() {
    rejected(
        r#"
        fn invalid(value: Int) -> fn() -> Int {
            let get = fn[ref value]() -> Int { value }
            get
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn borrow_early_return_ends_loan_on_that_path() {
    accepted(
        r#"
        fn main(flag: Bool) -> Int {
            var data = [1, 2, 3]
            let view = data[0..2]
            if flag {
                return view[0]
            }
            data[0] = 9
            return data[0]
        }
    "#,
    );
}
