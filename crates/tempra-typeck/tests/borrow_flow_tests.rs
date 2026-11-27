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
        fn retain(callback: fn() -> Int) -> fn() -> Int { callback }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let saved = retain(get)
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
            update(first=inout data[0..1], second=data[1..2])
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
            update(inout data[0..1])
        }
    "#,
    );
}

#[test]
fn borrow_return_parameter_with_summary_ok() {
    accepted(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) {
            data[0..1]
        }
    "#,
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

#[test]
fn borrowed_return_summary_retains_caller_owner() {
    rejected(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) { data[0..1] }
        fn main() {
            var data = [1, 2, 3]
            let view = prefix(data[0..2])
            data[0] = 9
            let x = view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrowed_return_summary_allows_mutation_after_last_use() {
    accepted(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) { data[0..1] }
        fn main() {
            var data = [1, 2, 3]
            let view = prefix(data[0..2])
            let x = view[0]
            data[0] = 9
        }
    "#,
    );
}

#[test]
fn borrowed_return_summary_propagates_through_direct_calls() {
    rejected(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) { data[0..1] }
        fn again(data: Slice(Int)) -> Slice(Int) { prefix(data[0..2]) }
        fn main() {
            var data = [1, 2, 3]
            let view = again(data[0..2])
            data[0] = 9
            let x = view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrowed_return_summary_accepts_last_explicit_return() {
    accepted(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) {
            return data[0..1]
        }
    "#,
    );
}

#[test]
fn borrowed_return_summary_handles_named_arguments() {
    rejected(
        r#"
        fn prefix(data: Slice(Int), limit: Int) -> Slice(Int) { data[0..limit] }
        fn main() {
            var data = [1, 2, 3]
            let view = prefix(limit=1, data=data[0..2])
            data[0] = 9
            let x = view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn borrowed_return_summary_rejects_ambiguous_owner() {
    rejected(
        r#"
        fn choose(a: Slice(Int), b: Slice(Int), flag: Bool) -> Slice(Int) {
            if flag { a[0..1] } else { b[0..1] }
        }
    "#,
        "borrow provenance conflict",
    );
}

#[test]
fn borrowed_return_summary_tracks_binding_shadowing() {
    accepted(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) { data[0..1] }
        fn main() {
            var data = [1, 2, 3]
            let view = prefix(data[0..2])
            if true {
                var data = [4, 5, 6]
                data[0] = 9
            }
            let x = view[0]
        }
    "#,
    );
}

#[test]
fn borrowed_return_summary_is_exposed_in_context() {
    use tempra_typeck::borrow_summary::{BorrowAccess, ReturnBorrow};
    let source = r#"
        fn prefix(data: Slice(Int), count: Int) -> Slice(Int) {
            data[0..count]
        }
    "#;
    let ast = parse_source(source).unwrap();
    let mut file = lower_file(source, &ast);
    let ctx = check_file_with_context(&mut file).unwrap();
    let summary = &ctx.borrow_summaries["prefix"];
    assert_eq!(
        summary.return_borrow,
        ReturnBorrow {
            parameter_index: 0,
            mode: BorrowAccess::Read,
        }
    );
    assert_eq!(
        summary.parameter_accesses,
        vec![BorrowAccess::Read, BorrowAccess::Value]
    );
}

#[test]
fn borrowed_return_summary_unknown_provenance_fails_closed() {
    rejected(
        r#"
        fn unproven(data: Slice(Int)) -> Slice(Int) {
            var other = data
            other[0..1]
        }
        fn main() {
            var data = [1, 2, 3]
            let view = unproven(data[0..2])
            let x = view[0]
        }
    "#,
        "borrow return contract unavailable",
    );
}

#[test]
fn callback_proven_non_escaping_direct_call() {
    accepted(
        r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn main() {
            var x = 10
            let get = fn[ref x]() -> Int { x }
            let value = invoke(get)
            x = 20
        }
    "#,
    );
}

#[test]
fn callback_proven_non_escaping_named_argument() {
    accepted(
        r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let value = invoke(callback=get)
        }
    "#,
    );
}

#[test]
fn callback_proven_non_escaping_transitive_forward() {
    accepted(
        r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn twice(callback: fn() -> Int) -> Int { invoke(callback) + invoke(callback) }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let value = twice(get)
        }
    "#,
    );
}

#[test]
fn callback_proven_non_escaping_branch() {
    accepted(
        r#"
        fn choose(callback: fn() -> Int, flag: Bool) -> Int {
            if flag { callback() } else { callback() }
        }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let value = choose(get, true)
        }
    "#,
    );
}

#[test]
fn callback_proven_non_escaping_contract_exposed() {
    let source = r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn twice(callback: fn() -> Int) -> Int { invoke(callback) + invoke(callback) }
    "#;
    let ast = parse_source(source).unwrap();
    let mut file = lower_file(source, &ast);
    let ctx = check_file_with_context(&mut file).unwrap();
    assert!(ctx.callback_summaries["invoke"]
        .non_escaping_parameters
        .contains(&0));
    assert!(ctx.callback_summaries["twice"]
        .non_escaping_parameters
        .contains(&0));
}

#[test]
fn callback_returned_to_caller_is_not_nonescaping() {
    rejected(
        r#"
        fn leak(callback: fn() -> Int) -> fn() -> Int { callback }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let saved = leak(get)
        }
    "#,
        "borrow callback escape",
    );
}

#[test]
fn callback_stored_in_local_is_not_proven_nonescaping() {
    rejected(
        r#"
        fn unproven(callback: fn() -> Int) -> Int {
            let stored = callback
            stored()
        }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let value = unproven(get)
        }
    "#,
        "borrow callback escape",
    );
}

