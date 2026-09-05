use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use tempra_hir::{lower_file, HirExpression, HirFile, HirItem, HirStatement};
use tempra_interpreter::env::RuntimeEnv;
use tempra_interpreter::interpreter::{EvalMode, Interpreter, InterpreterModule};
use tempra_interpreter::value::Value;
use tempra_interpreter::{ComptimeEvalError, RuntimeError};
use tempra_parser::parse_source;

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

fn evaluate(source: &str) -> Result<Value, ComptimeEvalError> {
    let file = lower(&format!("fn main() {{ {source} }}"));
    let expression = expression_in(&file, "main");
    Interpreter::new(&file).evaluate_comptime(
        &expression,
        &mut RuntimeEnv::new(),
        Default::default(),
    )
}

#[test]
fn cmd_construction_is_local_and_allowed() {
    let value = evaluate(r#"cmd("echo", "hello")"#).expect("construction must be allowed");
    match value {
        Value::Command(command) => {
            assert_eq!(command.program, "echo");
            assert_eq!(command.args, vec!["hello"]);
        }
        other => panic!("expected command value, found {other:?}"),
    }
}

#[test]
fn command_execution_is_rejected_at_the_shared_boundary() {
    for source in [
        r#"cmd("echo", "hello").run()"#,
        r#"cmd("true") && cmd("true")"#,
        r#"cmd("false") || cmd("true")"#,
        r#"cmd("printf", "x") | cmd("cat")"#,
    ] {
        assert_eq!(
            evaluate(source),
            Err(ComptimeEvalError::Evaluation(
                "command execution is not allowed during comptime evaluation".to_string()
            )),
            "unexpected result for {source}"
        );
    }
}

#[test]
fn observable_builtins_are_rejected_without_host_unwind() {
    for source in [
        r#"dbg("hidden")"#,
        r#"print("hidden")"#,
        r#"read_line()"#,
        r#"exit(0)"#,
        r#"fs.read("Cargo.toml")"#,
        r#"env.get("HOME")"#,
        r#"os.cwd()"#,
    ] {
        let result = catch_unwind(AssertUnwindSafe(|| evaluate(source)));
        assert!(result.is_ok(), "{source} must not unwind the host");
        let error = result.unwrap().expect_err("observable operation must fail");
        assert!(matches!(error, ComptimeEvalError::Evaluation(_)));
        assert!(error
            .to_string()
            .contains("not allowed during comptime evaluation"));
    }
}

#[test]
fn dbg_is_rejected_transitively_and_state_is_restored() {
    let file = lower(
        r#"
fn inner() { dbg("hidden") }
fn outer() { inner() }
"#,
    );
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(41);
    let error = interpreter
        .evaluate_comptime(
            &expression_in(&file, "outer"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect_err("transitive dbg must fail");

    assert_eq!(
        error,
        ComptimeEvalError::Evaluation(
            "operation 'dbg' is not allowed during comptime evaluation".to_string()
        )
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(41));
}

#[test]
fn qualified_helper_uses_module_identity_and_restores_the_caller_module() {
    let dependency = lower(
        r#"
pub fn forbidden() { dbg("hidden") }
pub fn value() -> Int64 { return 7 }
"#,
    );
    let caller = lower(
        r#"
fn failing() { dep.forbidden() }
fn succeeding() -> Int64 { return local() }
fn local() -> Int64 { return 7 }
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
        .expect_err("cross-module dbg must fail");
    assert_eq!(
        error,
        ComptimeEvalError::Evaluation(
            "operation 'dbg' is not allowed during comptime evaluation".to_string()
        )
    );

    let value = interpreter
        .evaluate_comptime(
            &expression_in(&caller, "succeeding"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect("caller module must remain usable");
    assert_eq!(value, Value::Int64(7));
}

#[test]
fn allowed_local_computation_and_runtime_behavior_remain_unchanged() {
    assert_eq!(evaluate("1 + 2").unwrap(), Value::Int64(3));

    assert_eq!(evaluate(r#"[1, 2, 3].len()"#).unwrap(), Value::Int64(3));
    assert_eq!(
        evaluate(r#""AbC".to_upper()"#).unwrap(),
        Value::String("ABC".to_string())
    );
    assert_eq!(
        evaluate(r#"Regex.text("a").is_match("a")"#).unwrap(),
        Value::Bool(true)
    );

    let runtime = lower(
        r#"
fn main() -> Int64 {
    dbg(1)
    return 7
}
"#,
    );
    match tempra_interpreter::run(&runtime) {
        Ok(value) => assert_eq!(value, Value::Int64(7)),
        Err(RuntimeError::Message(message)) => panic!("unexpected runtime error: {message}"),
        Err(other) => panic!("unexpected runtime result: {other:?}"),
    }
}
