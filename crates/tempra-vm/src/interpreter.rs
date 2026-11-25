use crate::error::VmError;
use crate::profile::{FunctionProfile, ProfilingState, VmOptions, VmProfile};
use crate::value::{VmBorrow, VmClosure, VmValue};
use std::collections::HashSet;
use std::rc::Rc;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::function::{BytecodeSignature, BytecodeType, FunctionId, SignatureId};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::verify::verify_module;

pub struct Frame {
    /// Unique lifetime identity; never reused within this interpreter.
    pub token: u64,
    pub function_id: FunctionId,
    pub ip: usize,
    pub locals: Vec<Option<VmValue>>,
    pub stack: Vec<VmValue>,
    /// Transient exclusive owner loans acquired by the caller for this frame.
    /// Release them on every normal return, including nested calls.
    pub inout_loans: Vec<(u64, u32)>,
}

pub type NativeResolver = fn(&str, &str) -> Option<fn(&[VmValue]) -> Result<VmValue, String>>;

pub struct Interpreter<'a> {
    pub module: &'a BytecodeModule,
    pub call_stack: Vec<Frame>,
    next_frame_token: u64,
    borrowed_sources: HashSet<(u64, u32)>,
    exclusive_sources: HashSet<(u64, u32)>,
    pub(crate) profiling: Option<ProfilingState>,
    pub native_resolver: Option<NativeResolver>,
}

impl<'a> Interpreter<'a> {
    pub fn new(module: &'a BytecodeModule) -> Self {
        Self {
            module,
            call_stack: Vec::with_capacity(32),
            next_frame_token: 0,
            borrowed_sources: HashSet::new(),
            exclusive_sources: HashSet::new(),
            profiling: None,
            native_resolver: None,
        }
    }

    pub fn with_options(module: &'a BytecodeModule, options: VmOptions) -> Self {
        let profiling = if options.profiling {
            Some(ProfilingState::new(module.functions.len(), options))
        } else {
            None
        };

        Self {
            module,
            call_stack: Vec::with_capacity(32),
            next_frame_token: 0,
            borrowed_sources: HashSet::new(),
            exclusive_sources: HashSet::new(),
            profiling,
            native_resolver: None,
        }
    }

    pub fn run_function_id(
        &mut self,
        function_id: FunctionId,
        args: Vec<VmValue>,
    ) -> Result<VmValue, VmError> {
        // Verify module before execution
        verify_module(self.module)?;

        let func = self
            .module
            .functions
            .get(function_id.0 as usize)
            .ok_or_else(|| VmError::UnknownFunction(function_id.to_string()))?;

        if func.inout_params.iter().any(|is_inout| *is_inout) {
            return Err(VmError::Message(
                "inout entry arguments require a live caller frame".into(),
            ));
        }
        if func.params.len() != args.len() {
            return Err(VmError::WrongArity {
                expected: func.params.len() as u16,
                found: args.len() as u16,
            });
        }

        // Initialize locals
        let mut locals = vec![None; func.locals.len()];
        for (i, arg) in args.into_iter().enumerate() {
            locals[i] = Some(arg);
        }

        let initial_max_stack = func.max_stack as usize;
        let initial_token = self.allocate_frame_token()?;
        let initial_frame = Frame {
            token: initial_token,
            function_id,
            ip: 0,
            locals,
            stack: Vec::with_capacity(initial_max_stack),
            inout_loans: Vec::new(),
        };

        self.call_stack.push(initial_frame);

        if let Some(ref mut prof) = self.profiling {
            prof.stats[function_id.0 as usize].call_count += 1;
        }

        self.execute()
    }

    pub fn run_function_profiled(
        module: &'a BytecodeModule,
        name: &str,
        args: Vec<VmValue>,
        options: VmOptions,
    ) -> Result<(VmValue, VmProfile), VmError> {
        Self::run_function_profiled_with_resolver(module, name, args, options, None)
    }

    pub fn run_function_profiled_with_resolver(
        module: &'a BytecodeModule,
        name: &str,
        args: Vec<VmValue>,
        options: VmOptions,
        resolver: Option<NativeResolver>,
    ) -> Result<(VmValue, VmProfile), VmError> {
        let id = module
            .functions
            .iter()
            .position(|f| f.name == name)
            .map(|i| FunctionId(i as u32))
            .ok_or_else(|| VmError::UnknownFunction(name.to_string()))?;

        let mut interpreter = Interpreter::with_options(module, options);
        interpreter.native_resolver = resolver;
        let res = interpreter.run_function_id(id, args)?;
        let profile = interpreter.build_profile();
        Ok((res, profile))
    }

