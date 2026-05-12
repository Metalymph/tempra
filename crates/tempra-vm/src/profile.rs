use tempra_bytecode::function::FunctionId;
use tempra_bytecode::opcode::BYTECODE_OPCODE_COUNT;

#[derive(Debug, Clone, Copy)]
pub struct VmOptions {
    pub profiling: bool,
    pub call_threshold: u64,
    pub backedge_threshold: u64,
}

impl Default for VmOptions {
    fn default() -> Self {
        Self {
            profiling: false,
            call_threshold: 1000,
            backedge_threshold: 10000,
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
            writeln!(s, "  {} calls={} instrs={} backedges={} hot=true", 
                f.function_name, f.call_count, f.instruction_count, f.backedge_count).unwrap();
        }

        writeln!(s, "\nGlobal Top Opcodes:").unwrap();
        let mut opcodes: Vec<_> = self.global_opcode_counts.iter()
            .enumerate()
            .filter(|(_, &count)| count > 0)
            .collect();
        opcodes.sort_by_key(|(_, &count)| std::cmp::Reverse(count));

        for (id, &count) in opcodes.into_iter().take(10) {
            let name = self.get_opcode_name(id);
            writeln!(s, "  {}: {}", name, count).unwrap();
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
}

#[derive(Debug, Clone)]
pub(crate) struct FunctionStats {
    pub call_count: u64,
    pub instruction_count: u64,
    pub backedge_count: u64,
    pub opcode_counts: Vec<u64>,
}

impl Default for FunctionStats {
    fn default() -> Self {
        Self {
            call_count: 0,
            instruction_count: 0,
            backedge_count: 0,
            opcode_counts: vec![0; BYTECODE_OPCODE_COUNT],
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
