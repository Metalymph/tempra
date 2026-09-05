use std::panic::{catch_unwind, AssertUnwindSafe};

use tempra_hir::{lower_file, HirExpression, HirFile, HirItem, HirStatement};
use tempra_interpreter::env::RuntimeEnv;
use tempra_interpreter::interpreter::{EvalMode, Interpreter};
use tempra_interpreter::value::Value;
use tempra_interpreter::{ComptimeEvalError, RuntimeError};
use tempra_parser::parse_source;

fn expression(source: &str) -> HirExpression {
    let source = format!("fn main() {{ {source} }}");
    let ast = parse_source(&source).expect("test source must parse");
    let hir = lower_file(&source, &ast);
    let function = match &hir.items[0] {
        HirItem::Function(function) => function,
        item => panic!("expected function, found {item:?}"),
    };
    let body = function.body.as_ref().expect("test function needs a body");
    match body
        .statements
        .last()
        .expect("test function needs a statement")
    {
        HirStatement::Expr(expression) => expression.clone(),
        statement => panic!("expected expression statement, found {statement:?}"),
    }
}

fn evaluate(source: &str) -> Result<Value, ComptimeEvalError> {
    let mut interpreter = Interpreter::new(&HirFile {
        items: Vec::new(),
        top_level_statements: Vec::new(),
    });
    let mut environment = RuntimeEnv::new();
    interpreter.evaluate_comptime(&expression(source), &mut environment, Default::default())
}

fn assert_evaluation_error(source: &str, expected: &str) {
    assert_eq!(
        evaluate(source),
        Err(ComptimeEvalError::Evaluation(expected.to_string()))
    );
}

#[test]
fn normal_integer_arithmetic_still_succeeds() {
    assert_eq!(evaluate("1 + 2"), Ok(Value::Int64(3)));
}

#[test]
fn signed_overflow_and_underflow_are_deterministic_errors() {
    assert_evaluation_error("9223372036854775807 + 1", "integer addition overflow");
    assert_evaluation_error(
        "(-9223372036854775807 - 1) - 1",
        "integer subtraction overflow",
    );
    assert_evaluation_error("9223372036854775807 * 2", "integer multiplication overflow");
}

#[test]
fn signed_division_failures_are_distinguished() {
    assert_evaluation_error("1 / 0", "integer division by zero");
    assert_evaluation_error(
        "(-9223372036854775807 - 1) / -1",
        "integer division overflow",
    );
}

#[test]
fn negating_minimum_integer_is_a_deterministic_error() {
    assert_evaluation_error("-(-9223372036854775807 - 1)", "integer negation overflow");
}

#[test]
fn unsigned_byte_division_by_zero_is_controlled() {
    assert_evaluation_error(
        r#""A".bytes()[0] / ("A".bytes()[0] - "A".bytes()[0])"#,
        "integer division by zero",
    );
}

#[test]
fn nested_arithmetic_propagates_the_semantic_error() {
    assert_evaluation_error("1 + (2 / 0)", "integer division by zero");
}

#[test]
fn float_division_by_zero_keeps_ieee_behavior() {
    let value = evaluate("1.0 / 0.0").expect("float division should produce a value");
    assert!(
        matches!(value, Value::Float64(number) if number.is_infinite() && number.is_sign_positive())
    );
}

#[test]
fn arithmetic_failure_restores_the_comptime_session_state() {
    let mut interpreter = Interpreter::new(&HirFile {
        items: Vec::new(),
        top_level_statements: Vec::new(),
    });
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(19);
    let mut environment = RuntimeEnv::new();

    let result =
        interpreter.evaluate_comptime(&expression("1 / 0"), &mut environment, Default::default());

    assert_eq!(
        result,
        Err(ComptimeEvalError::Evaluation(
            "integer division by zero".to_string()
        ))
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(19));
}

#[test]
fn arithmetic_failures_do_not_unwind_the_host() {
    for source in ["1 / 0", "9223372036854775807 + 1"] {
        let result = catch_unwind(AssertUnwindSafe(|| evaluate(source)));
        assert!(result.is_ok(), "host panic escaped for {source}");
        assert!(matches!(
            result.expect("unwind already checked"),
            Err(ComptimeEvalError::Evaluation(_))
        ));
    }
}

#[test]
fn shared_runtime_kernel_returns_the_same_controlled_error() {
    let source = "fn main() -> Int64 { return 9223372036854775807 + 1 }";
    let ast = parse_source(source).expect("test source must parse");
    let hir = lower_file(source, &ast);

    match tempra_interpreter::run(&hir) {
        Err(RuntimeError::Message(message)) => {
            assert_eq!(message, "integer addition overflow");
        }
        result => panic!("expected controlled runtime arithmetic error, found {result:?}"),
    }
}