    pub fn build_profile(&self) -> VmProfile {
        let stats = match &self.profiling {
            Some(s) => s,
            None => {
                return VmProfile {
                    function_stats: vec![],
                    global_opcode_counts: vec![],
                }
            }
        };

        let mut function_stats = Vec::new();
        for (i, s) in stats.stats.iter().enumerate() {
            let func = &self.module.functions[i];
            let is_hot = s.call_count >= stats.options.hot_call_threshold
                || s.backedge_count >= stats.options.hot_backedge_threshold;

            let mut top_opcodes: Vec<_> = s
                .opcode_counts
                .iter()
                .enumerate()
                .filter(|(_, &count)| count > 0)
                .collect();
            top_opcodes.sort_by_key(|(_, &count)| std::cmp::Reverse(count));

            let top_opcodes_named = top_opcodes
                .iter()
                .take(5)
                .map(|&(id, count)| (self.get_opcode_name(id), *count))
                .collect();

            function_stats.push(FunctionProfile {
                function_id: FunctionId(i as u32),
                function_name: func.name.clone(),
                call_count: s.call_count,
                instruction_count: s.instruction_count,
                backedge_count: s.backedge_count,
                is_hot,
                top_opcodes: top_opcodes_named,
            });
        }

        VmProfile {
            function_stats,
            global_opcode_counts: stats.global_opcode_counts.clone(),
        }
    }

