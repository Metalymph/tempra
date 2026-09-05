use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

use tempra_hir::{lower_file, HirExpression, HirFile, HirItem, HirStatement};
use tempra_interpreter::env::RuntimeEnv;
use tempra_interpreter::interpreter::{EvalMode, Interpreter, InterpreterModule};
use tempra_interpreter::value::Value;
use tempra_interpreter::{ComptimeEvalConfig, ComptimeEvalError};
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

fn evaluate(
    file: &HirFile,
    expression: &HirExpression,
    config: ComptimeEvalConfig,
) -> Result<Value, ComptimeEvalError> {
    let mut interpreter = Interpreter::new(file);
    interpreter.evaluate_comptime(expression, &mut RuntimeEnv::new(), config)
}

#[test]
fn one_tick_is_allowed_and_the_next_tick_is_rejected() {
    let file = lower("fn main() { 1 }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);

    assert_eq!(
        interpreter.evaluate_comptime(
            &expression,
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::with_fuel(1),
        ),
        Ok(Value::Int64(1))
    );

    let nested = lower("fn main() { comptime (comptime 1) }");
    let nested_expression = expression_in(&nested, "main");
    assert_eq!(
        evaluate(
            &nested,
            &nested_expression,
            ComptimeEvalConfig::with_fuel(1),
        ),
        Err(ComptimeEvalError::FuelExhausted)
    );
}

#[test]
fn fuel_exhaustion_is_typed_and_does_not_unwind() {
    let file = lower("fn main() { 1 }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(23);
    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(
            &expression,
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::with_fuel(0).with_max_call_depth(1_000),
        )
    }));

    assert_eq!(
        result.expect("fuel must not unwind"),
        Err(ComptimeEvalError::FuelExhausted)
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(23));
}

#[test]
fn default_call_depth_is_the_revisionable_v01_limit() {
    let config = ComptimeEvalConfig::default();
    assert_eq!(config.max_call_depth(), 8);
    assert_eq!(
        ComptimeEvalConfig::with_fuel(7)
            .with_max_call_depth(3)
            .max_call_depth(),
        3
    );
}

#[test]
fn termination_error_messages_are_stable() {
    assert_eq!(
        ComptimeEvalError::FuelExhausted.to_string(),
        "comptime evaluation fuel exhausted"
    );
    assert_eq!(
        ComptimeEvalError::CallDepthExceeded.to_string(),
        "comptime call depth exceeded"
    );
}

#[test]
fn recursive_computation_reports_fuel_before_depth() {
    let file = lower("fn recurse() { recurse() } fn main() { recurse() }");
    let expression = expression_in(&file, "main");

    assert_eq!(
        evaluate(
            &file,
            &expression,
            ComptimeEvalConfig::with_fuel(12).with_max_call_depth(1_000),
        ),
        Err(ComptimeEvalError::FuelExhausted)
    );
}

#[test]
fn recursive_computation_reports_call_depth_when_fuel_is_high() {
    let file = lower("fn recurse() { recurse() } fn main() { recurse() }");
    let expression = expression_in(&file, "main");
    let result = catch_unwind(AssertUnwindSafe(|| {
        evaluate(
            &file,
            &expression,
            ComptimeEvalConfig::with_fuel(1_000_000).with_max_call_depth(4),
        )
    }));

    assert_eq!(
        result.expect("call depth must not unwind"),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
}

#[test]
fn default_call_depth_precedes_host_stack_exhaustion_for_direct_recursion() {
    let file = lower("fn recurse() { recurse() } fn main() { recurse() }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(41);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(
            &expression,
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::default(),
        )
    }));

    assert_eq!(
        result.expect("default call depth must stop direct recursion without unwinding"),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(41));

    let finite = lower("fn main() { 1 + 2 }");
    assert_eq!(
        interpreter.evaluate_comptime(
            &expression_in(&finite, "main"),
            &mut RuntimeEnv::new(),
            Default::default(),
        ),
        Ok(Value::Int64(3))
    );
}

#[test]
fn mutual_recursion_uses_the_same_call_depth_guard() {
    let file = lower("fn a() { b() } fn b() { a() } fn main() { a() }");
    let expression = expression_in(&file, "main");

    assert_eq!(
        evaluate(
            &file,
            &expression,
            ComptimeEvalConfig::with_fuel(1_000_000).with_max_call_depth(5),
        ),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
}

#[test]
fn default_call_depth_precedes_host_stack_exhaustion_for_mutual_recursion() {
    let file = lower("fn a() { b() } fn b() { a() } fn main() { a() }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(43);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(
            &expression,
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::default(),
        )
    }));

    assert_eq!(
        result.expect("default call depth must stop mutual recursion without unwinding"),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(43));
}

