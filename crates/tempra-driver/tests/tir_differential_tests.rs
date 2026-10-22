//! K3.3A source-level HIR <-> TIR interpreter differential tests.
//!
//! TIR execution goes through `interpret_tir_source`, which lowers, verifies
//! and interprets TIR directly. These tests intentionally do not use
//! `emit_tir_source` plus the bytecode VM.

use tempra_driver::{run_source_hir, DriverError};
use tempra_interpreter::value::Value;
use tempra_tir_interp::{TirInterpError, TirRuntimeValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SupportedValue {
    Unit,
    Bool(bool),
    Int64(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Int64Failure {
    Overflow,
    DivisionByZero,
}

fn normalize_hir(value: Value) -> Result<SupportedValue, String> {
    match value {
        Value::Unit => Ok(SupportedValue::Unit),
        Value::Bool(value) => Ok(SupportedValue::Bool(value)),
        Value::Int64(value) => Ok(SupportedValue::Int64(value)),
        other => Err(format!(
            "HIR result is outside the K3.3A supported subset: {other:?}"
        )),
    }
}

fn normalize_tir(value: TirRuntimeValue) -> Result<SupportedValue, String> {
    match value {
        TirRuntimeValue::Unit => Ok(SupportedValue::Unit),
        TirRuntimeValue::Bool(value) => Ok(SupportedValue::Bool(value)),
        TirRuntimeValue::Int64(value) => Ok(SupportedValue::Int64(value)),
        other => Err(format!(
            "TIR result is outside the K3.3A supported subset: {other:?}"
        )),
    }
}

fn assert_supported_parity(source: &str) -> SupportedValue {
    let hir_value = run_source_hir(source, &[]).unwrap_or_else(|error| {
        panic!("HIR execution failed for source:\n{source}\nError: {error:?}")
    });
    let tir_value = tempra_driver::pipeline::interpret_tir_source(source, "main", vec![])
        .unwrap_or_else(|error| {
            panic!("TIR interpretation failed for source:\n{source}\nError: {error:?}")
        });

    let hir_value = normalize_hir(hir_value).unwrap_or_else(|message| {
        panic!("K3.3A coverage limitation for source:\n{source}\n{message}")
    });
    let tir_value = normalize_tir(tir_value).unwrap_or_else(|message| {
        panic!("K3.3A coverage limitation for source:\n{source}\n{message}")
    });

    assert_eq!(
        hir_value, tir_value,
        "HIR/TIR result mismatch for source:\n{source}"
    );
    hir_value
}

/// HIR and TIR expose different error representations. K3.3A compares only the
/// semantic category, never the exact diagnostic text.
fn classify_int64_message(message: &str) -> Option<Int64Failure> {
    if message.contains("overflow") {
        Some(Int64Failure::Overflow)
    } else if message.contains("division by zero") {
        Some(Int64Failure::DivisionByZero)
    } else {
        None
    }
}

fn classify_int64_failure(error: &DriverError) -> Option<Int64Failure> {
    match error {
        DriverError::RuntimeError(message) => classify_int64_message(message),
        DriverError::TirInterpError(TirInterpError::Int64Arithmetic(error)) => {
            classify_int64_message(&error.to_string())
        }
        DriverError::TirInterpError(TirInterpError::DivisionByZero) => {
            Some(Int64Failure::DivisionByZero)
        }
        _ => None,
    }
}

fn assert_int64_failure_parity(source: &str, expected: Int64Failure) {
    let hir_error = run_source_hir(source, &[]).expect_err(&format!(
        "HIR unexpectedly succeeded for source:\n{source}\nExpected: {expected:?}"
    ));
    let tir_error = tempra_driver::pipeline::interpret_tir_source(source, "main", vec![])
        .expect_err(&format!(
            "TIR unexpectedly succeeded for source:\n{source}\nExpected: {expected:?}"
        ));

    let hir_failure = classify_int64_failure(&hir_error).unwrap_or_else(|| {
        panic!(
            "HIR failure was not a recognized Int64 category for source:\n{source}\nError: {hir_error:?}"
        )
    });
    let tir_failure = classify_int64_failure(&tir_error).unwrap_or_else(|| {
        panic!(
            "TIR failure was not a recognized Int64 category for source:\n{source}\nError: {tir_error:?}"
        )
    });

    assert_eq!(
        hir_failure, expected,
        "HIR failure category mismatch for source:\n{source}"
    );
    assert_eq!(
        tir_failure, expected,
        "TIR failure category mismatch for source:\n{source}"
    );
}

#[test]
fn int64_arithmetic_matches() {
    assert_eq!(
        assert_supported_parity("fn main() -> Int64 { return 1 + 2 * 3 }"),
        SupportedValue::Int64(7)
    );
    assert_eq!(
        assert_supported_parity("fn main() -> Int64 { return (10 - 2) / 2 }"),
        SupportedValue::Int64(4)
    );
}

#[test]
fn bool_expressions_match() {
    assert_eq!(
        assert_supported_parity(
            "fn main() -> Bool { let a = true; let b = false; return a || !b }"
        ),
        SupportedValue::Bool(true)
    );
    assert_eq!(
        assert_supported_parity("fn main() -> Bool { return 10 > 5 && 2 <= 2 }"),
        SupportedValue::Bool(true)
    );
}

#[test]
fn unit_results_match() {
    assert_eq!(
        assert_supported_parity("fn main() {}"),
        SupportedValue::Unit
    );
}

#[test]
fn let_var_and_assignment_match() {
    let source = r#"
fn main() -> Int64 {
    let base = 10
    var total = base
    total = total + 5
    total = total * 2
    return total
}
"#;
    assert_eq!(assert_supported_parity(source), SupportedValue::Int64(30));
}

#[test]
fn parameters_and_calls_match() {
    let source = r#"
fn add(a: Int64, b: Int64) -> Int64 {
    return a + b
}

fn twice(value: Int64) -> Int64 {
    return add(value, value)
}

fn main() -> Int64 {
    return twice(21)
}
"#;
    assert_eq!(assert_supported_parity(source), SupportedValue::Int64(42));
}

#[test]
fn if_else_match() {
    let source = r#"
fn choose(flag: Bool) -> Int64 {
    if flag {
        return 11
    } else {
        return 22
    }
}

fn main() -> Int64 {
    return choose(true) + choose(false)
}
"#;
    assert_eq!(assert_supported_parity(source), SupportedValue::Int64(33));
}

#[test]
fn nested_terminating_branches_match() {
    let source = r#"
fn classify(n: Int64) -> Int64 {
    if n < 0 {
        return -1
    } else {
        if n == 0 {
            return 0
        } else {
            if n > 100 { return 2 } else { return 1 }
        }
    }
}

fn main() -> Int64 {
    return classify(150)
}
"#;
    assert_eq!(assert_supported_parity(source), SupportedValue::Int64(2));
}

#[test]
fn checked_int64_overflow_matches_by_category() {
    for source in [
        "fn main() -> Int64 { return 9223372036854775807 + 1 }",
        "fn main() -> Int64 { return (0 - 9223372036854775807) - 2 }",
        "fn main() -> Int64 { return 9223372036854775807 * 2 }",
        "fn main() -> Int64 { return (0 - 9223372036854775807 - 1) / (0 - 1) }",
    ] {
        assert_int64_failure_parity(source, Int64Failure::Overflow);
    }
}

#[test]
fn checked_int64_division_by_zero_matches_by_category() {
    assert_int64_failure_parity(
        "fn main() -> Int64 { return 1 / 0 }",
        Int64Failure::DivisionByZero,
    );
}

#[test]
fn unsupported_aggregate_results_are_reported_as_coverage_limitations() {
    // The normalizers reject anything outside Unit/Bool/Int64. This explicit
    // probe records an unsupported boundary instead of treating a TIR error as
    // successful parity.
    const SOURCE: &str = r#"
fn main() -> Array(Int) {
    let values: Array(Int) = [1, 2, 3]
    return values
}
"#;

    let hir_value = run_source_hir(SOURCE, &[]).expect("HIR supports array results");
    assert!(
        matches!(hir_value, Value::Array(_)),
        "unexpected HIR value for coverage probe: {hir_value:?}"
    );

    let error = tempra_driver::pipeline::interpret_tir_source(SOURCE, "main", vec![])
        .expect_err("unsupported aggregate results must not silently pass differential parity");
    assert!(
        matches!(
            &error,
            DriverError::TirLowerError(_)
                | DriverError::TirInterpError(TirInterpError::UnsupportedInstr(_))
        ),
        "expected an explicit TIR coverage limitation, got {error:?}"
    );
}
