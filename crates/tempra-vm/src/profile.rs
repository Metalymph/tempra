use tempra_bytecode::function::FunctionId;
use tempra_bytecode::opcode::BYTECODE_OPCODE_COUNT;

#[derive(Debug, Clone, Copy)]
pub struct VmOptions {
    pub profiling: bool,
    pub jit_enabled: bool,
    pub hot_call_threshold: u64,
    pub hot_backedge_threshold: u64,
    pub jit_min_instruction_count: u64,
    pub jit_retry_limit: u8,
}

impl Default for VmOptions {
    fn default() -> Self {
        Self {
            profiling: false,
            jit_enabled: false,
            hot_call_threshold: 100,
            hot_backedge_threshold: 1000,
            jit_min_instruction_count: 8,
            jit_retry_limit: 1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionProfile {
    pub function_id: FunctionId,
    pub function_name: String,
    pub call_count: u64,
    pub instruction_count: u64,
    pub backedge_count: u64,
    pub is_hot: bool,
    pub top_opcodes: Vec<(&'static str, u64)>,

    // JIT Stats
    pub jit_compile_attempts: u64,
    pub jit_compile_successes: u64,
    pub jit_compile_failures: u64,
    pub jit_runtime_calls: u64,
    pub jit_fallback_calls: u64,
    pub jit_rejection_reason: Option<String>,
    pub jit_compile_time_ns: u64,
    pub jit_total_native_time_ns: u64,
    pub jit_rejected_count: u64,

    // JIT Heap Stats
    pub jit_heap_runtime_calls: u64,
    pub jit_heap_helper_calls: u64,
    pub jit_heap_handle_allocations: u64,
    pub jit_heap_context_objects: u64,
    pub jit_heap_fallbacks: u64,
    pub jit_heap_errors: u64,
    pub jit_heap_handle_cache_hits: u64,
    pub jit_heap_handle_cache_misses: u64,

    // JIT Policy Stats
    pub jit_skipped_below_threshold: u64,
    pub jit_skipped_too_small: u64,
    pub jit_skipped_retry_limit: u64,
    pub jit_permanent_rejections: u64,
    pub jit_retryable_failures: u64,
    pub jit_is_permanently_rejected: bool,
}

#[derive(Debug, Clone)]
pub struct VmProfile {
    pub function_stats: Vec<FunctionProfile>,
    pub global_opcode_counts: Vec<u64>,
}

impl VmProfile {
    pub fn summary(&self) -> String {
        use std::fmt::Write;
        let mut s = String::new();

        writeln!(s, "Hot Functions:").unwrap();
        let mut hot_funcs: Vec<_> = self.function_stats.iter().filter(|f| f.is_hot).collect();
        hot_funcs.sort_by_key(|f| std::cmp::Reverse(f.instruction_count));

        for f in hot_funcs {
            writeln!(
                s,
                "  {} calls={} instrs={} backedges={} hot=true",
                f.function_name, f.call_count, f.instruction_count, f.backedge_count
            )
            .unwrap();
        }

        writeln!(s, "\nGlobal Top Opcodes:").unwrap();
        let mut opcodes: Vec<_> = self
            .global_opcode_counts
            .iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .collect();
        opcodes.sort_by_key(|(_, &count)| std::cmp::Reverse(count));

        for (id, &count) in opcodes.into_iter().take(10) {
            let name = self.get_opcode_name(id);
            writeln!(s, "  {}: {}", name, count).unwrap();
        }

        let total_attempts: u64 = self
            .function_stats
            .iter()
            .map(|f| f.jit_compile_attempts)
            .sum();
        if total_attempts > 0 {
            writeln!(s, "\nJIT Summary:").unwrap();
            let total_successes: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_compile_successes)
                .sum();
            let total_failures: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_compile_failures)
                .sum();
            let total_runtime: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_runtime_calls)
                .sum();
            let total_fallback: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_fallback_calls)
                .sum();
            let total_compile_time_ns: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_compile_time_ns)
                .sum();
            let total_native_time_ns: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_total_native_time_ns)
                .sum();
            let total_rejected_count: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_rejected_count)
                .sum();

            writeln!(s, "  Compile attempts: {}", total_attempts).unwrap();
            writeln!(s, "  Successes:        {}", total_successes).unwrap();
            writeln!(s, "  Failures:         {}", total_failures).unwrap();
            writeln!(s, "  Runtime calls:    {}", total_runtime).unwrap();
            writeln!(s, "  Fallback calls:   {}", total_fallback).unwrap();
            writeln!(s, "  Rejected count:   {}", total_rejected_count).unwrap();
            writeln!(s, "  Compile time:     {} ns", total_compile_time_ns).unwrap();
            writeln!(s, "  Native runtime:   {} ns", total_native_time_ns).unwrap();

            let total_heap_runtime: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_runtime_calls)
                .sum();
            let total_heap_helper: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_helper_calls)
                .sum();
            let total_heap_allocations: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_handle_allocations)
                .sum();
            let total_heap_objects: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_context_objects)
                .sum();
            let total_heap_fallbacks: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_fallbacks)
                .sum();
            let total_heap_errors: u64 =
                self.function_stats.iter().map(|f| f.jit_heap_errors).sum();
            let total_heap_hits: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_handle_cache_hits)
                .sum();
            let total_heap_misses: u64 = self
                .function_stats
                .iter()
                .map(|f| f.jit_heap_handle_cache_misses)
                .sum();

            writeln!(s, "  Heap runtime:     {}", total_heap_runtime).unwrap();
            writeln!(s, "  Heap helper:      {}", total_heap_helper).unwrap();
            writeln!(s, "  Heap allocations: {}", total_heap_allocations).unwrap();
            writeln!(s, "  Heap context objs:{}", total_heap_objects).unwrap();
            writeln!(s, "  Heap fallbacks:   {}", total_heap_fallbacks).unwrap();
            writeln!(s, "  Heap errors:      {}", total_heap_errors).unwrap();
            writeln!(s, "  Heap cache hits:  {}", total_heap_hits).unwrap();
            writeln!(s, "  Heap cache misses:{}", total_heap_misses).unwrap();

            let rejected: Vec<_> = self
                .function_stats
                .iter()
                .filter(|f| f.jit_rejection_reason.is_some())
                .collect();

            if !rejected.is_empty() {
                writeln!(s, "\nJIT Rejections:").unwrap();
                for f in rejected {
                    writeln!(
                        s,
                        "  function {}: {}",
                        f.function_name,
                        f.jit_rejection_reason.as_ref().unwrap()
                    )
                    .unwrap();
                }
            }
        }

        let total_skipped_threshold: u64 = self
            .function_stats
            .iter()
            .map(|f| f.jit_skipped_below_threshold)
            .sum();
        let total_skipped_small: u64 = self
            .function_stats
            .iter()
            .map(|f| f.jit_skipped_too_small)
            .sum();
        let total_skipped_retry: u64 = self
            .function_stats
            .iter()
            .map(|f| f.jit_skipped_retry_limit)
            .sum();
        let total_permanent_rejections: u64 = self
            .function_stats
            .iter()
            .map(|f| f.jit_permanent_rejections)
            .sum();

        if total_skipped_threshold > 0
            || total_skipped_small > 0
            || total_skipped_retry > 0
            || total_permanent_rejections > 0
        {
            writeln!(s, "\nJIT Policy:").unwrap();
            writeln!(s, "  skipped below threshold: {}", total_skipped_threshold).unwrap();
            writeln!(s, "  skipped too small:       {}", total_skipped_small).unwrap();
            writeln!(s, "  retry-limit skips:       {}", total_skipped_retry).unwrap();
            writeln!(
                s,
                "  permanent rejections:    {}",
                total_permanent_rejections
            )
            .unwrap();
        }

        s
    }

    fn get_opcode_name(&self, id: usize) -> &'static str {
        // We need a dummy instr to get the name if we don't want to duplicate the list
        // Or we can just use a static lookup table here.
        // For simplicity, we'll assume the IDs match our helper.
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
            _ => "Unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionStats {
    pub call_count: u64,
    pub instruction_count: u64,
    pub backedge_count: u64,
    pub opcode_counts: Vec<u64>,

    // JIT Stats
    pub jit_compile_attempts: u64,
    pub jit_compile_successes: u64,
    pub jit_compile_failures: u64,
    pub jit_runtime_calls: u64,
    pub jit_fallback_calls: u64,
    pub jit_rejection_reason: Option<String>,
    pub jit_compile_time_ns: u64,
    pub jit_total_native_time_ns: u64,
    pub jit_rejected_count: u64,

    // JIT Heap Stats
    pub jit_heap_runtime_calls: u64,
    pub jit_heap_helper_calls: u64,
    pub jit_heap_handle_allocations: u64,
    pub jit_heap_context_objects: u64,
    pub jit_heap_fallbacks: u64,
    pub jit_heap_errors: u64,
    pub jit_heap_handle_cache_hits: u64,
    pub jit_heap_handle_cache_misses: u64,

    // JIT Policy Stats
    pub jit_skipped_below_threshold: u64,
    pub jit_skipped_too_small: u64,
    pub jit_skipped_retry_limit: u64,
    pub jit_permanent_rejections: u64,
    pub jit_retryable_failures: u64,
    pub jit_is_permanently_rejected: bool,
}

