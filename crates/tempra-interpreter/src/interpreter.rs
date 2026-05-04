use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::value::Value;
use std::collections::{BTreeMap, HashMap};
use tempra_hir::*;

#[derive(Debug)]
pub enum RuntimeSignal {
    Exit(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalMode {
    Runtime,
    Comptime,
}

pub struct Interpreter {
    pub functions: HashMap<String, HirFunction>,
    pub structs: HashMap<String, HirStruct>,
    pub enums: BTreeMap<String, HirEnum>,
    pub top_level_statements: Vec<HirStatement>,
    pub args: Vec<String>,
    pub mode: EvalMode,
    pub fuel: Option<u64>,
}

impl Interpreter {
    pub fn new(file: &HirFile) -> Self {
        let mut functions = HashMap::new();
        let mut structs = HashMap::new();
        let mut enums = BTreeMap::new();
        for item in &file.items {
            crate::eval_item::register_item(&mut functions, &mut structs, &mut enums, item);
        }
        Self {
            functions,
            structs,
            enums,
            top_level_statements: file.top_level_statements.clone(),
            args: Vec::new(),
            mode: EvalMode::Runtime,
            fuel: None,
        }
    }

    pub fn run_main(&mut self) -> Result<Value, crate::RuntimeError> {
        let main_func = self.functions.get("main").cloned().ok_or_else(|| {
            crate::RuntimeError::Message("Could not find main function".to_string())
        })?;
        let flow = self
            .call_function(&main_func, None, vec![])
            .map_err(crate::RuntimeError::Message)?;
        match flow {
            ControlFlow::Value(v) | ControlFlow::Return(v) => Ok(v),
            ControlFlow::Exit(code) => Err(crate::RuntimeError::Exit(code)),
            ControlFlow::Crash(msg) => Err(crate::RuntimeError::Crash(msg)),
            ControlFlow::Break | ControlFlow::Continue => Err(crate::RuntimeError::Message(
                "Break/Continue outside of loop".into(),
            )),
        }
    }

    pub fn run_script(&mut self) -> Result<Value, crate::RuntimeError> {
        let mut env = RuntimeEnv::new();
        let mut _result_val = Value::Unit;

        let statements = self.top_level_statements.clone();
        for stmt in &statements {
            let flow = self
                .eval_statement(stmt, &mut env)
                .map_err(crate::RuntimeError::Message)?;
            match flow {
                ControlFlow::Value(v) => _result_val = v,
                ControlFlow::Return(_) => {
                    return Err(crate::RuntimeError::Message(
                        "Top-level return not allowed".into(),
                    ))
                }
                ControlFlow::Exit(code) => return Err(crate::RuntimeError::Exit(code)),
                ControlFlow::Crash(msg) => return Err(crate::RuntimeError::Crash(msg)),
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err(crate::RuntimeError::Message(
                        "Break/Continue outside of loop".into(),
                    ));
                }
            }
        }

        // Script mode implied fallible Unit
        Ok(Value::Result(Box::new(crate::value::ValueResult::Ok(
            Value::Unit,
        ))))
    }

    pub fn call_function(
        &mut self,
        func: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, String> {
        let mut env = RuntimeEnv::new();

        // Bind generic parameters
        if let Some(t_args) = type_args {
            for (param, arg) in func.generic_parameters.iter().zip(t_args.into_iter()) {
                env.insert(param.name.clone(), arg);
            }
        }

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
            Ok(ControlFlow::Value(Value::Result(Box::new(
                crate::value::ValueResult::Ok(Value::Unit),
            ))))
        } else {
            Ok(ControlFlow::Value(result_val))
        }
    }

    pub fn call_closure(
        &mut self,
        closure: &std::rc::Rc<crate::value::ClosureValue>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, String> {
        let mut env = RuntimeEnv::new();

        // Bind captures
        for (name, val) in &closure.captures {
            env.insert(name.clone(), val.clone());
        }

        // Bind parameters
        if closure.params.len() != args.len() {
            return Err("Arg count mismatch for closure".to_string());
        }
        for (param, arg) in closure.params.iter().zip(args.into_iter()) {
            env.insert(param.name.clone(), arg);
        }

        let mut result_val = Value::Unit;
        for stmt in &closure.body.statements {
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

        Ok(ControlFlow::Value(result_val))
    }

    pub fn resolve_hir_type(&self, hir_type: &HirType) -> tempra_hir::types::Type {
        match hir_type {
            HirType::Named(_, name) => {
                let ty = tempra_hir::types::resolve_named_type(name);
                if ty == tempra_hir::types::Type::Unknown {
                    if self.structs.contains_key(name) {
                        return tempra_hir::types::Type::Struct(name.clone(), vec![]);
                    }
                    if self.enums.contains_key(name) {
                        return tempra_hir::types::Type::Enum(name.clone(), vec![]);
                    }
                }
                ty
            }
            HirType::Array(_, inner) => {
                tempra_hir::types::Type::Array(Box::new(self.resolve_hir_type(inner)))
            }
            HirType::Tuple(_, elems) => tempra_hir::types::Type::Tuple(
                elems.iter().map(|e| self.resolve_hir_type(e)).collect(),
            ),
            HirType::Apply(_, callee, args) => {
                let base = self.resolve_hir_type(callee);
                let arg_types = args
                    .iter()
                    .map(|a| self.resolve_hir_type(a))
                    .collect::<Vec<_>>();
                match base {
                    tempra_hir::types::Type::Struct(name, _) => {
                        tempra_hir::types::Type::Struct(name, arg_types)
                    }
                    tempra_hir::types::Type::Enum(name, _) => {
                        tempra_hir::types::Type::Enum(name, arg_types)
                    }
                    tempra_hir::types::Type::Option(_) => {
                        if arg_types.len() == 1 {
                            tempra_hir::types::Type::Option(Box::new(arg_types[0].clone()))
                        } else {
                            tempra_hir::types::Type::Error
                        }
                    }
                    tempra_hir::types::Type::Result(_, _) => {
                        if arg_types.len() == 2 {
                            tempra_hir::types::Type::Result(
                                Box::new(arg_types[0].clone()),
                                Box::new(arg_types[1].clone()),
                            )
                        } else {
                            tempra_hir::types::Type::Error
                        }
                    }
                    _ => tempra_hir::types::Type::Error,
                }
            }
            HirType::Optional(_, inner) => {
                tempra_hir::types::Type::Option(Box::new(self.resolve_hir_type(inner)))
            }
            HirType::Result(_, ok, err) => tempra_hir::types::Type::Result(
                Box::new(self.resolve_hir_type(ok)),
                Box::new(self.resolve_hir_type(err)),
            ),
            HirType::Function(_, params, ret) => tempra_hir::types::Type::Function(
                params.iter().map(|p| self.resolve_hir_type(p)).collect(),
                Box::new(self.resolve_hir_type(ret)),
            ),
        }
    }
}
