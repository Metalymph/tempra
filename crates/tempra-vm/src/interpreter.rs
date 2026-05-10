use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{FunctionId, BytecodeType};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::verify::verify_module;
use crate::value::VmValue;
use crate::error::VmError;

pub struct Frame {
    pub function_id: FunctionId,
    pub ip: usize,
    pub locals: Vec<Option<VmValue>>,
    pub stack: Vec<VmValue>,
}

pub struct Interpreter<'a> {
    pub module: &'a BytecodeModule,
    pub call_stack: Vec<Frame>,
}

impl<'a> Interpreter<'a> {
    pub fn new(module: &'a BytecodeModule) -> Self {
        Self {
            module,
            call_stack: Vec::new(),
        }
    }

    pub fn run_function_id(&mut self, function_id: FunctionId, args: Vec<VmValue>) -> Result<VmValue, VmError> {
        // Verify module before execution
        verify_module(self.module)?;

        let func = self.module.functions.get(function_id.0 as usize)
            .ok_or_else(|| VmError::UnknownFunction(function_id.to_string()))?;

        if func.params.len() != args.len() {
            return Err(VmError::WrongArity { expected: func.params.len() as u16, found: args.len() as u16 });
        }

        // Initialize locals
        let mut locals = vec![None; func.locals.len()];
        for (i, arg) in args.into_iter().enumerate() {
            locals[i] = Some(arg);
        }

        let initial_frame = Frame {
            function_id,
            ip: 0,
            locals,
            stack: Vec::with_capacity(func.max_stack as usize),
        };

        self.call_stack.push(initial_frame);

        self.execute()
    }