#[test]
fn callback_forward_to_unproven_is_rejected() {
    rejected(
        r#"
        fn leak(callback: fn() -> Int) -> fn() -> Int { callback }
        fn forward(callback: fn() -> Int) -> fn() -> Int { leak(callback) }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let saved = forward(get)
        }
    "#,
        "borrow callback escape",
    );
}

#[test]
fn callback_shared_read_and_unrelated_slice_is_compatible() {
    accepted(
        r#"
        fn inspect(callback: fn() -> Int, view: Slice(Int)) -> Int {
            callback() + view[0]
        }
        fn main() {
            let x = 7
            let get = fn[ref x]() -> Int { x }
            let data = [1, 2, 3]
            let result = inspect(get, data[0..1])
        }
    "#,
    );
}

#[test]
fn callback_ref_and_inout_slice_same_owner_conflict() {
    rejected(
        r#"
        fn touch(callback: fn() -> Int, inout view: Slice(Int)) -> Int {
            view[0] = 9
            callback()
        }
        fn main() {
            var data = [1, 2, 3]
            let get = fn[ref data]() -> Int { data[0] }
            let result = touch(get, inout data[0..1])
        }
    "#,
        "borrow alias",
    );
}

#[test]
fn callback_inout_and_read_slice_same_owner_conflict() {
    rejected(
        r#"
        fn inspect(callback: fn() -> Int, view: Slice(Int)) -> Int {
            callback() + view[0]
        }
        fn main() {
            var data = [1, 2, 3]
            let get = fn[inout data]() -> Int {
                data[0] = 9
                data[0]
            }
            let result = inspect(get, data[0..1])
        }
    "#,
        "borrow alias",
    );
}

#[test]
fn callback_nonescape_deterministic_across_rechecks() {
    let source = r#"
        fn invoke(callback: fn() -> Int) -> Int { callback() }
        fn forward(callback: fn() -> Int) -> Int { invoke(callback) }
        fn main() {
            let x = 10
            let get = fn[ref x]() -> Int { x }
            let result = forward(get)
        }
    "#;
    assert_eq!(check(source), check(source));
}

#[test]
fn borrow_inout_slice_requires_mutable_owner_err() {
    rejected(
        r#"
        fn update(inout data: Slice(Int)) { data[0] = 9 }
        fn main() {
            let data = [1, 2, 3]
            update(inout data[0..1])
        }
    "#,
        "inout slice owner",
    );
}