    fn get_opcode_name(&self, id: usize) -> &'static str {
        match id {
            0 => "ConstUnit",
            1 => "ConstBool",
            2 => "ConstI64",
            3 => "ConstU64",
            4 => "ConstF64",
            5 => "ConstString",
            6 => "LoadLocal",
            7 => "StoreLocal",
            8 => "Pop",
            9 => "Dup",
            10 => "AddI64",
            11 => "SubI64",
            12 => "MulI64",
            13 => "DivI64",
            14 => "AddU64",
            15 => "SubU64",
            16 => "MulU64",
            17 => "DivU64",
            18 => "AddF64",
            19 => "SubF64",
            20 => "MulF64",
            21 => "DivF64",
            22 => "Eq",
            23 => "LtI64",
            24 => "GtI64",
            25 => "LtU64",
            26 => "GtU64",
            27 => "LtF64",
            28 => "GtF64",
            29 => "Not",
            30 => "And",
            31 => "Or",
            32 => "Jump",
            33 => "JumpIfFalse",
            34 => "Return",
            35 => "Unreachable",
            36 => "Call",
            37 => "MakeEnum",
            38 => "Tag",
            39 => "Unwrap",
            40 => "MakeArray",
            41 => "MakeTuple",
            42 => "GetIndex",
            43 => "Slice",
            44 => "Len",
            45 => "Size",
            46 => "Bytes",
            47 => "MakeStruct",
            48 => "GetField",
            49 => "SetField",
            50 => "ArrayPush",
            51 => "AddI64LL",
            52 => "AddI64LC",
            53 => "MakeClosure",
            54 => "CallClosure",
            55 => "BorrowLocal",
            56 => "MakeClosureRef",
            57 => "BorrowLocalMut",
            58 => "MakeClosureInout",
            59 => "BorrowSliceMut",
            60 => "InoutSliceGet",
            61 => "InoutSliceSet",
            62 => "InoutSliceLen",
            _ => "Unknown",
        }
    }

    fn execute(&mut self) -> Result<VmValue, VmError> {
        loop {
            let (instr, ret_type) = {
                let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                let func = &self.module.functions[frame.function_id.0 as usize];
                if frame.ip >= func.code.len() {
                    return Err(VmError::InvalidInstructionPointer(frame.ip));
                }
                (func.code[frame.ip], func.ret)
            };

            // Increment IP before execution
            let (frame_func_id, current_ip) = {
                let frame = self.call_stack.last_mut().unwrap();
                let func_id = frame.function_id;
                let ip = frame.ip;
                frame.ip += 1;
                (func_id, ip)
            };

            if let Some(ref mut prof) = self.profiling {
                let stats = &mut prof.stats[frame_func_id.0 as usize];
                stats.instruction_count += 1;
                let op_id = instr.opcode_id();
                if op_id < stats.opcode_counts.len() {
                    stats.opcode_counts[op_id] += 1;
                }
                if op_id < prof.global_opcode_counts.len() {
                    prof.global_opcode_counts[op_id] += 1;
                }
            }

            match instr {
                BytecodeInstr::ConstUnit => self.push(VmValue::Unit)?,
                BytecodeInstr::ConstBool(b) => self.push(VmValue::Bool(b))?,
                BytecodeInstr::ConstI64(id) => {
                    let val = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::Int64(i)) => VmValue::Int64(*i),
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstU64(id) => {
                    let val = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::UInt64(u)) => VmValue::UInt64(*u),
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstF64(id) => {
                    let val = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::Float64(f)) => VmValue::Float64(*f),
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };
                    self.push(val)?;
                }
                BytecodeInstr::ConstString(id) => {
                    let val = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::String(s)) => VmValue::String(s.clone()),
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };
                    self.push(val)?;
                }

                BytecodeInstr::LoadLocal(id) => {
                    let val = {
                        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                        frame
                            .locals
                            .get(id.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(id.0))?
                            .as_ref()
                            .ok_or(VmError::LocalNotInitialized(id.0))?
                            .clone()
                    };
                    let value = self.resolve_borrowed_value(val)?;
                    self.push(value)?;
                }
                BytecodeInstr::StoreLocal(id) => {
                    let val = self.pop_raw()?;
                    let (token, existing) = {
                        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                        let existing = frame
                            .locals
                            .get(id.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(id.0))?
                            .clone();
                        (frame.token, existing)
                    };
                    if matches!(&val, VmValue::BorrowedLocal(_)) {
                        return Err(VmError::Message(
                            "raw borrow handle cannot be stored as a value".into(),
                        ));
                    }
                    if let Some(VmValue::BorrowedLocal(handle)) = existing {
                        if handle.range.is_some() {
                            return Err(VmError::Message(
                                "exclusive slice parameter cannot be rebound".into(),
                            ));
                        }
                        if !handle.mutable {
                            return Err(VmError::Message(
                                "cannot mutate through immutable ref capture".into(),
                            ));
                        }
                        if !self
                            .exclusive_sources
                            .contains(&(handle.owner_frame, handle.local_id.0))
                        {
                            return Err(VmError::Message(
                                "exclusive borrowed owner loan is no longer active".into(),
                            ));
                        }
                        let owner = self
                            .call_stack
                            .iter_mut()
                            .find(|frame| frame.token == handle.owner_frame)
                            .ok_or_else(|| {
                                VmError::Message(
                                    "exclusive borrowed owner frame is no longer alive".into(),
                                )
                            })?;
                        let target = owner
                            .locals
                            .get_mut(handle.local_id.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(handle.local_id.0))?;
                        if matches!(target, Some(VmValue::BorrowedLocal(_))) {
                            return Err(VmError::Message(
                                "nested exclusive borrowed owner is unsupported".into(),
                            ));
                        }
                        *target = Some(val);
                        continue;
                    }
                    if self.borrowed_sources.contains(&(token, id.0)) {
                        return Err(VmError::Message(
                            "cannot overwrite an active borrowed owner directly".into(),
                        ));
                    }
                    self.call_stack
                        .last_mut()
                        .ok_or(VmError::MissingReturn)?
                        .locals[id.0 as usize] = Some(val);
                }

                BytecodeInstr::Pop => {
                    self.pop()?;
                }
                BytecodeInstr::Dup => {
                    let val = self.pop()?;
                    self.push(val.clone())?;
                    self.push(val)?;
                }

                BytecodeInstr::AddI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(tempra_runtime::add_i64(a, b)?))?;
                }
                BytecodeInstr::SubI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(tempra_runtime::sub_i64(a, b)?))?;
                }
                BytecodeInstr::MulI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(tempra_runtime::mul_i64(a, b)?))?;
                }
                BytecodeInstr::DivI64 => {
                    let b = self.pop_i64()?;
                    let a = self.pop_i64()?;
                    self.push(VmValue::Int64(tempra_runtime::div_i64(a, b)?))?;
                }

                BytecodeInstr::AddU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(tempra_runtime::add_u64(a, b)?))?;
                }
                BytecodeInstr::SubU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(tempra_runtime::sub_u64(a, b)?))?;
                }
                BytecodeInstr::MulU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(tempra_runtime::mul_u64(a, b)?))?;
                }
                BytecodeInstr::DivU64 => {
                    let b = self.pop_u64()?;
                    let a = self.pop_u64()?;
                    self.push(VmValue::UInt64(tempra_runtime::div_u64(a, b)?))?;
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
                    if let Some(ref mut prof) = self.profiling {
                        if offset.0 <= current_ip {
                            let stats = &mut prof.stats[frame_func_id.0 as usize];
                            stats.backedge_count += 1;
                        }
                    }
                    self.call_stack.last_mut().ok_or(VmError::MissingReturn)?.ip = offset.0;
                }
                BytecodeInstr::JumpIfFalse(offset) => {
                    let cond = self.pop_bool()?;
                    if !cond {
                        if let Some(ref mut prof) = self.profiling {
                            if offset.0 <= current_ip {
                                let stats = &mut prof.stats[frame_func_id.0 as usize];
                                stats.backedge_count += 1;
                            }
                        }
                        self.call_stack.last_mut().ok_or(VmError::MissingReturn)?.ip = offset.0;
                    }
                }

                BytecodeInstr::BorrowSliceMut(local) => {
                    let end =
                        usize::try_from(self.pop_i64()?).map_err(|_| VmError::ExpectedIntIndex)?;
                    let start =
                        usize::try_from(self.pop_i64()?).map_err(|_| VmError::ExpectedIntIndex)?;
                    let handle = self.borrow_slice_mut(local, start, end)?;
                    self.push(VmValue::BorrowedLocal(handle))?;
                }
                BytecodeInstr::InoutSliceGet(local) => {
                    let index =
                        usize::try_from(self.pop_i64()?).map_err(|_| VmError::ExpectedIntIndex)?;
                    let (handle, start, end) = self.exclusive_slice_handle(local)?;
                    if index >= end - start {
                        return Err(VmError::IndexOutOfBounds(index));
                    }
                    let value = self
                        .slice_owner(handle)?
                        .get(start + index)
                        .cloned()
                        .ok_or(VmError::IndexOutOfBounds(index))?;
                    if !is_slice_scalar(&value) {
                        return Err(VmError::Message(
                            "exclusive slice element is not a supported scalar".into(),
                        ));
                    }
                    self.push(value)?;
                }
                BytecodeInstr::InoutSliceSet(local) => {
                    let value = self.pop()?;
                    let index =
                        usize::try_from(self.pop_i64()?).map_err(|_| VmError::ExpectedIntIndex)?;
                    let (handle, start, end) = self.exclusive_slice_handle(local)?;
                    if index >= end - start {
                        return Err(VmError::IndexOutOfBounds(index));
                    }
                    let existing = self
                        .slice_owner(handle)?
                        .get(start + index)
                        .ok_or(VmError::IndexOutOfBounds(index))?;
                    if !is_slice_scalar(&value)
                        || std::mem::discriminant(existing) != std::mem::discriminant(&value)
                    {
                        return Err(VmError::Message(
                            "exclusive slice write requires matching scalar element type".into(),
                        ));
                    }
                    let owner = self
                        .call_stack
                        .iter_mut()
                        .find(|frame| frame.token == handle.owner_frame)
                        .ok_or_else(|| {
                            VmError::Message("exclusive slice owner is no longer alive".into())
                        })?;
                    let Some(Some(VmValue::Array(buffer))) =
                        owner.locals.get_mut(handle.local_id.0 as usize)
                    else {
                        return Err(VmError::ExpectedArrayOrSlice);
                    };
                    // The owner slot is updated, never the descriptor or a detached copy.
                    // Physical COW preserves value semantics for independent array copies.
                    Rc::make_mut(buffer)[start + index] = value;
                }
                BytecodeInstr::InoutSliceLen(local) => {
                    let (_, start, end) = self.exclusive_slice_handle(local)?;
                    let len = i64::try_from(end - start)
                        .map_err(|_| VmError::Message("exclusive slice length overflow".into()))?;
                    self.push(VmValue::Int64(len))?;
                }
                BytecodeInstr::BorrowLocal(local) | BytecodeInstr::BorrowLocalMut(local) => {
                    let owner = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                    let value = owner
                        .locals
                        .get(local.0 as usize)
                        .ok_or(VmError::LocalOutOfBounds(local.0))?
                        .as_ref()
                        .ok_or(VmError::LocalNotInitialized(local.0))?;
                    let exclusive = matches!(instr, BytecodeInstr::BorrowLocalMut(_));
                    if let VmValue::BorrowedLocal(handle) = value {
                        if !exclusive || !handle.mutable || handle.range.is_some() {
                            return Err(VmError::Message(
                                "cannot reborrow a read-only or invalid exclusive parameter".into(),
                            ));
                        }
                        let handle = *handle;
                        let key = (handle.owner_frame, handle.local_id.0);
                        if !self.exclusive_sources.contains(&key)
                            || !self
                                .call_stack
                                .iter()
                                .any(|frame| frame.token == handle.owner_frame)
                        {
                            return Err(VmError::Message(
                                "exclusive reborrow owner is not active".into(),
                            ));
                        }
                        // The parent call retains the loan. No second owner
                        // token is acquired or released by this nested call.
                        self.push(VmValue::BorrowedLocal(handle))?;
                        continue;
                    }
                    let token = owner.token;
                    let key = (token, local.0);
                    if self.exclusive_sources.contains(&key)
                        || (exclusive && self.borrowed_sources.contains(&key))
                    {
                        return Err(VmError::Message(
                            "conflicting exclusive borrow of local".into(),
                        ));
                    }
                    if exclusive {
                        self.exclusive_sources.insert(key);
                    }
                    self.borrowed_sources.insert(key);
                    self.push(VmValue::BorrowedLocal(VmBorrow {
                        owner_frame: token,
                        local_id: local,
                        mutable: exclusive,
                        range: None,
                    }))?;
                }
                BytecodeInstr::MakeClosureRef(target_id, capture_count)
                | BytecodeInstr::MakeClosureInout(target_id, capture_count) => {
                    let target = self
                        .module
                        .functions
                        .get(target_id.0 as usize)
                        .ok_or_else(|| VmError::UnknownFunction(target_id.to_string()))?;
                    let count = usize::from(capture_count);
                    if count == 0 || count > target.params.len() || target.is_extern {
                        return Err(VmError::Message(
                            "invalid borrowed closure target/capture count".into(),
                        ));
                    }
                    let public_signature = BytecodeSignature {
                        params: target.params[count..].to_vec(),
                        ret: target.ret,
                    };
                    let signature_id = self
                        .module
                        .signatures
                        .iter()
                        .position(|sig| sig == &public_signature)
                        .map(|index| SignatureId(index as u32))
                        .ok_or_else(|| {
                            VmError::Message("borrowed closure signature missing".into())
                        })?;
                    let owner_token = self.call_stack.last().ok_or(VmError::MissingReturn)?.token;
                    let exclusive = matches!(instr, BytecodeInstr::MakeClosureInout(_, _));
                    let mut captures = Vec::with_capacity(count);
                    for _ in 0..count {
                        let raw = self.pop_raw()?;
                        match &raw {
                            VmValue::BorrowedLocal(handle)
                                if handle.owner_frame == owner_token
                                    && handle.mutable == exclusive =>
                            {
                                captures.push(raw)
                            }
                            _ => {
                                return Err(VmError::Message(
                                    "MakeClosureRef requires current-frame borrow handles".into(),
                                ))
                            }
                        }
                    }
                    captures.reverse();
                    self.push(VmValue::Closure(VmClosure {
                        function_id: target_id,
                        signature_id,
                        captures,
                    }))?;
                }
                BytecodeInstr::MakeClosure(target_id, capture_count) => {
                    let target = self
                        .module
                        .functions
                        .get(target_id.0 as usize)
                        .ok_or_else(|| VmError::UnknownFunction(target_id.to_string()))?;
                    let capture_len = usize::from(capture_count);
                    if capture_len > target.params.len() {
                        return Err(VmError::WrongArity {
                            expected: target.params.len() as u16,
                            found: capture_count,
                        });
                    }
                    // The verifier requires the public signature to be in the
                    // module table. Resolve it when constructing the value,
                    // so indirect dispatch only compares compact SignatureIds.
                    let public_signature = BytecodeSignature {
                        params: target.params[capture_len..].to_vec(),
                        ret: target.ret,
                    };
                    let signature_id = self
                        .module
                        .signatures
                        .iter()
                        .position(|signature| signature == &public_signature)
                        .map(|index| SignatureId(index as u32))
                        .ok_or_else(|| {
                            VmError::Message("closure public signature missing from module".into())
                        })?;

                    let mut captures = Vec::with_capacity(capture_len);
                    for _ in 0..capture_len {
                        captures.push(self.pop()?);
                    }
                    captures.reverse();
                    self.push(VmValue::Closure(VmClosure {
                        function_id: target_id,
                        signature_id,
                        captures,
                    }))?;
                }
                BytecodeInstr::CallClosure(signature_id, argc) => {
                    // Stack contract: [..., callable, arg0, ..., argN].
                    let mut args = Vec::with_capacity(usize::from(argc));
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let closure = match self.pop()? {
                        VmValue::Closure(closure) => closure,
                        other => {
                            return Err(VmError::TypeMismatch {
                                expected: "Closure".into(),
                                found: other.type_name().into(),
                            })
                        }
                    };
                    if closure.signature_id != signature_id {
                        return Err(VmError::TypeMismatch {
                            expected: format!("Callable(#{})", signature_id.0),
                            found: format!("Callable(#{})", closure.signature_id.0),
                        });
                    }
                    let (param_count, local_count, max_stack, is_extern) = {
                        let target = self
                            .module
                            .functions
                            .get(closure.function_id.0 as usize)
                            .ok_or_else(|| {
                                VmError::UnknownFunction(closure.function_id.to_string())
                            })?;
                        (
                            target.params.len(),
                            target.locals.len(),
                            target.max_stack as usize,
                            target.is_extern,
                        )
                    };
                    if is_extern {
                        return Err(VmError::Native(
                            "indirect calls to native externs are not supported yet".into(),
                        ));
                    }
                    let supplied = closure.captures.len() + args.len();
                    if supplied != param_count {
                        return Err(VmError::WrongArity {
                            expected: param_count as u16,
                            found: supplied as u16,
                        });
                    }
                    if local_count < supplied {
                        return Err(VmError::LocalOutOfBounds(local_count as u32));
                    }
                    let target = &self.module.functions[closure.function_id.0 as usize];
                    let count = closure.captures.len();
                    let signature = self
                        .module
                        .signatures
                        .get(signature_id.0 as usize)
                        .ok_or_else(|| VmError::Message("invalid closure signature".into()))?;
                    if target.params[count..] != signature.params[..]
                        || target.ret != signature.ret
                        || target.inout_params[count..].iter().any(|access| *access)
                        || closure
                            .captures
                            .iter()
                            .enumerate()
                            .any(|(index, capture)| match capture {
                                VmValue::BorrowedLocal(handle) => {
                                    let key = (handle.owner_frame, handle.local_id.0);
                                    handle.range.is_some()
                                        || handle.mutable != target.inout_params[index]
                                        || !self.borrowed_sources.contains(&key)
                                        || (handle.mutable
                                            && !self.exclusive_sources.contains(&key))
                                        || !self
                                            .call_stack
                                            .iter()
                                            .any(|frame| frame.token == handle.owner_frame)
                                }
                                _ => target.inout_params[index],
                            })
                    {
                        return Err(VmError::Message(
                            "closure access/signature contract mismatch".into(),
                        ));
                    }
                    let mut locals = vec![None; local_count];
                    // Captured parameters are hidden, ordered before the
                    // explicitly supplied arguments.
                    for (index, value) in closure.captures.into_iter().chain(args).enumerate() {
                        locals[index] = Some(value);
                    }
                    self.push_interpreted_frame(closure.function_id, locals, max_stack)?;
                }
                BytecodeInstr::Call(target_id, argc) => {
                    let target_func = self
                        .module
                        .functions
                        .get(target_id.0 as usize)
                        .ok_or_else(|| VmError::UnknownFunction(target_id.to_string()))?;

                    if target_func.is_extern {
                        let mut args = Vec::with_capacity(argc as usize);
                        for _ in 0..argc {
                            args.push(self.pop()?);
                        }
                        args.reverse();

                        let resolver = self.native_resolver.ok_or_else(|| {
                            VmError::Native("native resolver not registered".into())
                        })?;

                        let name = target_func.name.as_str();
                        let last_dot = name.rfind('.');
                        let (mod_name, func_name) = if let Some(i) = last_dot {
                            (&name[..i], &name[i + 1..])
                        } else {
                            ("", name)
                        };

                        let func_ptr = resolver(mod_name, func_name).ok_or_else(|| {
                            VmError::Native(format!(
                                "unresolved native function: {}",
                                target_func.name
                            ))
                        })?;
                        let res = func_ptr(&args).map_err(VmError::Native)?;

                        if target_func.ret != BytecodeType::Unit {
                            self.push(res)?;
                        }
                        continue;
                    }

                    let mut locals = vec![None; target_func.locals.len()];
                    let mut acquired_loans = Vec::new();
                    let mut exclusive_arguments = HashSet::new();
                    let caller_token = self.call_stack.last().ok_or(VmError::MissingReturn)?.token;
                    // Ordinary arguments are materialized by value, while
                    // exclusive parameters preserve the non-owning handle.
                    for i in (0..argc).rev() {
                        let value = if target_func
                            .inout_params
                            .get(i as usize)
                            .copied()
                            .unwrap_or(false)
                        {
                            let raw = self.pop_raw()?;
                            match &raw {
                                VmValue::BorrowedLocal(handle) if handle.mutable => {
                                    let key = (handle.owner_frame, handle.local_id.0);
                                    if handle.range.is_some()
                                        != (target_func.params[i as usize]
                                            == BytecodeType::ExclusiveSlice)
                                        || !exclusive_arguments.insert(key)
                                    {
                                        return Err(VmError::Message(
                                            "exclusive argument kind or owner alias mismatch"
                                                .into(),
                                        ));
                                    }
                                    if !self.exclusive_sources.contains(&key)
                                        || !self
                                            .call_stack
                                            .iter()
                                            .any(|frame| frame.token == handle.owner_frame)
                                    {
                                        return Err(VmError::Message(
                                            "invalid or expired exclusive inout handle".into(),
                                        ));
                                    }
                                    if handle.owner_frame == caller_token {
                                        acquired_loans.push(key);
                                    }
                                }
                                _ => {
                                    return Err(VmError::Message(
                                        "inout parameter requires an exclusive borrow handle"
                                            .into(),
                                    ))
                                }
                            }
                            raw
                        } else {
                            self.pop()?
                        };
                        locals[i as usize] = Some(value);
                    }

                    let callee_max_stack = target_func.max_stack as usize;
                    self.push_interpreted_frame(target_id, locals, callee_max_stack)?;
                    self.call_stack
                        .last_mut()
                        .ok_or(VmError::MissingReturn)?
                        .inout_loans = acquired_loans;
                }

                BytecodeInstr::Return => {
                    let ret_val = if ret_type != BytecodeType::Unit {
                        Some(self.pop()?)
                    } else {
                        None
                    };

                    if let Some(popped) = self.call_stack.pop() {
                        for key in &popped.inout_loans {
                            self.exclusive_sources.remove(key);
                            self.borrowed_sources.remove(key);
                        }
                        self.borrowed_sources
                            .retain(|(frame_id, _)| *frame_id != popped.token);
                        self.exclusive_sources
                            .retain(|(frame_id, _)| *frame_id != popped.token);
                    }

                    if let Some(prev_frame) = self.call_stack.last_mut() {
                        if let Some(val) = ret_val {
                            prev_frame.stack.push(val);
                        }
                    } else {
                        return Ok(ret_val.unwrap_or(VmValue::Unit));
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
                            return Err(VmError::WrongVariant {
                                expected: expected_variant,
                                found: variant,
                            });
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
                BytecodeInstr::MakeTuple(count) => {
                    let mut elements = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.push(VmValue::Tuple(elements))?;
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
                        VmValue::Tuple(elements) => {
                            if index >= elements.len() {
                                return Err(VmError::IndexOutOfBounds(index));
                            }
                            self.push(elements[index].clone())?;
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
                                return Err(VmError::InvalidSliceRange {
                                    start,
                                    end,
                                    len: arr.len(),
                                });
                            }
                            self.push(VmValue::Slice {
                                parent: arr,
                                start,
                                end,
                            })?;
                        }
                        VmValue::Slice {
                            parent,
                            start: p_start,
                            end: p_end,
                        } => {
                            let p_len = p_end - p_start;
                            if start > end || end > p_len {
                                return Err(VmError::InvalidSliceRange {
                                    start,
                                    end,
                                    len: p_len,
                                });
                            }
                            self.push(VmValue::Slice {
                                parent,
                                start: p_start + start,
                                end: p_start + end,
                            })?;
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
                        let bytes: Vec<VmValue> =
                            s.as_bytes().iter().map(|&b| VmValue::UInt8(b)).collect();
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

                BytecodeInstr::AddI64LL(a, b) => {
                    let (left, right) = {
                        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                        let left = frame
                            .locals
                            .get(a.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(a.0))?
                            .as_ref()
                            .ok_or(VmError::LocalNotInitialized(a.0))?
                            .clone();
                        let right = frame
                            .locals
                            .get(b.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(b.0))?
                            .as_ref()
                            .ok_or(VmError::LocalNotInitialized(b.0))?
                            .clone();
                        (left, right)
                    };
                    let left = self.resolve_borrowed_value(left)?;
                    let right = self.resolve_borrowed_value(right)?;
                    match (left, right) {
                        (VmValue::Int64(x), VmValue::Int64(y)) => {
                            self.push(VmValue::Int64(tempra_runtime::add_i64(x, y)?))?;
                        }
                        _ => return Err(VmError::Message("Type mismatch in AddI64LL".into())),
                    }
                }
                BytecodeInstr::AddI64LC(a, id) => {
                    let raw = {
                        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                        frame
                            .locals
                            .get(a.0 as usize)
                            .ok_or(VmError::LocalOutOfBounds(a.0))?
                            .as_ref()
                            .ok_or(VmError::LocalNotInitialized(a.0))?
                            .clone()
                    };
                    let value = self.resolve_borrowed_value(raw)?;
                    let constant = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::Int64(x)) => *x,
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };
                    match value {
                        VmValue::Int64(x) => {
                            self.push(VmValue::Int64(tempra_runtime::add_i64(x, constant)?))?;
                        }
                        _ => return Err(VmError::Message("Type mismatch in AddI64LC".into())),
                    }
                }
            }
        }
    }

    /// Enters a verified bytecode function using the same frame lifecycle for
    /// direct and indirect calls. The caller has already populated parameter
    /// locals and handled native extern dispatch where applicable.
    #[inline]
    fn push_interpreted_frame(
        &mut self,
        function_id: FunctionId,
        locals: Vec<Option<VmValue>>,
        max_stack: usize,
    ) -> Result<(), VmError> {
        let token = self.allocate_frame_token()?;
        if let Some(ref mut profiling) = self.profiling {
            profiling.stats[function_id.0 as usize].call_count += 1;
        }
        self.call_stack.push(Frame {
            token,
            function_id,
            ip: 0,
            locals,
            stack: Vec::with_capacity(max_stack),
            inout_loans: Vec::new(),
        });
        Ok(())
    }

    #[inline(always)]
    fn push(&mut self, val: VmValue) -> Result<(), VmError> {
        self.call_stack.last_mut().unwrap().stack.push(val);
        Ok(())
    }

    fn allocate_frame_token(&mut self) -> Result<u64, VmError> {
        self.next_frame_token = self
            .next_frame_token
            .checked_add(1)
            .ok_or_else(|| VmError::Message("VM frame token space exhausted".into()))?;
        Ok(self.next_frame_token)
    }

    fn slice_owner(&self, handle: VmBorrow) -> Result<&Vec<VmValue>, VmError> {
        if !handle.mutable
            || !self
                .exclusive_sources
                .contains(&(handle.owner_frame, handle.local_id.0))
        {
            return Err(VmError::Message(
                "exclusive slice loan is not active".into(),
            ));
        }
        let owner = self
            .call_stack
            .iter()
            .find(|frame| frame.token == handle.owner_frame)
            .ok_or_else(|| VmError::Message("exclusive slice owner is no longer alive".into()))?;
        let Some(Some(VmValue::Array(buffer))) = owner.locals.get(handle.local_id.0 as usize)
        else {
            return Err(VmError::ExpectedArrayOrSlice);
        };
        Ok(buffer.as_ref())
    }

    fn exclusive_slice_handle(
        &self,
        local: tempra_bytecode::function::LocalId,
    ) -> Result<(VmBorrow, usize, usize), VmError> {
        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
        let Some(Some(VmValue::BorrowedLocal(handle))) = frame.locals.get(local.0 as usize) else {
            return Err(VmError::Message(
                "exclusive slice parameter requires a live handle".into(),
            ));
        };
        let Some((start, end)) = handle.range else {
            return Err(VmError::Message("expected exclusive slice range".into()));
        };
        if start > end || end > self.slice_owner(*handle)?.len() {
            return Err(VmError::Message(
                "invalid exclusive slice owner bounds".into(),
            ));
        }
        Ok((*handle, start, end))
    }

    fn borrow_slice_mut(
        &mut self,
        local: tempra_bytecode::function::LocalId,
        start: usize,
        end: usize,
    ) -> Result<VmBorrow, VmError> {
        let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
        let value = frame
            .locals
            .get(local.0 as usize)
            .ok_or(VmError::LocalOutOfBounds(local.0))?
            .as_ref()
            .ok_or(VmError::LocalNotInitialized(local.0))?;
        if matches!(value, VmValue::BorrowedLocal(_)) {
            let (mut handle, parent_start, parent_end) = self.exclusive_slice_handle(local)?;
            let len = parent_end - parent_start;
            if start > end || end > len {
                return Err(VmError::InvalidSliceRange { start, end, len });
            }
            handle.range = Some((parent_start + start, parent_start + end));
            return Ok(handle);
        }
        let VmValue::Array(buffer) = value else {
            return Err(VmError::ExpectedArrayOrSlice);
        };
        let len = buffer.len();
        if start > end || end > len {
            return Err(VmError::InvalidSliceRange { start, end, len });
        }
        let token = frame.token;
        let key = (token, local.0);
        if self.borrowed_sources.contains(&key) || self.exclusive_sources.contains(&key) {
            return Err(VmError::Message(
                "conflicting exclusive slice borrow of local".into(),
            ));
        }
        self.exclusive_sources.insert(key);
        self.borrowed_sources.insert(key);
        Ok(VmBorrow {
            owner_frame: token,
            local_id: local,
            mutable: true,
            range: Some((start, end)),
        })
    }

    /// Read a borrowed place as a value, never as another transferable handle.
    fn resolve_borrowed_value(&self, value: VmValue) -> Result<VmValue, VmError> {
        let VmValue::BorrowedLocal(handle) = value else {
            return Ok(value);
        };
        if handle.range.is_some() {
            return Err(VmError::Message(
                "exclusive slice descriptor cannot be materialized".into(),
            ));
        }
        let key = (handle.owner_frame, handle.local_id.0);
        if !self.borrowed_sources.contains(&key)
            || (handle.mutable && !self.exclusive_sources.contains(&key))
        {
            return Err(VmError::Message(
                "borrowed owner loan is no longer active".into(),
            ));
        }
        let frame = self
            .call_stack
            .iter()
            .find(|frame| frame.token == handle.owner_frame)
            .ok_or_else(|| VmError::Message("borrowed owner frame is no longer alive".into()))?;
        let value = frame
            .locals
            .get(handle.local_id.0 as usize)
            .ok_or(VmError::LocalOutOfBounds(handle.local_id.0))?
            .as_ref()
            .ok_or(VmError::LocalNotInitialized(handle.local_id.0))?;
        if matches!(value, VmValue::BorrowedLocal(_)) {
            return Err(VmError::Message(
                "borrowed handle must name an original owner".into(),
            ));
        }
        Ok(value.clone())
    }

    #[inline(always)]
    fn pop_raw(&mut self) -> Result<VmValue, VmError> {
        self.call_stack
            .last_mut()
            .ok_or(VmError::MissingReturn)?
            .stack
            .pop()
            .ok_or(VmError::StackUnderflow)
    }

    #[inline(always)]
    fn pop(&mut self) -> Result<VmValue, VmError> {
        let value = self.pop_raw()?;
        if matches!(&value, VmValue::BorrowedLocal(_)) {
            return Err(VmError::Message(
                "raw borrow handle used as an ordinary operand".into(),
            ));
        }
        Ok(value)
    }

    #[inline(always)]
    fn pop_i64(&mut self) -> Result<i64, VmError> {
        match self.pop()? {
            VmValue::Int64(i) => Ok(i),
            other => Err(VmError::TypeMismatch {
                expected: "Int64".into(),
                found: other.type_name().into(),
            }),
        }
    }

    #[inline(always)]
    fn pop_u64(&mut self) -> Result<u64, VmError> {
        match self.pop()? {
            VmValue::UInt64(u) => Ok(u),
            other => Err(VmError::TypeMismatch {
                expected: "UInt64".into(),
                found: other.type_name().into(),
            }),
        }
    }

    #[inline(always)]
    fn pop_f64(&mut self) -> Result<f64, VmError> {
        match self.pop()? {
            VmValue::Float64(f) => Ok(f),
            other => Err(VmError::TypeMismatch {
                expected: "Float64".into(),
                found: other.type_name().into(),
            }),
        }
    }

    #[inline(always)]
    fn pop_bool(&mut self) -> Result<bool, VmError> {
        match self.pop()? {
            VmValue::Bool(b) => Ok(b),
            other => Err(VmError::TypeMismatch {
                expected: "Bool".into(),
                found: other.type_name().into(),
            }),
        }
    }
}

