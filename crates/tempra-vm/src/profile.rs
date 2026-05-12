use tempra_bytecode::function::FunctionId;

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
}

#[derive(Debug, Clone)]
pub struct VmProfile {
    pub function_stats: Vec<FunctionProfile>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FunctionStats {
    pub call_count: u64,
    pub instruction_count: u64,
    pub backedge_count: u64,
}

pub(crate) struct ProfilingState {
    pub stats: Vec<FunctionStats>,
    pub options: VmOptions,
}

impl ProfilingState {
    pub fn new(num_functions: usize, options: VmOptions) -> Self {
        Self {
            stats: vec![FunctionStats::default(); num_functions],
            options,
        }
    }
}
