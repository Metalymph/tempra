pub mod control_flow;
pub mod env;
pub mod eval_expr;
pub mod eval_item;
pub mod eval_stmt;
pub mod interpreter;
pub mod value;

use crate::interpreter::Interpreter;
use crate::value::Value;
pub use crate::interpreter::RuntimeSignal;
use fero_hir::HirFile;

#[derive(Debug)]
pub enum RuntimeError {
    Message(String),
    Exit(i32),
}

pub fn run(file: &HirFile) -> Result<Value, RuntimeError> {
    let mut interpreter = Interpreter::new(file);
    interpreter.run_main()
}