// A2: access modes, temporary call loans and composite provenance.
#[test]
fn a2_exclusive_capture_rejects_owner_read_before_last_use() {
    rejected(
        r#"
        fn main() -> Int {
            var total = 10
            let update = fn[inout total]() { total = total + 1 }
            let snapshot = total
            update()
            snapshot
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_exclusive_capture_allows_owner_read_after_last_use() {
    accepted(
        r#"
        fn main() -> Int {
            var total = 10
            let update = fn[inout total]() { total = total + 1 }
            update()
            total
        }
    "#,
    );
}

#[test]
fn a2_shared_capture_allows_owner_read() {
    accepted(
        r#"
        fn main() -> Int {
            var total = 10
            let get = fn[ref total]() -> Int { total }
            let snapshot = total
            snapshot + get()
        }
    "#,
    );
}

#[test]
fn a2_scalar_inout_conflicts_with_live_shared_capture() {
    rejected(
        r#"
        fn increment(inout value: Int) { value = value + 1 }
        fn main() -> Int {
            var total = 10
            let get = fn[ref total]() -> Int { total }
            increment(inout total)
            get()
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_scalar_inout_allows_sequential_shared_capture() {
    accepted(
        r#"
        fn increment(inout value: Int) { value = value + 1 }
        fn main() -> Int {
            var total = 10
            let get = fn[ref total]() -> Int { total }
            let before = get()
            increment(inout total)
            before + total
        }
    "#,
    );
}

#[test]
fn a2_inout_argument_kept_live_during_later_argument() {
    rejected(
        r#"
        fn bump(inout value: Int) -> Int { value = value + 1; value }
        fn combine(inout value: Int, other: Int) { value = other }
        fn main() {
            var number = 10
            combine(inout number, bump(inout number))
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_inout_arguments_of_independent_owners_are_compatible() {
    accepted(
        r#"
        fn bump(inout value: Int) -> Int { value = value + 1; value }
        fn combine(inout value: Int, other: Int) { value = other }
        fn main() {
            var first = 10
            var second = 20
            combine(inout first, bump(inout second))
        }
    "#,
    );
}

#[test]
fn a2_exclusive_receiver_conflicts_with_live_capture() {
    rejected(
        r#"
        struct Counter { var value: Int }
        extend Counter {
            fn bump(inout self) { self.value = self.value + 1 }
        }
        fn main() -> Int {
            var counter = Counter { value: 10 }
            let get = fn[ref counter]() -> Int { counter.value }
            counter.bump()
            get()
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_struct_field_preserves_borrow_until_last_use() {
    rejected(
        r#"
        struct Window { data: Slice(Int) }
        fn main() -> Int {
            var data = [1, 2, 3]
            let window = Window { data: data[0..2] }
            data[0] = 99
            window.data[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_struct_field_allows_owner_mutation_after_last_use() {
    accepted(
        r#"
        struct Window { data: Slice(Int) }
        fn main() -> Int {
            var data = [1, 2, 3]
            let window = Window { data: data[0..2] }
            let before = window.data[0]
            data[0] = 99
            before
        }
    "#,
    );
}

#[test]
fn a2_struct_with_borrowed_parameter_cannot_escape() {
    rejected(
        r#"
        struct Window { data: Slice(Int) }
        fn invalid(data: Slice(Int)) -> Window {
            Window { data: data[0..1] }
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a2_tuple_pattern_preserves_borrow_provenance() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [1, 2, 3]
            let packed = (data[0..2], 7)
            match packed {
                (view, count) => {
                    data[0] = 99
                    view[0] + count
                }
                _ => 0
            }
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_array_of_views_preserves_borrow_provenance() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [1, 2, 3]
            let views = [data[0..1], data[1..2]]
            data[0] = 99
            views[0][0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_option_pattern_preserves_borrow_provenance() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [1, 2, 3]
            let packed: Option(Slice(Int)) = Some(data[0..2])
            match packed {
                Some(view) => {
                    data[0] = 99
                    view[0]
                }
                None => 0
            }
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_enum_payload_preserves_borrow_provenance() {
    rejected(
        r#"
        enum Window { View(Slice(Int)) }
        fn main() -> Int {
            var data = [1, 2, 3]
            let packed = View(data[0..2])
            data[0] = 99
            match packed { View(view) => view[0] }
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_aggregate_assignment_cannot_erase_provenance() {
    rejected(
        r#"
        struct Window { data: Slice(Int) }
        fn main() {
            var first = [1, 2, 3]
            var second = [4, 5, 6]
            var window = Window { data: first[0..1] }
            window = Window { data: second[0..1] }
        }
    "#,
        "borrow provenance",
    );
}

#[test]
fn a2_branch_local_owner_cannot_escape_in_aggregate() {
    rejected(
        r#"
        struct Window { data: Slice(Int) }
        fn main() -> Int {
            let window = {
                let data = [1, 2, 3]
                Window { data: data[0..2] }
            }
            window.data[0]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a2_generic_value_parameter_cannot_launder_slice() {
    rejected(
        r#"
        fn identity(type T, value: T) -> T { value }
        fn main() -> Int {
            let data = [1, 2, 3]
            let view = identity(data[0..1])
            view[0]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a2_exclusive_closure_cannot_be_copied_into_second_binding() {
    rejected(
        r#"
        fn main() -> Int {
            var total = 10
            let update = fn[inout total]() { total = total + 1 }
            let alias = update
            alias()
            update()
            total
        }
    "#,
        "borrow provenance",
    );
}

#[test]
fn a2_inline_slice_iterable_keeps_owner_live() {
    rejected(
        r#"
        fn main() {
            var data = [1, 2, 3]
            for item in data[0..2] {
                data[0] = item
            }
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_if_pattern_preserves_borrow_provenance() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [1, 2, 3]
            let packed: Option(Slice(Int)) = Some(data[0..2])
            if packed is Some(view) {
                data[0] = 99
                return view[0]
            }
            0
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_for_expression_cannot_discard_result_provenance() {
    rejected(
        r#"
        fn main() {
            let data = [1, 2, 3]
            let views = for i in [0, 1] { data[i..i+1] }
        }
    "#,
        "borrow provenance",
    );
}

#[test]
fn a2_method_receiver_modes_are_isolated_per_module() {
    use std::collections::HashMap;
    use tempra_typeck::{check_modules, ModuleToCheck};

    let bad = r#"
        struct Counter { var value: Int }
        extend Counter { fn bump(inout self) { self.value = self.value + 1 } }
        fn main() -> Int {
            var counter = Counter { value: 10 }
            let get = fn[ref counter]() -> Int { counter.value }
            counter.bump()
            get()
        }
    "#;
    let good = bad
        .replace("inout self", "      self")
        .replace("self.value = self.value + 1", "let unused = self.value + 1");
    let mut bad_file = lower_file(bad, &parse_source(bad).unwrap());
    let mut good_file = lower_file(&good, &parse_source(&good).unwrap());
    let mut modules = [
        ModuleToCheck {
            name: "bad".into(),
            imports: HashMap::new(),
            file: &mut bad_file,
        },
        ModuleToCheck {
            name: "good".into(),
            imports: HashMap::new(),
            file: &mut good_file,
        },
    ];
    let errors = match check_modules(&mut modules, false) {
        Ok(_) => panic!("exclusive receiver in first module must be checked"),
        Err(errors) => errors,
    };
    assert!(
        errors
            .iter()
            .any(|e| e.module_name.as_deref() == Some("bad")
                && e.message.contains("borrow conflict")),
        "{errors:?}"
    );
    assert!(
        errors
            .iter()
            .all(|e| e.module_name.as_deref() == Some("bad")),
        "{errors:?}"
    );
}

#[test]
fn a2_generic_slice_observers_allow_non_escaping_reads() {
    accepted(
        r#"
        fn measure(type T: Collection, value: T) -> Int { value.len() }
        fn empty(type T: Collection, value: T) -> Bool { value.is_empty() }
        fn main() -> Int {
            var data = [1, 2, 3]
            let view = data[0..2]
            let length = measure[Slice(Int)](view)
            let is_empty = empty[Slice(Int)](view)
            data[0] = 9
            length
        }
    "#,
    );
}

#[test]
fn a2_generic_slice_observer_keeps_existing_loan_live() {
    rejected(
        r#"
        fn measure(type T: Collection, value: T) -> Int { value.len() }
        fn main() -> Int {
            var data = [1, 2, 3]
            let view = data[0..2]
            let length = measure[Slice(Int)](view)
            data[0] = 9
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_generic_slice_observer_name_does_not_authorize_escape() {
    rejected(
        r#"
        fn measure(type T: Collection, value: T) -> T { value }
        fn main() -> Int {
            let data = [1, 2, 3]
            let view = measure[Slice(Int)](data[0..2])
            view[0]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a2_slice_assignment_allows_last_use_copy() {
    accepted(
        r#"
        fn main() {
            var data = [1, 2, 3, 4]
            let view = data[0..2]
            data[1..3] = view
        }
    "#,
    );
}

#[test]
fn a2_slice_assignment_rejects_live_source() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [1, 2, 3, 4]
            let view = data[0..2]
            data[2..4] = view
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a2_slice_assignment_keeps_source_live_during_bounds() {
    rejected(
        r#"
        fn bound(inout data: Slice(Int)) -> Int {
            data[0] = 9
            0
        }
        fn main() {
            var data = [1, 2, 3, 4]
            var target = [0, 0]
            let view = data[0..2]
            target[bound(inout data[0..1])..2] = view
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_1_mutating_slice_return_summary_has_read_result() {
    use tempra_typeck::borrow_summary::{BorrowAccess, ReturnBorrow};
    let source = r#"
        fn normalize(inout data: Slice(Int)) -> Slice(Int) {
            data[0] = 99
            let count = data.len()
            data[0..count]
        }
    "#;
    let ast = parse_source(source).unwrap();
    let mut file = lower_file(source, &ast);
    let ctx = check_file_with_context(&mut file).unwrap();
    let summary = &ctx.borrow_summaries["normalize"];
    assert_eq!(summary.parameter_accesses, vec![BorrowAccess::Exclusive]);
    assert_eq!(
        summary.return_borrow,
        ReturnBorrow {
            parameter_index: 0,
            mode: BorrowAccess::Read,
        }
    );
}

#[test]
fn a3_1_exclusive_return_releases_owner_after_last_use() {
    accepted(
        r#"
        fn normalize(inout data: Slice(Int)) -> Slice(Int) {
            data[0] = 99
            data[0..2]
        }
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = normalize(inout data[0..3])
            let first = view[0]
            data[0] = 42
            first
        }
    "#,
    );
}

#[test]
fn a3_1_exclusive_return_retains_caller_read_loan() {
    rejected(
        r#"
        fn normalize(inout data: Slice(Int)) -> Slice(Int) {
            data[0] = 99
            data[0..2]
        }
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = normalize(inout data[0..3])
            data[0] = 42
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_1_exclusive_return_does_not_retain_exclusive_access() {
    accepted(
        r#"
        fn normalize(inout data: Slice(Int)) -> Slice(Int) {
            data[0] = 99
            data[0..1]
        }
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = normalize(inout data[0..3])
            data[1] + view[0]
        }
    "#,
    );
}

#[test]
fn a3_1_same_owner_early_and_tail_returns() {
    accepted(
        r#"
        fn prefix(data: Slice(Int), flag: Bool) -> Slice(Int) {
            let count = data.len()
            if flag { return data[0..1] }
            data[0..count]
        }
    "#,
    );
}

#[test]
fn a3_1_distinct_early_return_owner_is_rejected() {
    rejected(
        r#"
        fn choose(a: Slice(Int), b: Slice(Int), flag: Bool) -> Slice(Int) {
            if flag { return a[0..1] }
            b[0..1]
        }
    "#,
        "borrow return contract unavailable",
    );
}

#[test]
fn a3_1_early_return_of_local_owner_is_rejected() {
    rejected(
        r#"
        fn choose(data: Slice(Int), flag: Bool) -> Slice(Int) {
            if flag {
                let local = [1, 2, 3]
                return local[0..1]
            }
            data[0..1]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a3_1_shadowing_cannot_forge_parameter_origin() {
    rejected(
        r#"
        fn invalid(data: Slice(Int)) -> Slice(Int) {
            let data = [1, 2, 3]
            data[0..1]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a3_1_intermediate_owned_binding_does_not_discard_origin() {
    accepted(
        r#"
        fn prefix(data: Slice(Int)) -> Slice(Int) {
            let alias = data
            let count = alias.len()
            alias[0..count]
        }
    "#,
    );
}

#[test]
fn a3_1_resolved_named_argument_retains_return_origin() {
    rejected(
        r#"
        fn prefix(data: Slice(Int), count: Int) -> Slice(Int) {
            let limit = count
            data[0..limit]
        }
        fn forward(data: Slice(Int)) -> Slice(Int) {
            prefix(count=1, data=data)
        }
        fn main() -> Int {
            var data = [1, 2, 3]
            let view = forward(data[0..2])
            data[0] = 99
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b1_returned_read_loan_conflicts_with_new_exclusive_call() {
    rejected(
        r#"
        fn normalize(inout data: Slice(Int)) -> Slice(Int) {
            data[0] = 99
            data[0..2]
        }
        fn overwrite(inout data: Slice(Int)) {
            data[0] = 42
        }
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = normalize(inout data[0..3])
            overwrite(inout data[0..1])
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_rejects_exclusive_call_during_read_slice_bounds() {
    rejected(
        r#"
        fn bound(inout data: Slice(Int)) -> Int {
            data[0] = 9
            0
        }
        fn main() -> Int {
            var data = [1, 2, 3]
            let view = data[bound(inout data[0..1])..2]
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_cursor_materialization_does_not_extend_loan() {
    accepted(
        r#"
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = data[0..2]
            let values = view.take(1)
            data[0] = 99
            values[0]
        }
    "#,
    );
}

#[test]
fn a3_2b2_cursor_rejects_mutation_before_final_read() {
    rejected(
        r#"
        fn main() -> Int {
            var data = [10, 20, 30]
            let view = data[0..2]
            let values = view.take(1)
            data[0] = 99
            values[0] + view.count()
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_f01_accepts_array_replacement_after_read() {
    accepted(
        r#"
        fn update(inout data: Array(Int)) -> Int {
            let view = data[0..1]
            let old = view[0]
            data = [7, 8]
            old + data[0]
        }
        fn main() -> Int {
            var data = [1, 2]
            update(inout data)
        }
    "#,
    );
}

#[test]
fn a3_2b2_f01_rejects_array_replacement_before_last_use() {
    rejected(
        r#"
        fn update(inout data: Array(Int)) -> Int {
            let view = data[0..1]
            data = [7, 8]
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_f01_rejects_array_forwarding_before_last_use() {
    rejected(
        r#"
        fn replace(inout data: Array(Int)) { data = [7, 8] }
        fn update(inout data: Array(Int)) -> Int {
            let view = data[0..1]
            replace(inout data)
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_f01_rejects_array_mutation_during_read_bounds() {
    rejected(
        r#"
        fn bound(inout data: Array(Int)) -> Int {
            data = [7, 8]
            0
        }
        fn update(inout data: Array(Int)) -> Int {
            let view = data[bound(inout data)..1]
            view[0]
        }
    "#,
        "borrow conflict",
    );
}

#[test]
fn a3_2b2_f01_rejects_array_slice_escape() {
    rejected(
        r#"
        fn escape(inout data: Array(Int)) -> Slice(Int) {
            data[0..1]
        }
    "#,
        "borrow escape",
    );
}

#[test]
fn a3_2b2_f01_rejects_exclusive_subview_before_last_use() {
    rejected(
        r#"
        fn overwrite(inout data: Slice(Int)) { data[0] = 42 }
        fn update(inout data: Array(Int)) -> Int {
            let view = data[0..1]
            overwrite(inout data[0..1])
            view[0]
        }
    "#,
        "borrow conflict",
    );
}
