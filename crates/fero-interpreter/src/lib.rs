pub mod control_flow;
pub mod env;
pub mod eval_expr;
pub mod eval_item;
pub mod eval_stmt;
pub mod interpreter;
pub mod value;

use crate::interpreter::Interpreter;
use crate::value::Value;
use fero_hir::HirFile;

pub fn run(file: &HirFile) -> Result<Value, String> {
    let mut interpreter = Interpreter::new(file);
    interpreter.run_main()
}
