use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::value::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
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

/// Typed errors produced by the interpreter kernel before legacy string
/// adapters convert them for existing runtime callers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InterpreterEvalError {
    Message(String),
    Int64Arithmetic(tempra_runtime::Int64ArithmeticError),
    FuelExhausted,
    CallDepthExceeded,
}

impl From<String> for InterpreterEvalError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for InterpreterEvalError {
    fn from(message: &str) -> Self {
        Self::Message(message.to_string())
    }
}

impl From<InterpreterEvalError> for String {
    fn from(error: InterpreterEvalError) -> Self {
        error.to_string()
    }
}

impl fmt::Display for InterpreterEvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Message(message) => formatter.write_str(message),
            Self::Int64Arithmetic(error) => error.fmt(formatter),
            Self::FuelExhausted => formatter.write_str(crate::comptime::FUEL_EXHAUSTED_MESSAGE),
            Self::CallDepthExceeded => {
                formatter.write_str(crate::comptime::CALL_DEPTH_EXCEEDED_MESSAGE)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CallDispatchError {
    MissingExecutableBody { function: String },
}

impl fmt::Display for CallDispatchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingExecutableBody { function } => {
                write!(
                    formatter,
                    "function '{function}' has no executable HIR body"
                )
            }
        }
    }
}

/// One module-owned HIR input for the interpreter's module-aware lookup adapter.
///
/// The adapter preserves each module's existing qualified identity instead of
/// flattening its declarations into a synthetic [`HirFile`].
pub struct InterpreterModule<'a> {
    /// The module identity already established by the module graph.
    pub name: &'a str,
    /// The module's resolved local import names and their qualified targets.
    pub imports: &'a HashMap<String, String>,
    /// The HIR owned by this module.
    pub file: &'a HirFile,
}

#[derive(Clone, Default)]
struct ModuleLookupContext {
    current_module: Option<String>,
    module_names: HashSet<String>,
    imports: HashMap<String, HashMap<String, String>>,
    function_owners: HashMap<String, String>,
}

#[derive(Clone)]
pub struct Interpreter {
    pub functions: HashMap<String, HirFunction>,
    pub structs: HashMap<String, HirStruct>,
    pub enums: BTreeMap<String, HirEnum>,
    pub interfaces: HashMap<String, HirInterface>,
    pub top_level_statements: Vec<HirStatement>,
    pub args: Vec<String>,
    pub mode: EvalMode,
    pub fuel: Option<u64>,
    pub(crate) comptime_call_depth: u32,
    pub(crate) max_comptime_call_depth: Option<u32>,
    module_lookup: Option<ModuleLookupContext>,
}

impl Interpreter {
    pub fn new(file: &HirFile) -> Self {
        let mut functions = HashMap::new();
        let mut structs = HashMap::new();
        let mut enums = BTreeMap::new();
        let mut interfaces = HashMap::new();
        for item in &file.items {
            crate::eval_item::register_item(
                &mut functions,
                &mut structs,
                &mut enums,
                &mut interfaces,
                item,
            );
        }
        Self {
            functions,
            structs,
            enums,
            interfaces,
            top_level_statements: file.top_level_statements.clone(),
            args: Vec::new(),
            mode: EvalMode::Runtime,
            fuel: None,
            comptime_call_depth: 0,
            max_comptime_call_depth: None,
            module_lookup: None,
        }
    }

