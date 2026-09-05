use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use tempra_hir::{lower_file, HirExpression, HirFile, HirItem, HirStatement};
use tempra_interpreter::env::RuntimeEnv;
use tempra_interpreter::interpreter::{EvalMode, Interpreter, InterpreterModule};
use tempra_interpreter::value::Value;
use tempra_interpreter::{run, ComptimeEvalError, RuntimeError};
use tempra_parser::parse_source;

const MISSING_BODY_ERROR: &str = "function 'missing' has no executable HIR body";

fn lower(source: &str) -> HirFile {
    let ast = parse_source(source).expect("test source must parse");
    lower_file(source, &ast)
}

fn expression_in(file: &HirFile, function_name: &str) -> HirExpression {
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            HirItem::Function(function) if function.name == function_name => Some(function),
            _ => None,
        })
        .expect("test function missing");
    let body = function.body.as_ref().expect("test function needs a body");
    match body
        .statements
        .last()
        .expect("test function needs a statement")
    {
        HirStatement::Expr(expression) => expression.clone(),
        HirStatement::Return(Some(expression)) => expression.clone(),
        statement => panic!("expected value-producing statement, found {statement:?}"),
    }
}

fn bodyless_program() -> HirFile {
    lower(
        r#"
pub extern fn missing() -> Unit
fn main() {
    missing()
}
"#,
    )
}

#[test]
fn comptime_bodyless_call_is_a_controlled_error_and_restores_state() {
    let file = bodyless_program();
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(37);
    let mut environment = RuntimeEnv::new();

    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(&expression, &mut environment, Default::default())
    }));

    assert!(result.is_ok(), "bodyless dispatch must not unwind the host");
    assert_eq!(
        result.expect("unwind already checked"),
        Err(ComptimeEvalError::Evaluation(
            MISSING_BODY_ERROR.to_string()
        ))
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(37));

    let valid = lower("fn main() { 1 + 2 }");
    let value = interpreter
        .evaluate_comptime(
            &expression_in(&valid, "main"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect("the same interpreter must remain usable");
    assert_eq!(value, Value::Int64(3));
}

#[test]
fn runtime_bodyless_call_uses_the_existing_controlled_error_boundary() {
    let result = catch_unwind(AssertUnwindSafe(|| run(&bodyless_program())));

    assert!(result.is_ok(), "bodyless dispatch must not unwind the host");
    match result.expect("unwind already checked") {
        Err(RuntimeError::Message(message)) => assert_eq!(message, MISSING_BODY_ERROR),
        other => panic!("expected controlled runtime error, found {other:?}"),
    }
}

#[test]
fn qualified_bodyless_error_preserves_identity_and_active_module() {
    let dependency = lower("pub extern fn missing() -> Unit");
    let caller = lower(
        r#"
fn local() -> Int64 { return 7 }
fn failing() { dep.missing() }
fn succeeding() -> Int64 { return local() }
"#,
    );
    let imports = HashMap::from([("dep".to_string(), "dependency".to_string())]);
    let empty_imports = HashMap::new();
    let mut interpreter = Interpreter::new_for_modules([
        InterpreterModule {
            name: "dependency",
            imports: &empty_imports,
            file: &dependency,
        },
        InterpreterModule {
            name: "caller",
            imports: &imports,
            file: &caller,
        },
    ]);
    assert!(interpreter.select_module("caller"));

    let error = interpreter
        .evaluate_comptime(
            &expression_in(&caller, "failing"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect_err("qualified bodyless call must fail");
    assert_eq!(
        error,
        ComptimeEvalError::Evaluation(
            "function 'dependency.missing' has no executable HIR body".to_string()
        )
    );

    let value = interpreter
        .evaluate_comptime(
            &expression_in(&caller, "succeeding"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect("caller module must be restored after the failed call");
    assert_eq!(value, Value::Int64(7));
}

#[test]
fn executable_empty_body_still_returns_unit() {
    let file = lower(
        r#"
fn empty() {}
fn main() { empty() }
"#,
    );

    assert_eq!(
        run(&file).expect("empty function must execute"),
        Value::Unit
    );
}

#[test]
fn fmt_builtin_still_dispatches_before_function_lookup() {
    let file = lower(
        r#"
pub extern fn fmt(value: String) -> String
fn main() { fmt("hello ", "world") }
"#,
    );

    assert_eq!(
        run(&file).expect("fmt must remain an interpreter builtin"),
        Value::String("hello world".to_string())
    );
}
