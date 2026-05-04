use anyhow::Result;
use tempra_ir::module::Module;
use tempra_runtime::Value;

use crate::frame::CallStack;

/// Evaluates a compiled [`Module`] and returns the result of its entry point.
pub fn eval(module: &Module) -> Result<Value> {
    let mut stack = CallStack::new();
    // TODO: locate the entry function, execute its body instruction-by-instruction
    let _ = &mut stack;
    let _ = module;
    Ok(Value::Unit)
}
