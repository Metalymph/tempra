use tempra_hir as hir;
use tempra_interpreter::value::Value;
use tempra_parser::parse_source;
use tempra_typeck::context::TypeContext;

use tempra_hir::HirFile;
use tempra_interpreter::control_flow::ControlFlow;
use tempra_interpreter::env::RuntimeEnv;
use tempra_interpreter::interpreter::Interpreter;
#[derive(Debug)]
pub enum ReplOutcome {
    Value(Value),
    Empty,
    Quit,
    Diagnostic(String),
}

pub struct ReplSession {
    type_ctx: TypeContext,
    interpreter: Interpreter,
    env: RuntimeEnv,
}

impl ReplSession {
    pub fn new() -> Self {
        let mut type_ctx = TypeContext::new();
        type_ctx.register_builtins();

        let empty_hir = HirFile {
            items: vec![],
            top_level_statements: vec![],
        };
        let interpreter = Interpreter::new(&empty_hir);

        // Setup stdlib
        // The stdlib needs a Rc<RefCell<Env>> for tests, but Interpreter handles its own RuntimeEnv.
        // Interpreter relies on its own functions map, but there might be external methods or hooks.
        // For bootstrap, we'll initialize an empty RuntimeEnv.
        let env = RuntimeEnv::new();

        Self {
            type_ctx,
            interpreter,
            env,
        }
    }

    pub fn eval_input(&mut self, input: &str) -> ReplOutcome {
        let trimmed = input.trim();
        if trimmed == ":quit" || trimmed == ":exit" {
            return ReplOutcome::Quit;
        }
        if trimmed == ":help" {
            return ReplOutcome::Diagnostic(
                "Commands:\n  :quit, :exit - Exit REPL\n  :help        - Show help".to_string(),
            );
        }
        if trimmed.is_empty() {
            return ReplOutcome::Empty;
        }

        let parse_result = parse_source(input);

        match parse_result {
            Ok(file) => {
                let mut candidate_type_ctx = self.type_ctx.clone();
                let mut hir_file = hir::lower_file(input, &file);
                if let Err(message) =
                    tempra_interpreter::ensure_capability_contracts_supported(&hir_file)
                {
                    return ReplOutcome::Diagnostic(message.to_owned());
                }

                // Re-register existing items in the AST is not needed since they are already in the context.
                // We just need to check the NEW file with the cloned context.
                // However, `tempra_typeck::check_file` is meant for fresh files.
                // We need `register_items` then `check_item` then `check_statement`.
                // For simplicity, we can use a custom pass here using internal typeck functions.

                tempra_typeck::check_item::register_items(&mut candidate_type_ctx, &hir_file.items);

                for item in &mut hir_file.items {
                    tempra_typeck::check_item::check_item(&mut candidate_type_ctx, item);
                }

                if !hir_file.top_level_statements.is_empty() {
                    candidate_type_ctx.current_is_fallible = true;
                    candidate_type_ctx.current_return_type = None;
                    for stmt in &mut hir_file.top_level_statements {
                        tempra_typeck::check_stmt::check_statement(
                            &mut candidate_type_ctx,
                            stmt,
                            None,
                        );
                    }
                    candidate_type_ctx.current_is_fallible = false;
                }

                if !candidate_type_ctx.errors.is_empty() {
                    return ReplOutcome::Diagnostic(format!(
                        "Type Error: {:?}",
                        candidate_type_ctx.errors
                    ));
                }

                // If typecheck succeeds, commit type context
                self.type_ctx = candidate_type_ctx;

                // Evaluate
                let mut candidate_interpreter = self.interpreter.clone();
                candidate_interpreter.top_level_statements.clear();
                candidate_interpreter.args.clear();

                // Register new items
                for item in &hir_file.items {
                    tempra_interpreter::eval_item::register_item(
                        &mut candidate_interpreter.functions,
                        &mut candidate_interpreter.structs,
                        &mut candidate_interpreter.enums,
                        &mut candidate_interpreter.interfaces,
                        item,
                    );
                }

                let mut candidate_env = self.env.clone();
                let mut result_val = Value::Unit;

                for stmt in &hir_file.top_level_statements {
                    match candidate_interpreter.eval_statement(stmt, &mut candidate_env) {
                        Ok(ControlFlow::Value(v)) => result_val = v,
                        Ok(_) => {
                            return ReplOutcome::Diagnostic("Unexpected control flow".to_string())
                        }
                        Err(e) => {
                            return ReplOutcome::Diagnostic(format!("Runtime Error: {:?}", e))
                        }
                    }
                }

                self.interpreter = candidate_interpreter;
                self.env = candidate_env;

                if matches!(result_val, Value::Unit) {
                    ReplOutcome::Empty
                } else {
                    ReplOutcome::Value(result_val)
                }
            }
            Err(e) => ReplOutcome::Diagnostic(format!("Parse Error: {:?}", e)),
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;

    #[test]
    fn rejected_input_is_transactional_before_type_or_runtime_registration() {
        let mut session = ReplSession::new();
        assert!(matches!(
            session.eval_input("let old = 42"),
            ReplOutcome::Empty
        ));
        for input in [
            "@deny(io) fn bad() {} let partial = 9",
            "@deny(io) fn generic(type T, value: T) -> T { return value }",
            "extend Int64 { @deny(io) fn bad_method(self) -> Int64 { return self } }",
            "@deny(panic) fn bad() -> Int64 { return 10 / 2 }",
            "@deny(fs.write(destination: \"blocked.txt\")) fn bad_specialized() {}",
            "@pure fn bad() {} let partial = 9",
            "@pure fn generic(type T, value: T) -> T { return value }",
            "extend Int64 { @pure fn bad_method(self) -> Int64 { return self } }",
            "@pure fn bad() -> Int64 { return 10 / 2 }",
        ] {
            assert!(
                matches!(session.eval_input(input), ReplOutcome::Diagnostic(message)
                if message.contains("Capability-contract enforcement is not supported"))
            );
            assert!(matches!(
                session.eval_input("old"),
                ReplOutcome::Value(Value::Int64(42))
            ));
        }
        assert!(!session.interpreter.functions.contains_key("bad"));
        assert!(!session.interpreter.functions.contains_key("generic"));
        assert!(!session
            .interpreter
            .functions
            .contains_key("Int64.bad_method"));
        assert!(!session
            .interpreter
            .functions
            .contains_key("bad_specialized"));
        assert!(
            matches!(session.eval_input("bad()"), ReplOutcome::Diagnostic(message) if message.starts_with("Type Error:"))
        );
        assert!(
            matches!(session.eval_input("partial"), ReplOutcome::Diagnostic(message) if message.starts_with("Type Error:"))
        );
        // No stale type declaration prevents registering a differently typed function.
        assert!(matches!(
            session.eval_input("fn bad() -> String { return \"clean\" }"),
            ReplOutcome::Empty
        ));
        assert!(
            matches!(session.eval_input("bad()"), ReplOutcome::Value(Value::String(value)) if value == "clean")
        );
        assert!(matches!(
            session.eval_input("old"),
            ReplOutcome::Value(Value::Int64(42))
        ));
    }
}