    /// Creates an interpreter with a qualified, module-aware declaration registry.
    ///
    /// Each input file remains independently owned. Functions and nominal types
    /// are indexed using the same `module.name` identity already used by the
    /// typechecker. Call [`Self::select_module`] before evaluating an expression
    /// belonging to a particular module.
    pub fn new_for_modules<'a>(modules: impl IntoIterator<Item = InterpreterModule<'a>>) -> Self {
        let mut functions = HashMap::new();
        let mut structs = HashMap::new();
        let mut enums = BTreeMap::new();
        let mut interfaces = HashMap::new();
        let mut module_lookup = ModuleLookupContext::default();

        for module in modules {
            module_lookup.module_names.insert(module.name.to_string());
            module_lookup
                .imports
                .insert(module.name.to_string(), module.imports.clone());

            let mut local_functions = HashMap::new();
            let mut local_structs = HashMap::new();
            let mut local_enums = BTreeMap::new();
            let mut local_interfaces = HashMap::new();
            for item in &module.file.items {
                crate::eval_item::register_item(
                    &mut local_functions,
                    &mut local_structs,
                    &mut local_enums,
                    &mut local_interfaces,
                    item,
                );
            }

            for (local_name, function) in local_functions {
                let qualified = qualify_module_item(module.name, &local_name);
                module_lookup
                    .function_owners
                    .insert(qualified.clone(), module.name.to_string());
                functions.insert(qualified, function);
            }
            for (local_name, structure) in local_structs {
                structs.insert(qualify_module_item(module.name, &local_name), structure);
            }
            for (local_name, enumeration) in local_enums {
                enums.insert(qualify_module_item(module.name, &local_name), enumeration);
            }
            for (local_name, interface) in local_interfaces {
                interfaces.insert(qualify_module_item(module.name, &local_name), interface);
            }
        }

        Self {
            functions,
            structs,
            enums,
            interfaces,
            top_level_statements: Vec::new(),
            args: Vec::new(),
            mode: EvalMode::Runtime,
            fuel: None,
            comptime_call_depth: 0,
            max_comptime_call_depth: None,
            module_lookup: Some(module_lookup),
        }
    }

    /// Selects the module whose expression will next be evaluated.
    ///
    /// Returns `false` when the module was not part of the registry supplied to
    /// [`Self::new_for_modules`].
    pub fn select_module(&mut self, module: &str) -> bool {
        let Some(context) = &mut self.module_lookup else {
            return false;
        };
        if !context.module_names.contains(module) {
            return false;
        }
        context.current_module = Some(module.to_string());
        true
    }

    pub(crate) fn resolve_user_function(&self, name: &str) -> Option<(String, HirFunction)> {
        if let Some(context) = &self.module_lookup {
            if let Some(module) = &context.current_module {
                let qualified = qualify_module_item(module, name);
                if let Some(function) = self.functions.get(&qualified) {
                    return Some((qualified, function.clone()));
                }
            }
        }
        self.functions
            .get(name)
            .cloned()
            .map(|function| (name.to_string(), function))
    }

    pub(crate) fn resolve_module_function(
        &self,
        namespace: &str,
        name: &str,
    ) -> Option<(String, HirFunction)> {
        let context = self.module_lookup.as_ref()?;
        let current_module = context.current_module.as_ref()?;
        let resolved_module = context
            .imports
            .get(current_module)
            .and_then(|imports| imports.get(namespace))
            .map(String::as_str)
            .or_else(|| {
                context
                    .module_names
                    .contains(namespace)
                    .then_some(namespace)
            })?;
        let qualified = qualify_module_item(resolved_module, name);
        self.functions
            .get(&qualified)
            .cloned()
            .map(|function| (qualified, function))
    }

    pub(crate) fn call_resolved_function(
        &mut self,
        identity: &str,
        function: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, String> {
        self.call_resolved_function_internal(identity, function, type_args, args)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn call_resolved_function_internal(
        &mut self,
        identity: &str,
        function: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, InterpreterEvalError> {
        let previous_module = self
            .module_lookup
            .as_ref()
            .and_then(|context| context.current_module.clone());
        if let Some(context) = &mut self.module_lookup {
            if let Some(owner) = context.function_owners.get(identity) {
                context.current_module = Some(owner.clone());
            }
        }
        let result = self.call_function_with_identity_internal(identity, function, type_args, args);
        if let Some(context) = &mut self.module_lookup {
            context.current_module = previous_module;
        }
        result
    }

    pub(crate) fn call_resolved_function_with_env_internal(
        &mut self,
        identity: &str,
        function: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<(ControlFlow, RuntimeEnv), InterpreterEvalError> {
        let previous_module = self
            .module_lookup
            .as_ref()
            .and_then(|context| context.current_module.clone());
        if let Some(context) = &mut self.module_lookup {
            if let Some(owner) = context.function_owners.get(identity) {
                context.current_module = Some(owner.clone());
            }
        }
        let result =
            self.call_function_with_env_identity_internal(identity, function, type_args, args);
        if let Some(context) = &mut self.module_lookup {
            context.current_module = previous_module;
        }
        result
    }

    pub(crate) fn resolve_struct(&self, name: &str) -> Option<HirStruct> {
        if let Some(context) = &self.module_lookup {
            if let Some(module) = &context.current_module {
                if let Some(structure) = self.structs.get(&qualify_module_item(module, name)) {
                    return Some(structure.clone());
                }
            }
        }
        self.structs.get(name).cloned()
    }

    pub fn run_main(&mut self) -> Result<Value, crate::RuntimeError> {
        let (identity, main_func) = self.resolve_user_function("main").ok_or_else(|| {
            crate::RuntimeError::Message("Could not find main function".to_string())
        })?;
        let flow = self
            .call_resolved_function(&identity, &main_func, None, vec![])
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
        self.call_function_with_identity(&func.name, func, type_args, args)
    }

    fn call_function_with_identity(
        &mut self,
        identity: &str,
        func: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, String> {
        self.call_function_with_identity_internal(identity, func, type_args, args)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn call_function_with_identity_internal(
        &mut self,
        identity: &str,
        func: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, InterpreterEvalError> {
        self.call_function_with_env_identity_internal(identity, func, type_args, args)
            .map(|(cf, _)| cf)
    }

    pub fn call_function_with_env(
        &mut self,
        func: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<(ControlFlow, RuntimeEnv), String> {
        self.call_function_with_env_identity_internal(&func.name, func, type_args, args)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn call_function_with_env_identity_internal(
        &mut self,
        identity: &str,
        func: &HirFunction,
        type_args: Option<Vec<Value>>,
        args: Vec<Value>,
    ) -> Result<(ControlFlow, RuntimeEnv), InterpreterEvalError> {
        let body = func.body.as_ref().ok_or_else(|| {
            CallDispatchError::MissingExecutableBody {
                function: identity.to_string(),
            }
            .to_string()
        })?;

        let entered_frame = self.enter_comptime_call()?;
        let mut env = RuntimeEnv::new();

        // Bind parameters
        if func.parameters.len() != args.len() {
            let error = Err(format!("Arg count mismatch for {}", func.name).into());
            self.leave_comptime_call(entered_frame);
            return error;
        }
        for (param, arg) in func.parameters.iter().zip(args.into_iter()) {
            env.insert(param.name.clone(), arg);
        }

        // Bind generic parameters
        if let Some(t_args) = type_args {
            for (param, arg) in func.generic_parameters.iter().zip(t_args.into_iter()) {
                env.insert(param.name.clone(), arg);
            }
        }

        let mut result_val = Value::Unit;
        for stmt in &body.statements {
            let flow = match self.eval_statement_internal(stmt, &mut env) {
                Ok(flow) => flow,
                Err(error) => {
                    self.leave_comptime_call(entered_frame);
                    return Err(error);
                }
            };
            match flow {
                ControlFlow::Value(v) => result_val = v,
                ControlFlow::Return(v) => {
                    result_val = v;
                    break;
                }
                ControlFlow::Exit(code) => {
                    self.leave_comptime_call(entered_frame);
                    return Ok((ControlFlow::Exit(code), env));
                }
                ControlFlow::Crash(msg) => {
                    self.leave_comptime_call(entered_frame);
                    return Ok((ControlFlow::Crash(msg), env));
                }
                ControlFlow::Break | ControlFlow::Continue => {
                    self.leave_comptime_call(entered_frame);
                    return Err("Break/Continue outside of loop".into());
                }
            }
        }

        if func.is_fallible {
            // Implicit Ok(()) for fallible functions returning Unit
            let result = Ok((
                ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(
                    Value::Unit,
                )))),
                env,
            ));
            self.leave_comptime_call(entered_frame);
            result
        } else {
            let result = Ok((ControlFlow::Value(result_val), env));
            self.leave_comptime_call(entered_frame);
            result
        }
    }

    pub fn call_closure(
        &mut self,
        closure: &std::rc::Rc<crate::value::ClosureValue>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, String> {
        self.call_closure_internal(closure, args)
            .map_err(|error| error.to_string())
    }

    pub(crate) fn call_closure_internal(
        &mut self,
        closure: &std::rc::Rc<crate::value::ClosureValue>,
        args: Vec<Value>,
    ) -> Result<ControlFlow, InterpreterEvalError> {
        let entered_frame = self.enter_comptime_call()?;
        let mut env = RuntimeEnv::new();

        // Bind captures
        for (name, val) in &closure.captures {
            env.insert(name.clone(), val.clone());
        }

        // Bind parameters
        if closure.params.len() != args.len() {
            self.leave_comptime_call(entered_frame);
            return Err("Arg count mismatch for closure".into());
        }
        for (param, arg) in closure.params.iter().zip(args.into_iter()) {
            env.insert(param.name.clone(), arg);
        }

        let mut result_val = Value::Unit;
        for stmt in &closure.body.statements {
            let flow = match self.eval_statement_internal(stmt, &mut env) {
                Ok(flow) => flow,
                Err(error) => {
                    self.leave_comptime_call(entered_frame);
                    return Err(error);
                }
            };
            match flow {
                ControlFlow::Value(v) => result_val = v,
                ControlFlow::Return(v) => {
                    result_val = v;
                    break;
                }
                ControlFlow::Exit(code) => {
                    self.leave_comptime_call(entered_frame);
                    return Ok(ControlFlow::Exit(code));
                }
                ControlFlow::Crash(msg) => {
                    self.leave_comptime_call(entered_frame);
                    return Ok(ControlFlow::Crash(msg));
                }
                ControlFlow::Break | ControlFlow::Continue => {
                    self.leave_comptime_call(entered_frame);
                    return Err("Break/Continue outside of loop".into());
                }
            }
        }

        self.leave_comptime_call(entered_frame);
        Ok(ControlFlow::Value(result_val))
    }

    fn enter_comptime_call(&mut self) -> Result<bool, InterpreterEvalError> {
        if self.mode != EvalMode::Comptime {
            return Ok(false);
        }
        self.tick_comptime()?;
        if let Some(max_depth) = self.max_comptime_call_depth {
            if self.comptime_call_depth >= max_depth {
                return Err(InterpreterEvalError::CallDepthExceeded);
            }
        }
        self.comptime_call_depth += 1;
        Ok(true)
    }

    fn leave_comptime_call(&mut self, entered: bool) {
        if entered {
            debug_assert!(self.comptime_call_depth > 0);
            self.comptime_call_depth -= 1;
        }
    }

    pub(crate) fn tick_comptime(&mut self) -> Result<(), InterpreterEvalError> {
        if self.mode != EvalMode::Comptime {
            return Ok(());
        }
        if let Some(fuel) = &mut self.fuel {
            if *fuel == 0 {
                return Err(InterpreterEvalError::FuelExhausted);
            }
            *fuel -= 1;
        }
        Ok(())
    }

    pub fn resolve_hir_type(&self, hir_type: &HirType) -> tempra_hir::types::Type {
        match hir_type {
            HirType::Named(_, name) => {
                let ty = tempra_hir::types::resolve_named_type(name);
                if ty == tempra_hir::types::Type::Unknown {
                    if self.resolve_struct(name).is_some() {
                        return tempra_hir::types::Type::Struct(name.clone(), vec![]);
                    }
                    if self.resolve_enum(name).is_some() {
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
            HirType::SelfType(_) => tempra_hir::types::Type::SelfType,
            HirType::Projection(_, _, _) => tempra_hir::types::Type::Error,
        }
    }

    fn resolve_enum(&self, name: &str) -> Option<HirEnum> {
        if let Some(context) = &self.module_lookup {
            if let Some(module) = &context.current_module {
                if let Some(enumeration) = self.enums.get(&qualify_module_item(module, name)) {
                    return Some(enumeration.clone());
                }
            }
        }
        self.enums.get(name).cloned()
    }
}

fn qualify_module_item(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_string()
    } else {
        format!("{module}.{name}")
    }
}
