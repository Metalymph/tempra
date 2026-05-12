use std::rc::Rc;
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

                BytecodeInstr::MakeEnum(enum_id, variant_id, count) => {
                    let mut payloads = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        payloads.push(self.pop()?);
                    }
                    payloads.reverse(); // Args are pushed left-to-right
                    
                    let enum_name = match self.module.constants.get(enum_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s.clone(),
                        _ => return Err(VmError::Message("Invalid enum name constant".into())),
                    };
                    let variant_name = match self.module.constants.get(variant_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s.clone(),
                        _ => return Err(VmError::Message("Invalid variant name constant".into())),
                    };
                    self.push(VmValue::Enum(enum_name, variant_name, payloads))?;
                }
                BytecodeInstr::Tag => {
                    let val = self.pop()?;
                    if let VmValue::Enum(_, variant, _) = val {
                        self.push(VmValue::String(variant))?;
                    } else {
                        return Err(VmError::ExpectedEnum);
                    }
                }
                BytecodeInstr::Unwrap(variant_id, index) => {
                    let expected_variant = match self.module.constants.get(variant_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s.clone(),
                        _ => return Err(VmError::Message("Invalid variant name constant".into())),
                    };
                    let val = self.pop()?;
                    if let VmValue::Enum(_, variant, payloads) = val {
                        if variant != expected_variant {
                            return Err(VmError::WrongVariant { expected: expected_variant, found: variant });
                        }
                        if index as usize >= payloads.len() {
                            return Err(VmError::PayloadOutOfBounds(index));
                        }
                        self.push(payloads[index as usize].clone())?;
                    } else {
                        return Err(VmError::ExpectedEnum);
                    }
                }
                BytecodeInstr::MakeArray(count) => {
                    let mut elements = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.push(VmValue::Array(Rc::new(elements)))?;
                }
                BytecodeInstr::GetIndex => {
                    let index_val = self.pop()?;
                    let object = self.pop()?;
                    let index = match index_val {
                        VmValue::Int64(i) if i >= 0 => i as usize,
                        _ => return Err(VmError::ExpectedIntIndex),
                    };

                    match object {
                        VmValue::Array(arr) => {
                            if index >= arr.len() {
                                return Err(VmError::IndexOutOfBounds(index));
                            }
                            self.push(arr[index].clone())?;
                        }
                        VmValue::Slice { parent, start, end } => {
                            let len = end - start;
                            if index >= len {
                                return Err(VmError::IndexOutOfBounds(index));
                            }
                            self.push(parent[start + index].clone())?;
                        }
                        _ => return Err(VmError::ExpectedArrayOrSlice),
                    }
                }
                BytecodeInstr::Slice => {
                    let end_val = self.pop()?;
                    let start_val = self.pop()?;
                    let object = self.pop()?;

                    let start = match start_val {
                        VmValue::Int64(i) if i >= 0 => i as usize,
                        _ => return Err(VmError::ExpectedIntIndex),
                    };
                    let end = match end_val {
                        VmValue::Int64(i) if i >= 0 => i as usize,
                        _ => return Err(VmError::ExpectedIntIndex),
                    };

                    match object {
                        VmValue::Array(arr) => {
                            if start > end || end > arr.len() {
                                return Err(VmError::InvalidSliceRange { start, end, len: arr.len() });
                            }
                            self.push(VmValue::Slice { parent: arr, start, end })?;
                        }
                        VmValue::Slice { parent, start: p_start, end: p_end } => {
                            let p_len = p_end - p_start;
                            if start > end || end > p_len {
                                return Err(VmError::InvalidSliceRange { start, end, len: p_len });
                            }
                            self.push(VmValue::Slice { parent, start: p_start + start, end: p_start + end })?;
                        }
                        _ => return Err(VmError::ExpectedArrayOrSlice),
                    }
                }
                BytecodeInstr::Len => {
                    let object = self.pop()?;
                    match object {
                        VmValue::Array(arr) => {
                            self.push(VmValue::Int64(arr.len() as i64))?;
                        }
                        VmValue::Slice { start, end, .. } => {
                            self.push(VmValue::Int64((end - start) as i64))?;
                        }
                        VmValue::String(s) => {
                            // Number of Unicode scalar values (chars)
                            self.push(VmValue::Int64(s.chars().count() as i64))?;
                        }
                        _ => return Err(VmError::ExpectedArrayOrSlice),
                    }
                }
                BytecodeInstr::Size => {
                    let object = self.pop()?;
                    if let VmValue::String(s) = object {
                        self.push(VmValue::Int64(s.len() as i64))?; // UTF-8 byte count
                    } else {
                        return Err(VmError::Message("Expected String for .size()".into()));
                    }
                }
                BytecodeInstr::Bytes => {
                    let object = self.pop()?;
                    if let VmValue::String(s) = object {
                        let bytes: Vec<VmValue> = s.as_bytes().iter().map(|&b| VmValue::UInt8(b)).collect();
                        let len = bytes.len();
                        self.push(VmValue::Slice {
                            parent: Rc::new(bytes),
                            start: 0,
                            end: len,
                        })?;
                    } else {
                        return Err(VmError::Message("Expected String for .bytes()".into()));
                    }
                }
                BytecodeInstr::ArrayPush => {
                    let value = self.pop()?;
                    let array_val = self.pop()?;
                    match array_val {
                        VmValue::Array(arr) => {
                            let mut new_arr = (*arr).clone();
                            new_arr.push(value);
                            self.push(VmValue::Array(Rc::new(new_arr)))?;
                        }
                        _ => return Err(VmError::ExpectedArray),
                    }
                }
                BytecodeInstr::MakeStruct(name_id, fields_id, count) => {
                    let mut fields = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        fields.push(self.pop()?);
                    }
                    fields.reverse();
                    
                    let name = match self.module.constants.get(name_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s,
                        _ => return Err(VmError::InvalidConstant(name_id.0)),
                    };
                        
                    let field_names = match self.module.constants.get(fields_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::FieldList(fs)) => fs,
                        _ => return Err(VmError::InvalidConstant(fields_id.0)),
                    };
                    
                    if field_names.len() != count as usize {
                        return Err(VmError::Message("MakeStruct field count mismatch".into()));
                    }
                    
                    let mut named_fields = Vec::with_capacity(count as usize);
                    for (i, name) in field_names.iter().enumerate() {
                        named_fields.push((name.clone(), fields[i].clone()));
                    }
                    
                    self.push(VmValue::Struct {
                        name: name.to_string(),
                        fields: named_fields,
                    })?;
                }
                BytecodeInstr::GetField(name_id) => {
                    let field_name = match self.module.constants.get(name_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s,
                        _ => return Err(VmError::InvalidConstant(name_id.0)),
                    };
                    let object = self.pop()?;
                    
                    if let VmValue::Struct { name: _, fields } = object {
                        if let Some((_, val)) = fields.iter().find(|(k, _)| k == field_name) {
                            self.push(val.clone())?;
                        } else {
                            return Err(VmError::Message(format!("Unknown field: {}", field_name)));
                        }
                    } else {
                        return Err(VmError::Message("Expected Struct for field access".into()));
                    }
                }
                BytecodeInstr::SetField(name_id) => {
                    let field_name = match self.module.constants.get(name_id.0 as usize) {
                        Some(tempra_bytecode::constant::BytecodeConstant::String(s)) => s,
                        _ => return Err(VmError::InvalidConstant(name_id.0)),
                    };
                    let new_val = self.pop()?;
                    let object = self.pop()?;
                    
                    if let VmValue::Struct { name, mut fields } = object {
                        let mut found = false;
                        for (k, v) in fields.iter_mut() {
                            if k == field_name {
                                *v = new_val.clone();
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Err(VmError::Message(format!("Unknown field: {}", field_name)));
                        }
                        self.push(VmValue::Struct { name, fields })?;
                    } else {
                        return Err(VmError::Message("Expected Struct for field set".into()));
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