#[test]
fn default_call_depth_precedes_host_stack_exhaustion_for_recursive_closures() {
    let file = lower("fn main() { comptime { let recurse = fn[self]() { self() }; recurse() } }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(47);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(
            &expression,
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::default(),
        )
    }));

    assert_eq!(
        result.expect("default call depth must stop closure recursion without unwinding"),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(47));
}

#[test]
fn cross_module_recursion_preserves_the_caller_module() {
    let dependency = lower("pub fn recurse() { recurse() }");
    let caller = lower("fn local() -> Int64 { return 7 } fn main() { dep.recurse() }");
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

    assert_eq!(
        interpreter.evaluate_comptime(
            &expression_in(&caller, "main"),
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::with_fuel(1_000_000).with_max_call_depth(3),
        ),
        Err(ComptimeEvalError::CallDepthExceeded)
    );

    let local = expression_in(&caller, "local");
    assert_eq!(
        interpreter.evaluate_comptime(&local, &mut RuntimeEnv::new(), Default::default()),
        Ok(Value::Int64(7))
    );
}

#[test]
fn default_call_depth_precedes_host_stack_exhaustion_cross_module() {
    let dependency = lower("pub fn recurse() { recurse() }");
    let caller = lower("fn local() -> Int64 { return 7 } fn main() { dep.recurse() }");
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
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(53);

    let result = catch_unwind(AssertUnwindSafe(|| {
        interpreter.evaluate_comptime(
            &expression_in(&caller, "main"),
            &mut RuntimeEnv::new(),
            ComptimeEvalConfig::default(),
        )
    }));

    assert_eq!(
        result.expect("default call depth must stop cross-module recursion without unwinding"),
        Err(ComptimeEvalError::CallDepthExceeded)
    );
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(53));
    assert_eq!(
        interpreter.evaluate_comptime(
            &expression_in(&caller, "local"),
            &mut RuntimeEnv::new(),
            Default::default(),
        ),
        Ok(Value::Int64(7))
    );
}

#[test]
fn for_body_iterations_consume_fuel_without_a_phantom_end_tick() {
    let file = lower("fn main() { comptime { for x in \"ab\" { x } } }");
    let expression = expression_in(&file, "main");

    let result = evaluate(
        &file,
        &expression,
        ComptimeEvalConfig::with_fuel(10).with_max_call_depth(128),
    );
    assert!(matches!(result, Ok(Value::String(_))));

    assert_eq!(
        evaluate(
            &file,
            &expression,
            ComptimeEvalConfig::with_fuel(9).with_max_call_depth(128),
        ),
        Err(ComptimeEvalError::FuelExhausted)
    );
}

#[test]
fn session_state_is_restored_after_termination_errors() {
    let file = lower("fn recurse() { recurse() } fn main() { recurse() }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    interpreter.mode = EvalMode::Runtime;
    interpreter.fuel = Some(31);

    let error = interpreter.evaluate_comptime(
        &expression,
        &mut RuntimeEnv::new(),
        ComptimeEvalConfig::with_fuel(1_000_000).with_max_call_depth(2),
    );
    assert_eq!(error, Err(ComptimeEvalError::CallDepthExceeded));
    assert_eq!(interpreter.mode, EvalMode::Runtime);
    assert_eq!(interpreter.fuel, Some(31));

    let file = lower("fn main() { 1 + 2 }");
    let value = interpreter
        .evaluate_comptime(
            &expression_in(&file, "main"),
            &mut RuntimeEnv::new(),
            Default::default(),
        )
        .expect("interpreter remains usable after a depth error");
    assert_eq!(value, Value::Int64(3));
}

#[test]
fn runtime_expression_evaluation_remains_unbounded_by_comptime_limits() {
    let file = lower("fn main() { 1 + 2 }");
    let expression = expression_in(&file, "main");
    let mut interpreter = Interpreter::new(&file);
    assert!(matches!(
        interpreter
            .eval_expression(&expression, &mut RuntimeEnv::new())
            .expect("runtime evaluation succeeds"),
        tempra_interpreter::control_flow::ControlFlow::Value(Value::Int64(3))
    ));
}
