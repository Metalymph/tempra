use tempra_driver::DriverError;
use tempra_interpreter::value::{Value, ValueResult};
use tempra_vm::value::VmValue;

fn assert_exit(result: Result<(), DriverError>, expected: i32) {
    match result {
        Err(DriverError::Exit(code)) => assert_eq!(code, expected),
        other => panic!("expected exit signal {expected}, got {other:?}"),
    }
}

#[test]
fn raw_hir_execution_preserves_main_return_value() {
    let value = tempra_driver::run_source_hir("pub fn main() -> Int64 { return 7 }", &[]).unwrap();
    assert!(matches!(value, Value::Int64(7)));

    let value = tempra_driver::run_source_hir(
        "pub fn main() -> Result(Int64, String) { return Ok(42) }",
        &[],
    )
    .unwrap();
    let Value::Result(result) = value else {
        panic!("expected Result value");
    };
    assert!(matches!(*result, ValueResult::Ok(Value::Int64(42))));
}

#[test]
fn raw_vm_execution_preserves_main_return_value() {
    let value = tempra_driver::run_source_vm("pub fn main() -> Int64 { return 7 }", &[]).unwrap();
    assert_eq!(value, VmValue::Int64(7));

    let value = tempra_driver::run_source_vm(
        "pub fn main() -> Result(Int64, String) { return Ok(42) }",
        &[],
    )
    .unwrap();
    assert_eq!(
        value,
        VmValue::Enum("Result".into(), "Ok".into(), vec![VmValue::Int64(42)])
    );
}

#[test]
fn hir_entry_int_returns_an_exit_signal() {
    assert_exit(
        tempra_driver::run_source_with_args("pub fn main() -> Int64 { return 0 }", &[]),
        0,
    );
    assert_exit(
        tempra_driver::run_source_with_args("pub fn main() -> Int64 { return 7 }", &[]),
        7,
    );
}

#[test]
fn vm_entry_int_returns_an_exit_signal() {
    assert_exit(
        tempra_driver::run_source_with_experimental_vm("pub fn main() -> Int64 { return 0 }", &[]),
        0,
    );
    assert_exit(
        tempra_driver::run_source_with_experimental_vm("pub fn main() -> Int64 { return 7 }", &[]),
        7,
    );
}

#[test]
fn hir_entry_result_ok_succeeds_and_err_is_uncaught() {
    tempra_driver::run_source_with_args(
        "pub fn main() -> Result(Int64, String) { return Ok(42) }",
        &[],
    )
    .unwrap();

    let error = tempra_driver::run_source_with_args(
        "pub fn main() -> Result(Int64, String) { return Err(\"boom\") }",
        &[],
    )
    .unwrap_err();
    assert!(
        matches!(error, DriverError::RuntimeError(ref message) if message == "Uncaught error: boom")
    );
}

#[test]
fn vm_entry_result_ok_succeeds_and_err_is_uncaught() {
    tempra_driver::run_source_with_experimental_vm(
        "pub fn main() -> Result(Int64, String) { return Ok(42) }",
        &[],
    )
    .unwrap();

    let error = tempra_driver::run_source_with_experimental_vm(
        "pub fn main() -> Result(Int64, String) { return Err(\"boom\") }",
        &[],
    )
    .unwrap_err();
    assert!(
        matches!(error, DriverError::RuntimeError(ref message) if message == "Uncaught error: boom")
    );
}

#[test]
fn entry_int_outside_the_portable_range_is_not_wrapped_or_truncated() {
    for source in [
        "pub fn main() -> Int64 { return 256 }",
        "pub fn main() -> Int64 { return -1 }",
    ] {
        for result in [
            tempra_driver::run_source_with_args(source, &[]),
            tempra_driver::run_source_with_experimental_vm(source, &[]),
        ] {
            let error = result.unwrap_err();
            assert!(
                matches!(error, DriverError::RuntimeError(ref message)
                    if message.contains("portable exit codes are 0..=255")),
                "unexpected error: {error:?}"
            );
        }
    }
}