    fn execute(&mut self) -> Result<VmValue, VmError> {
        loop {
            let (instr, ret_type) = {
                let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                let func = &self.module.functions[frame.function_id.0 as usize];
                if frame.ip >= func.code.len() {
                    return Err(VmError::InvalidInstructionPointer(frame.ip));
                }
                (func.code[frame.ip].clone(), func.ret)
            };

            // Increment IP before execution (jumps will override this)
            self.call_stack.last_mut().unwrap().ip += 1;

            match instr {
                BytecodeInstr::ConstUnit => self.push(VmValue::Unit)?,
                BytecodeInstr::ConstBool(b) => self.push(VmValue::Bool(b))?,
                BytecodeInstr::ConstI64(id) => {
                    let val = match &self.module.constants[id.0 as usize] {
                        BytecodeConstant::Int64(i) => VmValue::Int64(*i),
                        _ => unreachable!("Verifier should ensure type safety"),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstU64(id) => {
                    let val = match &self.module.constants[id.0 as usize] {
                        BytecodeConstant::UInt64(u) => VmValue::UInt64(*u),
                        _ => unreachable!("Verifier should ensure type safety"),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstF64(id) => {
                    let val = match &self.module.constants[id.0 as usize] {
                        BytecodeConstant::Float64(f) => VmValue::Float64(*f),
                        _ => unreachable!("Verifier should ensure type safety"),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstString(id) => {
                    let val = match &self.module.constants[id.0 as usize] {
                        BytecodeConstant::String(s) => VmValue::String(s.clone()),
                        _ => unreachable!("Verifier should ensure type safety"),
                    };
                    self.push(val)?;
                }

                BytecodeInstr::LoadLocal(id) => {
                    let val = {
                        let frame = self.call_stack.last().unwrap();
                        frame.locals.get(id.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(id.0))?
                            .clone()
                            .ok_or(VmError::LocalNotInitialized(id.0))?
                    };
                    self.push(val)?;
                }
                BytecodeInstr::StoreLocal(id) => {
                    let val = self.pop()?;
                    let frame = self.call_stack.last_mut().unwrap();
                    if id.0 as usize >= frame.locals.len() {
                        return Err(VmError::LocalOutOfBounds(id.0));
                    }
                    frame.locals[id.0 as usize] = Some(val);
                }

                BytecodeInstr::Pop => { self.pop()?; }
                BytecodeInstr::Dup => {
                    let val = self.pop()?;
                    self.push(val.clone())?;
                    self.push(val)?;
                }

                BytecodeInstr::AddI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(a + b))?;
                }
                BytecodeInstr::SubI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(a - b))?;
                }
                BytecodeInstr::MulI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(a * b))?;
                }
                BytecodeInstr::DivI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    if b == 0 { return Err(VmError::DivisionByZero); }
                    self.push(VmValue::Int64(a / b))?;
                }

                BytecodeInstr::AddU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(a + b))?;
                }
                BytecodeInstr::SubU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(a - b))?;
                }
                BytecodeInstr::MulU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(a * b))?;
                }
                BytecodeInstr::DivU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    if b == 0 { return Err(VmError::DivisionByZero); }
                    self.push(VmValue::UInt64(a / b))?;
                }

                BytecodeInstr::AddF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    self.push(VmValue::Float64(a + b))?;
                }
                BytecodeInstr::SubF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    self.push(VmValue::Float64(a - b))?;
                }
                BytecodeInstr::MulF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    self.push(VmValue::Float64(a * b))?;
                }
                BytecodeInstr::DivF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    // Float division by zero follows IEEE 754 (results in Inf/NaN)
                    self.push(VmValue::Float64(a / b))?;
                }

                BytecodeInstr::Eq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(VmValue::Bool(a == b))?;
                }
                BytecodeInstr::LtI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Bool(a < b))?;
                }
                BytecodeInstr::GtI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Bool(a > b))?;
                }
                BytecodeInstr::LtU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::Bool(a < b))?;
                }
                BytecodeInstr::GtU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::Bool(a > b))?;
                }
                BytecodeInstr::LtF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    self.push(VmValue::Bool(a < b))?;
                }
                BytecodeInstr::GtF64 => {
                    let b = self.pop_f64()?;
                    let a = self.pop_f64()?;
                    self.push(VmValue::Bool(a > b))?;
                }

                BytecodeInstr::Not => {
                    let a = self.pop_bool()?;
                    self.push(VmValue::Bool(!a))?;
                }
                BytecodeInstr::And => {
                    let b = self.pop_bool()?;
                    let a = self.pop_bool()?;
                    self.push(VmValue::Bool(a && b))?;
                }
                BytecodeInstr::Or => {
                    let b = self.pop_bool()?;
                    let a = self.pop_bool()?;
                    self.push(VmValue::Bool(a || b))?;
                }

                BytecodeInstr::Jump(offset) => {
                    self.call_stack.last_mut().unwrap().ip = offset.0;
                }
                BytecodeInstr::JumpIfFalse(offset) => {
                    let cond = self.pop_bool()?;
                    if !cond {
                        self.call_stack.last_mut().unwrap().ip = offset.0;
                    }
                }

                BytecodeInstr::Call(target_id, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse(); // Args are pushed left-to-right, so popped in reverse

                    let target_func = self.module.functions.get(target_id.0 as usize)
                        .ok_or_else(|| VmError::UnknownFunction(target_id.to_string()))?;

                    let mut locals = vec![None; target_func.locals.len()];
                    for (i, arg) in args.into_iter().enumerate() {
                        locals[i] = Some(arg);
                    }

                    let new_frame = Frame {
                        function_id: target_id,
                        ip: 0,
                        locals,
                        stack: Vec::with_capacity(target_func.max_stack as usize),
                    };
                    self.call_stack.push(new_frame);
                }

                BytecodeInstr::Return => {
                    let ret_val = if ret_type != BytecodeType::Unit {
                        Some(self.pop()?)
                    } else {
                        None
                    };

                    self.call_stack.pop();

                    if self.call_stack.is_empty() {
                        return Ok(ret_val.unwrap_or(VmValue::Unit));
                    } else {
                        if let Some(val) = ret_val {
                            self.push(val)?;
                        }
                    }
                }

                BytecodeInstr::Unreachable => return Err(VmError::UnreachableExecuted),
            }
        }
    }

    fn push(&mut self, val: VmValue) -> Result<(), VmError> {
        self.call_stack.last_mut().unwrap().stack.push(val);
        Ok(())
    }

    fn pop(&mut self) -> Result<VmValue, VmError> {
        self.call_stack.last_mut().unwrap().stack.pop()
            .ok_or(VmError::StackUnderflow)
    }

    fn pop_i64(&mut self) -> Result<i64, VmError> {
        match self.pop()? {
            VmValue::Int64(i) => Ok(i),
            other => Err(VmError::TypeMismatch { expected: "Int64".into(), found: format!("{:?}", other) }),
        }
    }

    fn pop_u64(&mut self) -> Result<u64, VmError> {
        match self.pop()? {
            VmValue::UInt64(u) => Ok(u),
            other => Err(VmError::TypeMismatch { expected: "UInt64".into(), found: format!("{:?}", other) }),
        }
    }

    fn pop_f64(&mut self) -> Result<f64, VmError> {
        match self.pop()? {
            VmValue::Float64(f) => Ok(f),
            other => Err(VmError::TypeMismatch { expected: "Float64".into(), found: format!("{:?}", other) }),
        }
    }

    fn pop_bool(&mut self) -> Result<bool, VmError> {
        match self.pop()? {
            VmValue::Bool(b) => Ok(b),
            other => Err(VmError::TypeMismatch { expected: "Bool".into(), found: format!("{:?}", other) }),
        }
    }
}

pub fn run_function(module: &BytecodeModule, name: &str, args: Vec<VmValue>) -> Result<VmValue, VmError> {
    let id = module.functions.iter().position(|f| f.name == name)
        .ok_or_else(|| VmError::UnknownFunction(name.to_string()))?;
    let mut interp = Interpreter::new(module);
    interp.run_function_id(FunctionId(id as u32), args)
}

pub fn run_function_id(module: &BytecodeModule, id: FunctionId, args: Vec<VmValue>) -> Result<VmValue, VmError> {
    let mut interp = Interpreter::new(module);
    interp.run_function_id(id, args)
}