impl Default for FunctionStats {
    fn default() -> Self {
        Self {
            call_count: 0,
            instruction_count: 0,
            backedge_count: 0,
            opcode_counts: vec![0; BYTECODE_OPCODE_COUNT],
            jit_compile_attempts: 0,
            jit_compile_successes: 0,
            jit_compile_failures: 0,
            jit_runtime_calls: 0,
            jit_fallback_calls: 0,
            jit_rejection_reason: None,
            jit_compile_time_ns: 0,
            jit_total_native_time_ns: 0,
            jit_rejected_count: 0,
            jit_heap_runtime_calls: 0,
            jit_heap_helper_calls: 0,
            jit_heap_handle_allocations: 0,
            jit_heap_context_objects: 0,
            jit_heap_fallbacks: 0,
            jit_heap_errors: 0,
            jit_heap_handle_cache_hits: 0,
            jit_heap_handle_cache_misses: 0,
            jit_skipped_below_threshold: 0,
            jit_skipped_too_small: 0,
            jit_skipped_retry_limit: 0,
            jit_permanent_rejections: 0,
            jit_retryable_failures: 0,
            jit_is_permanently_rejected: false,
        }
    }
}

pub(crate) struct ProfilingState {
    pub stats: Vec<FunctionStats>,
    pub global_opcode_counts: Vec<u64>,
    pub options: VmOptions,
}

impl ProfilingState {
    pub fn new(num_functions: usize, options: VmOptions) -> Self {
        Self {
            stats: vec![FunctionStats::default(); num_functions],
            global_opcode_counts: vec![0; BYTECODE_OPCODE_COUNT],
            options,
        }
    }
}
