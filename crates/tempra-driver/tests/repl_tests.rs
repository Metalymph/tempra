use tempra_interpreter::value::Value;
use tempra_repl::repl::{ReplOutcome, ReplSession};

#[test]
fn test_repl_persistent_binding() {
    let mut session = ReplSession::new();

    // Binding
    match session.eval_input("let x = 1") {
        ReplOutcome::Empty => {}
        other => panic!("Expected Empty, got {:?}", other),
    }

    // Persistence
    match session.eval_input("x + 2") {
        ReplOutcome::Value(v) => {
            assert_eq!(v, Value::Int64(3));
        }
        other => panic!("Expected Value(3), got {:?}", other),
    }
}

#[test]
fn test_repl_function_persistence() {
    let mut session = ReplSession::new();

    session.eval_input("fn add(a: Int, b: Int) -> Int { a + b }");

    match session.eval_input("add(2, 3)") {
        ReplOutcome::Value(v) => {
            assert_eq!(v, Value::Int64(5));
        }
        other => panic!("Expected Value(5), got {:?}", other),
    }
}

#[test]
fn test_repl_error_recovery() {
    let mut session = ReplSession::new();

    session.eval_input("let x = 10");

    // Failed entry
    match session.eval_input("let y = \"oops\" +") {
        ReplOutcome::Diagnostic(_) => {}
        other => panic!("Expected Diagnostic, got {:?}", other),
    }

    // x should still be 10
    match session.eval_input("x") {
        ReplOutcome::Value(v) => {
            assert_eq!(v, Value::Int64(10));
        }
        other => panic!("Expected Value(10), got {:?}", other),
    }
}

#[test]
fn test_repl_multiline_struct() {
    let mut session = ReplSession::new();

    // Note: The session.eval_input expects a COMPLETE snippet.
    // The CLI handles buffering until it's complete.
    session.eval_input("struct User { name: String }");
    session.eval_input("let u = User { name: \"Lorenzo\" }");

    match session.eval_input("u.name") {
        ReplOutcome::Value(v) => {
            assert_eq!(v, Value::String("Lorenzo".to_string()));
        }
        other => panic!("Expected Value(\"Lorenzo\"), got {:?}", other),
    }
}