pub fn run_function(
    module: &BytecodeModule,
    name: &str,
    args: Vec<VmValue>,
) -> Result<VmValue, VmError> {
    let id = module
        .functions
        .iter()
        .position(|f| f.name == name)
        .ok_or_else(|| VmError::UnknownFunction(name.to_string()))?;
    let mut interp = Interpreter::new(module);
    interp.run_function_id(FunctionId(id as u32), args)
}

pub fn run_function_id(
    module: &BytecodeModule,
    id: FunctionId,
    args: Vec<VmValue>,
) -> Result<VmValue, VmError> {
    let mut interp = Interpreter::new(module);
    interp.run_function_id(id, args)
}

pub fn run_function_profiled(
    module: &BytecodeModule,
    name: &str,
    args: Vec<VmValue>,
    options: VmOptions,
) -> Result<(VmValue, VmProfile), VmError> {
    Interpreter::run_function_profiled(module, name, args, options)
}

pub fn run_function_with_options(
    module: &BytecodeModule,
    name: &str,
    args: Vec<VmValue>,
    options: VmOptions,
) -> Result<VmValue, VmError> {
    run_function_with_resolver(module, name, args, options, None)
}

pub fn run_function_with_resolver(
    module: &BytecodeModule,
    name: &str,
    args: Vec<VmValue>,
    options: VmOptions,
    resolver: Option<NativeResolver>,
) -> Result<VmValue, VmError> {
    let id = module
        .functions
        .iter()
        .position(|f| f.name == name)
        .ok_or_else(|| VmError::UnknownFunction(name.to_string()))?;
    let mut interp = Interpreter::with_options(module, options);
    interp.native_resolver = resolver;
    interp.run_function_id(FunctionId(id as u32), args)
}

fn is_slice_scalar(value: &VmValue) -> bool {
    matches!(
        value,
        VmValue::Int64(_)
            | VmValue::UInt64(_)
            | VmValue::UInt8(_)
            | VmValue::Float64(_)
            | VmValue::Bool(_)
    )
}
