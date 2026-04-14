use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::value::Value;
use fero_hir::*;
use std::collections::HashMap;

#[derive(Debug)]
pub enum RuntimeSignal {
    Exit(i32),
}

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

    pub fn run_main(&mut self) -> Result<Value, crate::RuntimeError> {
        let main_func = self
            .functions
            .get("main")
            .cloned()
            .ok_or_else(|| crate::RuntimeError::Message("Could not find main function".to_string()))?;
        let flow = self.call_function(&main_func, vec![])
            .map_err(crate::RuntimeError::Message)?;
        match flow {
            ControlFlow::Value(v) | ControlFlow::Return(v) => Ok(v),
            ControlFlow::Exit(code) => Err(crate::RuntimeError::Exit(code)),
            ControlFlow::Break | ControlFlow::Continue => Err(crate::RuntimeError::Message("Break/Continue outside of loop".into())),
        }
    }

    pub fn call_function(&mut self, func: &HirFunction, args: Vec<Value>) -> Result<ControlFlow, String> {
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
                ControlFlow::Return(v) => return Ok(ControlFlow::Return(v)),
                ControlFlow::Exit(code) => return Ok(ControlFlow::Exit(code)),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err("Break/Continue outside of loop".into());
                }
            }
        }
        Ok(ControlFlow::Value(result_val))
    }
}
