use crate::error::VmError;
use crate::profile::{FunctionProfile, ProfilingState, VmOptions, VmProfile};
use crate::value::VmValue;
use std::rc::Rc;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::function::{BytecodeType, FunctionId};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::verify::verify_module;

#[cfg(feature = "jit-cranelift")]
use std::collections::HashMap;
#[cfg(feature = "jit-cranelift")]
use tempra_jit_cranelift::helpers::{JitHeapScalar, JitRuntimeContext};
#[cfg(feature = "jit-cranelift")]
use tempra_jit_cranelift::{
    compile_function_heap_aware, is_heap_requiring, JitEngine, JitFunctionI64,
    JitFunctionI64HeapAware,
};

pub struct Frame {
    pub function_id: FunctionId,
    pub ip: usize,
    pub locals: Vec<Option<VmValue>>,
    pub stack: Vec<VmValue>,
}

pub type NativeResolver = fn(&str, &str) -> Option<fn(&[VmValue]) -> Result<VmValue, String>>;

pub struct Interpreter<'a> {
    pub module: &'a BytecodeModule,
    pub call_stack: Vec<Frame>,
    pub(crate) profiling: Option<ProfilingState>,
    pub native_resolver: Option<NativeResolver>,

    #[cfg(feature = "jit-cranelift")]
    pub(crate) jit_engine: Option<JitEngine>,
    /// Cache for pure-Int64 JIT functions (no heap params/locals).
    #[cfg(feature = "jit-cranelift")]
    pub(crate) jit_cache: HashMap<FunctionId, JitFunctionI64>,
    /// Cache for heap-aware JIT functions (String/Array params or Len/Size/GetIndex opcodes).
    #[cfg(feature = "jit-cranelift")]
    pub(crate) jit_cache_heap: HashMap<FunctionId, JitFunctionI64HeapAware>,
}

impl<'a> Interpreter<'a> {
    pub fn new(module: &'a BytecodeModule) -> Self {
        Self {
            module,
            call_stack: Vec::with_capacity(32),
            profiling: None,
            native_resolver: None,
            #[cfg(feature = "jit-cranelift")]
            jit_engine: None,
            #[cfg(feature = "jit-cranelift")]
            jit_cache: HashMap::new(),
            #[cfg(feature = "jit-cranelift")]
            jit_cache_heap: HashMap::new(),
        }
    }

