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
    pub top_level_statements: Vec<HirStatement>,
}

impl Interpreter {
    pub fn new(file: &HirFile) -> Self {
        let mut functions = HashMap::new();
        for item in &file.items {
            crate::eval_item::register_item(&mut functions, item);
        }
        Self { 
            functions,
            top_level_statements: file.top_level_statements.clone(),
        }
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
            ControlFlow::Crash(msg) => Err(crate::RuntimeError::Crash(msg)),
            ControlFlow::Break | ControlFlow::Continue => Err(crate::RuntimeError::Message("Break/Continue outside of loop".into())),
        }
    }

    pub fn run_script(&mut self) -> Result<Value, crate::RuntimeError> {
        let mut env = RuntimeEnv::new();
        let mut _result_val = Value::Unit;
        
        let statements = self.top_level_statements.clone();
        for stmt in &statements {
            let flow = self.eval_statement(stmt, &mut env)
                .map_err(crate::RuntimeError::Message)?;
            match flow {
                ControlFlow::Value(v) => _result_val = v,
                ControlFlow::Return(_) => return Err(crate::RuntimeError::Message("Top-level return not allowed".into())),
                ControlFlow::Exit(code) => return Err(crate::RuntimeError::Exit(code)),
                ControlFlow::Crash(msg) => return Err(crate::RuntimeError::Crash(msg)),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err(crate::RuntimeError::Message("Break/Continue outside of loop".into()));
                }
            }
        }
        
        // Script mode implied fallible Unit
        Ok(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))
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
                ControlFlow::Crash(msg) => return Ok(ControlFlow::Crash(msg)),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err("Break/Continue outside of loop".into());
                }
            }
        }
        
        if func.is_fallible {
            // Implicit Ok(()) for fallible functions returning Unit
            // We assume success here if it's Unit success type as per prompt
            // v0 check: if success type is Unit, wrap in Ok
            Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit)))))
        } else {
            Ok(ControlFlow::Value(result_val))
        }
    }
}
