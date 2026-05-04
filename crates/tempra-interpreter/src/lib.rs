pub mod control_flow;
pub mod env;
pub mod eval_expr;
pub mod eval_item;
pub mod eval_stmt;
pub mod interpreter;
pub mod value;

use crate::interpreter::Interpreter;
pub use crate::interpreter::RuntimeSignal;
use crate::value::Value;
use tempra_hir::HirFile;

#[derive(Debug)]
pub enum RuntimeError {
    Message(String),
    Exit(i32),
    Crash(String),
}

pub fn run(file: &HirFile) -> Result<Value, RuntimeError> {
    run_with_args(file, Vec::new())
}

pub fn run_with_args(file: &HirFile, args: Vec<String>) -> Result<Value, RuntimeError> {
    let mut interpreter = Interpreter::new(file);
    interpreter.args = args;
    if !file.top_level_statements.is_empty() {
        interpreter.run_script()
    } else {
        interpreter.run_main()
    }
}
