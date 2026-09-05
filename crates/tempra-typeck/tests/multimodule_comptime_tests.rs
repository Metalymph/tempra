use std::collections::HashMap;
use tempra_hir::{lower_file, HirExpression, HirFile, HirItem, HirStatement};
use tempra_parser::parse_source;
use tempra_typeck::{check_modules_always_with_comptime, ModuleToCheck};

fn lower(source: &str) -> HirFile {
    let ast = parse_source(source).unwrap();
    lower_file(source, &ast)
}

fn returned_integer<'a>(file: &'a HirFile, function_name: &str) -> Option<&'a str> {
    let function = file.items.iter().find_map(|item| match item {
        HirItem::Function(function) if function.name == function_name => Some(function),
        _ => None,
    })?;
    let statement = function.body.as_ref()?.statements.last()?;
    match statement {
        HirStatement::Return(Some(HirExpression::Integer(_, value))) => Some(value),
        _ => None,
    }
}

#[test]
fn evaluation_failure_does_not_corrupt_the_following_module() {
    let bad_source = "fn bad() -> String { return comptime os.cwd() }";
    let good_source = "fn good() -> Int64 { return comptime 1 + 2 }";
    let mut bad = lower(bad_source);
    let mut good = lower(good_source);
    let mut modules = [
        ModuleToCheck {
            name: "bad".to_string(),
            imports: HashMap::new(),
            file: &mut bad,
        },
        ModuleToCheck {
            name: "good".to_string(),
            imports: HashMap::new(),
            file: &mut good,
        },
    ];

    let context = check_modules_always_with_comptime(&mut modules, &HashMap::new(), false);
    drop(modules);

    assert!(context.errors.iter().any(|error| error
        .message
        .contains("not allowed during comptime evaluation")));
    assert_eq!(returned_integer(&good, "good"), Some("3"));
    let interpreter = context.interpreter.expect("canonical interpreter missing");
    assert_eq!(
        interpreter.mode,
        tempra_interpreter::interpreter::EvalMode::Runtime
    );
    assert_eq!(interpreter.fuel, None);
}

#[test]
fn arithmetic_failure_in_a_non_entry_module_does_not_corrupt_later_evaluation() {
    let bad_source = "fn unused() -> Int64 { return comptime 1 / 0 }";
    let good_source = "fn good() -> Int64 { return comptime 20 + 22 }";
    let mut bad = lower(bad_source);
    let mut good = lower(good_source);
    let mut modules = [
        ModuleToCheck {
            name: "bad".to_string(),
            imports: HashMap::new(),
            file: &mut bad,
        },
        ModuleToCheck {
            name: "good".to_string(),
            imports: HashMap::new(),
            file: &mut good,
        },
    ];

    let context = check_modules_always_with_comptime(&mut modules, &HashMap::new(), false);
    drop(modules);

    assert!(context
        .errors
        .iter()
        .any(|error| error.message.contains("integer division by zero")));
    assert_eq!(returned_integer(&good, "good"), Some("42"));
    let interpreter = context.interpreter.expect("canonical interpreter missing");
    assert_eq!(
        interpreter.mode,
        tempra_interpreter::interpreter::EvalMode::Runtime
    );
    assert_eq!(interpreter.fuel, None);
}