    pub fn with_options(module: &'a BytecodeModule, options: VmOptions) -> Self {
        let profiling = if options.profiling || options.jit_enabled {
            Some(ProfilingState::new(module.functions.len(), options))
        } else {
            None
        };

        #[cfg(feature = "jit-cranelift")]
        let (jit_engine, jit_cache) = if options.jit_enabled {
            (Some(JitEngine::new()), HashMap::new())
        } else {
            (None, HashMap::new())
        };

        Self {
            module,
            call_stack: Vec::with_capacity(32),
            profiling,
            native_resolver: None,
            #[cfg(feature = "jit-cranelift")]
            jit_engine,
            #[cfg(feature = "jit-cranelift")]
            jit_cache,
            #[cfg(feature = "jit-cranelift")]
            jit_cache_heap: HashMap::new(),
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

        if func.params.len() != args.len() {
            return Err(VmError::WrongArity {
                expected: func.params.len() as u16,
                found: args.len() as u16,
            });
        }

        #[cfg(feature = "jit-cranelift")]
        if self
            .profiling
            .as_ref()
            .map(|p| p.options.jit_enabled)
            .unwrap_or(false)
        {
            // ── Heap-aware JIT dispatch ──────────────────────────────────────────
            if let Some(jit_fn) = self.jit_cache_heap.get(&function_id).copied() {
                let mut ctx = JitRuntimeContext::new();
                let mut i64_args: Vec<i64> = Vec::with_capacity(args.len());
                let mut marshal_ok = true;
                for arg in &args {
                    match vm_value_to_jit_handle(arg, &mut ctx) {
                        Some(h) => i64_args.push(h),
                        None => {
                            marshal_ok = false;
                            break;
                        }
                    }
                }
                let mut returned_err = false;
                if marshal_ok {
                    let before_helpers = ctx.helper_calls;
                    let before_allocs = ctx.allocation_count();

                    let start_native = std::time::Instant::now();
                    match jit_fn.call_with_ctx(&mut ctx as *mut JitRuntimeContext, &i64_args) {
                        Ok(res) => {
                            let elapsed = start_native.elapsed().as_nanos() as u64;
                            let delta_helpers = ctx.helper_calls - before_helpers;
                            let delta_allocs = ctx.allocation_count() - before_allocs;
                            if let Some(ref mut prof) = self.profiling {
                                let stats = &mut prof.stats[function_id.0 as usize];
                                stats.jit_runtime_calls += 1;
                                stats.jit_heap_runtime_calls += 1;
                                stats.jit_heap_helper_calls += delta_helpers;
                                stats.jit_heap_handle_allocations += delta_allocs;
                                stats.jit_heap_context_objects += ctx.object_count() as u64;
                                stats.jit_heap_handle_cache_hits += ctx.cache_hits;
                                stats.jit_heap_handle_cache_misses += ctx.cache_misses;
                                stats.jit_total_native_time_ns += elapsed;
                            }
                            return Ok(VmValue::Int64(res));
                        }
                        Err(_) => {
                            returned_err = true;
                        }
                    }
                }
                if let Some(ref mut prof) = self.profiling {
                    let stats = &mut prof.stats[function_id.0 as usize];
                    stats.jit_fallback_calls += 1;
                    stats.jit_heap_fallbacks += 1;
                    if returned_err || !marshal_ok {
                        stats.jit_heap_errors += 1;
                    }
                }
            }
            // ── Pure-i64 JIT dispatch ────────────────────────────────────────────
            else if let Some(jit_fn) = self.jit_cache.get(&function_id).copied() {
                let mut i64_args = Vec::with_capacity(args.len());
                let mut compatible = true;
                for arg in &args {
                    if let VmValue::Int64(i) = arg {
                        i64_args.push(*i);
                    } else {
                        compatible = false;
                        break;
                    }
                }
                if compatible {
                    let start_native = std::time::Instant::now();
                    if let Ok(res) = jit_fn.call_i64(&i64_args) {
                        let elapsed = start_native.elapsed().as_nanos() as u64;
                        if let Some(ref mut prof) = self.profiling {
                            prof.stats[function_id.0 as usize].jit_runtime_calls += 1;
                            prof.stats[function_id.0 as usize].jit_total_native_time_ns += elapsed;
                        }
                        return Ok(VmValue::Int64(res));
                    }
                }
                if let Some(ref mut prof) = self.profiling {
                    prof.stats[function_id.0 as usize].jit_fallback_calls += 1;
                }
            }
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

        if let Some(ref mut prof) = self.profiling {
            prof.stats[function_id.0 as usize].call_count += 1;

            #[cfg(feature = "jit-cranelift")]
            self.maybe_compile_jit(function_id);
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
                jit_compile_attempts: s.jit_compile_attempts,
                jit_compile_successes: s.jit_compile_successes,
                jit_compile_failures: s.jit_compile_failures,
                jit_runtime_calls: s.jit_runtime_calls,
                jit_fallback_calls: s.jit_fallback_calls,
                jit_rejection_reason: s.jit_rejection_reason.clone(),
                jit_compile_time_ns: s.jit_compile_time_ns,
                jit_total_native_time_ns: s.jit_total_native_time_ns,
                jit_rejected_count: s.jit_rejected_count,
                jit_heap_runtime_calls: s.jit_heap_runtime_calls,
                jit_heap_helper_calls: s.jit_heap_helper_calls,
                jit_heap_handle_allocations: s.jit_heap_handle_allocations,
                jit_heap_context_objects: s.jit_heap_context_objects,
                jit_heap_fallbacks: s.jit_heap_fallbacks,
                jit_heap_errors: s.jit_heap_errors,
                jit_heap_handle_cache_hits: s.jit_heap_handle_cache_hits,
                jit_heap_handle_cache_misses: s.jit_heap_handle_cache_misses,
                jit_skipped_below_threshold: s.jit_skipped_below_threshold,
                jit_skipped_too_small: s.jit_skipped_too_small,
                jit_skipped_retry_limit: s.jit_skipped_retry_limit,
                jit_permanent_rejections: s.jit_permanent_rejections,
                jit_retryable_failures: s.jit_retryable_failures,
                jit_is_permanently_rejected: s.jit_is_permanently_rejected,
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
            41 => "GetIndex",
            42 => "Slice",
            43 => "Len",
            44 => "Size",
            45 => "Bytes",
            46 => "MakeStruct",
            47 => "GetField",
            48 => "SetField",
            49 => "ArrayPush",
            50 => "AddI64LL",
            51 => "AddI64LC",
            _ => "Unknown",
        }
    }

    #[cfg(feature = "jit-cranelift")]
    fn maybe_compile_jit(&mut self, function_id: FunctionId) {
        // Skip if already compiled (either cache)
        if self.jit_cache.contains_key(&function_id)
            || self.jit_cache_heap.contains_key(&function_id)
        {
            return;
        }

        let mut attempt_needed = false;
        if let Some(ref mut prof) = self.profiling {
            let stats = &mut prof.stats[function_id.0 as usize];

            if !prof.options.jit_enabled {
                return;
            }

            if stats.jit_is_permanently_rejected {
                return;
            }

            let is_hot = stats.call_count >= prof.options.hot_call_threshold
                || stats.backedge_count >= prof.options.hot_backedge_threshold;
            if !is_hot {
                stats.jit_skipped_below_threshold += 1;
                return;
            }

            let func_len = self.module.functions[function_id.0 as usize].code.len() as u64;
            if func_len < prof.options.jit_min_instruction_count {
                stats.jit_skipped_too_small += 1;
                return;
            }

            if stats.jit_compile_failures >= prof.options.jit_retry_limit as u64 {
                stats.jit_skipped_retry_limit += 1;
                return;
            }

            attempt_needed = true;
        }

        if !attempt_needed {
            return;
        }

        let start_time = std::time::Instant::now();

        let eligibility = tempra_jit_cranelift::explain_jit_eligibility(self.module, function_id);
        if let Some(ref mut prof) = self.profiling {
            prof.stats[function_id.0 as usize].jit_compile_attempts += 1;
            if !matches!(eligibility, tempra_jit_cranelift::JitEligibility::Eligible) {
                prof.stats[function_id.0 as usize].jit_rejection_reason =
                    Some(eligibility.to_string());
                prof.stats[function_id.0 as usize].jit_rejected_count += 1;
                prof.stats[function_id.0 as usize].jit_permanent_rejections += 1;
                prof.stats[function_id.0 as usize].jit_is_permanently_rejected = true;
                let elapsed = start_time.elapsed().as_nanos() as u64;
                prof.stats[function_id.0 as usize].jit_compile_time_ns += elapsed;
                return;
            }
        }

        // Determine compile path: heap-aware or pure-i64
        let heap_required = is_heap_requiring(&self.module.functions[function_id.0 as usize]);

        if let Some(ref mut engine) = self.jit_engine {
            let mut compiling = std::collections::HashSet::new();

            if heap_required {
                // ── Heap-aware compile path ───────────────────────────────────────
                match compile_function_heap_aware(
                    engine,
                    self.module,
                    function_id,
                    &mut self.jit_cache_heap,
                    &mut compiling,
                ) {
                    Ok(compiled) => {
                        self.jit_cache_heap.insert(function_id, compiled);
                        let elapsed = start_time.elapsed().as_nanos() as u64;
                        if let Some(ref mut prof) = self.profiling {
                            prof.stats[function_id.0 as usize].jit_compile_successes += 1;
                            prof.stats[function_id.0 as usize].jit_compile_time_ns += elapsed;
                        }
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed().as_nanos() as u64;
                        if let Some(ref mut prof) = self.profiling {
                            prof.stats[function_id.0 as usize].jit_compile_failures += 1;
                            prof.stats[function_id.0 as usize].jit_rejection_reason =
                                Some(format!("Heap compile failed: {:?}", e));
                            prof.stats[function_id.0 as usize].jit_compile_time_ns += elapsed;
                            let failures = prof.stats[function_id.0 as usize].jit_compile_failures;
                            if failures >= prof.options.jit_retry_limit as u64 {
                                prof.stats[function_id.0 as usize].jit_is_permanently_rejected =
                                    true;
                                prof.stats[function_id.0 as usize].jit_permanent_rejections += 1;
                            } else {
                                prof.stats[function_id.0 as usize].jit_retryable_failures += 1;
                            }
                        }
                    }
                }
            } else {
                // ── Pure-i64 compile path ─────────────────────────────────────────
                let old_keys: Vec<tempra_bytecode::function::FunctionId> =
                    self.jit_cache.keys().copied().collect();
                match tempra_jit_cranelift::compile_function_i64(
                    engine,
                    self.module,
                    function_id,
                    &mut self.jit_cache,
                    &mut compiling,
                ) {
                    Ok(compiled) => {
                        self.jit_cache.insert(function_id, compiled);
                        let elapsed = start_time.elapsed().as_nanos() as u64;
                        if let Some(ref mut prof) = self.profiling {
                            for new_key in self.jit_cache.keys() {
                                if !old_keys.contains(new_key) && *new_key != function_id {
                                    prof.stats[new_key.0 as usize].jit_compile_attempts += 1;
                                    prof.stats[new_key.0 as usize].jit_compile_successes += 1;
                                }
                            }
                            prof.stats[function_id.0 as usize].jit_compile_successes += 1;
                            prof.stats[function_id.0 as usize].jit_compile_time_ns += elapsed;
                        }
                    }
                    Err(e) => {
                        let elapsed = start_time.elapsed().as_nanos() as u64;
                        if let Some(ref mut prof) = self.profiling {
                            prof.stats[function_id.0 as usize].jit_compile_failures += 1;
                            prof.stats[function_id.0 as usize].jit_rejection_reason =
                                Some(format!("Compilation failed: {:?}", e));
                            prof.stats[function_id.0 as usize].jit_compile_time_ns += elapsed;
                            let failures = prof.stats[function_id.0 as usize].jit_compile_failures;
                            if failures >= prof.options.jit_retry_limit as u64 {
                                prof.stats[function_id.0 as usize].jit_is_permanently_rejected =
                                    true;
                                prof.stats[function_id.0 as usize].jit_permanent_rejections += 1;
                            } else {
                                prof.stats[function_id.0 as usize].jit_retryable_failures += 1;
                            }
                        }
                    }
                }
            }
        } else {
            if let Some(ref mut prof) = self.profiling {
                prof.stats[function_id.0 as usize].jit_compile_failures += 1;
                prof.stats[function_id.0 as usize].jit_rejection_reason =
                    Some("JIT engine not available".into());
            }
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
                let target_func = &self.module.functions[frame.function_id.0 as usize];
                let instr = &target_func.code[frame.ip];

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
                    self.push(val)?;
                }
                BytecodeInstr::StoreLocal(id) => {
                    let val = self.pop()?;
                    let frame = self.call_stack.last_mut().ok_or(VmError::MissingReturn)?;
                    if id.0 as usize >= frame.locals.len() {
                        return Err(VmError::LocalOutOfBounds(id.0));
                    }
                    frame.locals[id.0 as usize] = Some(val);
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
                    if b == 0 {
                        return Err(VmError::DivisionByZero);
                    }
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
                    if b == 0 {
                        return Err(VmError::DivisionByZero);
                    }
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
                    if let Some(ref mut prof) = self.profiling {
                        if offset.0 <= current_ip {
                            let stats = &mut prof.stats[frame_func_id.0 as usize];
                            stats.backedge_count += 1;

                            #[cfg(feature = "jit-cranelift")]
                            self.maybe_compile_jit(frame_func_id);
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

                                #[cfg(feature = "jit-cranelift")]
                                self.maybe_compile_jit(frame_func_id);
                            }
                        }
                        self.call_stack.last_mut().ok_or(VmError::MissingReturn)?.ip = offset.0;
                    }
                }

                BytecodeInstr::Call(target_id, argc) => {
                    #[cfg(feature = "jit-cranelift")]
                    if self
                        .profiling
                        .as_ref()
                        .map(|p| p.options.jit_enabled)
                        .unwrap_or(false)
                    {
                        // ── Heap-aware JIT call ──────────────────────────────────────────
                        if let Some(jit_fn) = self.jit_cache_heap.get(&target_id).copied() {
                            let mut args = Vec::with_capacity(argc as usize);
                            for _ in 0..argc {
                                args.push(self.pop()?);
                            }
                            args.reverse();

                            let mut ctx = JitRuntimeContext::new();
                            let mut i64_args: Vec<i64> = Vec::with_capacity(argc as usize);
                            let mut marshal_ok = true;
                            for arg in &args {
                                match vm_value_to_jit_handle(arg, &mut ctx) {
                                    Some(h) => i64_args.push(h),
                                    None => {
                                        marshal_ok = false;
                                        break;
                                    }
                                }
                            }

                            let mut success = false;
                            let mut returned_err = false;
                            let mut delta_helpers = 0;
                            let mut delta_allocs = 0;
                            let mut final_objects = 0;

                            if marshal_ok {
                                let before_helpers = ctx.helper_calls;
                                let before_allocs = ctx.allocation_count();

                                match jit_fn
                                    .call_with_ctx(&mut ctx as *mut JitRuntimeContext, &i64_args)
                                {
                                    Ok(res) => {
                                        self.push(VmValue::Int64(res))?;
                                        success = true;
                                        delta_helpers = ctx.helper_calls - before_helpers;
                                        delta_allocs = ctx.allocation_count() - before_allocs;
                                        final_objects = ctx.object_count() as u64;
                                    }
                                    Err(_) => {
                                        returned_err = true;
                                    }
                                }
                            }

                            if let Some(ref mut prof) = self.profiling {
                                let stats = &mut prof.stats[target_id.0 as usize];
                                if success {
                                    stats.jit_runtime_calls += 1;
                                    stats.jit_heap_runtime_calls += 1;
                                    stats.jit_heap_helper_calls += delta_helpers;
                                    stats.jit_heap_handle_allocations += delta_allocs;
                                    stats.jit_heap_context_objects += final_objects;
                                    stats.jit_heap_handle_cache_hits += ctx.cache_hits;
                                    stats.jit_heap_handle_cache_misses += ctx.cache_misses;
                                } else {
                                    stats.jit_fallback_calls += 1;
                                    stats.jit_heap_fallbacks += 1;
                                    if returned_err || !marshal_ok {
                                        stats.jit_heap_errors += 1;
                                    }
                                }
                            }

                            if success {
                                continue;
                            }
                            // Fallback: put args back on stack
                            for arg in args {
                                self.push(arg)?;
                            }
                        }
                        // ── Pure-i64 JIT call ────────────────────────────────────────────
                        else if let Some(jit_fn) = self.jit_cache.get(&target_id).copied() {
                            let mut args = Vec::with_capacity(argc as usize);
                            for _ in 0..argc {
                                args.push(self.pop()?);
                            }
                            args.reverse();

                            let mut i64_args = Vec::with_capacity(argc as usize);
                            let mut compatible = true;
                            for arg in &args {
                                if let VmValue::Int64(i) = arg {
                                    i64_args.push(*i);
                                } else {
                                    compatible = false;
                                    break;
                                }
                            }

                            let mut success = false;
                            if compatible {
                                if let Ok(res) = jit_fn.call_i64(&i64_args) {
                                    self.push(VmValue::Int64(res))?;
                                    success = true;
                                }
                            }

                            if let Some(ref mut prof) = self.profiling {
                                if success {
                                    prof.stats[target_id.0 as usize].jit_runtime_calls += 1;
                                } else {
                                    prof.stats[target_id.0 as usize].jit_fallback_calls += 1;
                                }
                            }

                            if success {
                                continue;
                            }
                            // Fallback: put args back on stack
                            for arg in args {
                                self.push(arg)?;
                            }
                        } else {
                            self.maybe_compile_jit(target_id);
                        }
                    }

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
                        let res = func_ptr(&args).map_err(|e| VmError::Native(e))?;

                        if target_func.ret != BytecodeType::Unit {
                            self.push(res)?;
                        }
                        continue;
                    }

                    let mut locals = vec![None; target_func.locals.len()];
                    // Pop arguments directly into locals in reverse order (right-to-left)
                    for i in (0..argc).rev() {
                        locals[i as usize] = Some(self.pop()?);
                    }

                    if let Some(ref mut prof) = self.profiling {
                        prof.stats[target_id.0 as usize].call_count += 1;
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
                    let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                    let val_a = frame
                        .locals
                        .get(a.0 as usize)
                        .ok_or(VmError::LocalOutOfBounds(a.0))?
                        .as_ref()
                        .ok_or(VmError::LocalNotInitialized(a.0))?;
                    let val_b = frame
                        .locals
                        .get(b.0 as usize)
                        .ok_or(VmError::LocalOutOfBounds(b.0))?
                        .as_ref()
                        .ok_or(VmError::LocalNotInitialized(b.0))?;

                    if let (VmValue::Int64(ia), VmValue::Int64(ib)) = (val_a, val_b) {
                        let res = VmValue::Int64(ia + ib);
                        self.push(res)?;
                    } else {
                        return Err(VmError::Message("Type mismatch in AddI64LL".into()));
                    }
                }
                BytecodeInstr::AddI64LC(a, id) => {
                    let frame = self.call_stack.last().ok_or(VmError::MissingReturn)?;
                    let val_a = frame
                        .locals
                        .get(a.0 as usize)
                        .ok_or(VmError::LocalOutOfBounds(a.0))?
                        .as_ref()
                        .ok_or(VmError::LocalNotInitialized(a.0))?;

                    let constant = match self.module.constants.get(id.0 as usize) {
                        Some(BytecodeConstant::Int64(i)) => *i,
                        _ => return Err(VmError::InvalidConstant(id.0)),
                    };

                    if let VmValue::Int64(ia) = val_a {
                        let res = VmValue::Int64(ia + constant);
                        self.push(res)?;
                    } else {
                        return Err(VmError::Message("Type mismatch in AddI64LC".into()));
                    }
                }
            }
        }
    }

    #[inline(always)]
    fn push(&mut self, val: VmValue) -> Result<(), VmError> {
        self.call_stack.last_mut().unwrap().stack.push(val);
        Ok(())
    }

    #[inline(always)]
    fn pop(&mut self) -> Result<VmValue, VmError> {
        self.call_stack
            .last_mut()
            .unwrap()
            .stack
            .pop()
            .ok_or(VmError::StackUnderflow)
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

/// Marshals a `VmValue` into an i64 suitable for passing to a heap-aware JIT function.
///
/// - `VmValue::Int64(i)`  → the integer itself, no allocation needed.
/// - `VmValue::String(s)` → allocates a string handle in `ctx`, returns handle as i64.
/// - `VmValue::Array(arr)` where every element is `Int64` → allocates an array handle
///   containing `JitHeapScalar::Int64` elements, returns handle as i64.
///
/// Returns `None` for any type that cannot be represented across the JIT boundary
/// (Bool, UInt64, Float64, Slice, Enum, Struct, heterogeneous arrays). The caller
/// must fall back to the interpreter in that case.
#[cfg(feature = "jit-cranelift")]
fn vm_value_to_jit_handle(value: &VmValue, ctx: &mut JitRuntimeContext) -> Option<i64> {
    match value {
        VmValue::Int64(i) => Some(*i),
        VmValue::String(s) => {
            let handle = ctx.alloc_or_get_string(s);
            Some(handle.0 as i64)
        }
        VmValue::Array(arr) => {
            let mut values = Vec::with_capacity(arr.len());
            for elem in arr.iter() {
                match elem {
                    VmValue::Int64(i) => values.push(*i),
                    _ => return None, // heterogeneous or unsupported element type
                }
            }
            let handle = ctx.alloc_or_get_array_int64(&values);
            Some(handle.0 as i64)
        }
        _ => None,
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
