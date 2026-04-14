use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::value::Value;
use fero_hir::*;
use std::collections::HashMap;

pub struct Interpreter {
    pub functions: HashMap<String, HirFunction>,
}

impl Interpreter {
    pub fn new(file: &HirFile) -> Self {
        let mut functions = HashMap::new();
        for item in &file.items {
            crate::eval_item::register_item(&mut functions, item);
        }
        Self { functions }
    }

    pub fn run_main(&mut self) -> Result<Value, String> {
        let main_func = self
            .functions
            .get("main")
            .cloned()
            .ok_or_else(|| "Could not find main function".to_string())?;
        self.call_function(&main_func, vec![])
    }

    pub fn call_function(&mut self, func: &HirFunction, args: Vec<Value>) -> Result<Value, String> {
        let mut env = RuntimeEnv::new();

        // Bind parameters
        if func.parameters.len() != args.len() {
            return Err(format!("Arg count mismatch for {}", func.name));
        }
        for (param, arg) in func.parameters.iter().zip(args.into_iter()) {
            env.insert(param.name.clone(), arg);
        }

        let mut result_val = Value::Unit;
        for stmt in &func.body.statements {
            let flow = self.eval_statement(stmt, &mut env)?;
            match flow {
                ControlFlow::Value(v) => result_val = v,
                ControlFlow::Return(v) => return Ok(v),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err("Break/Continue outside of loop".into());
                }
            }
        }
        Ok(result_val)
    }
}
