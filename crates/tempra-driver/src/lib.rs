pub mod cache;
mod compilation;
mod contracts;
pub mod error;
pub mod module_graph;
pub mod pipeline;

pub use compilation::VmCompilation;
pub use contracts::{ContractBindingError, ContractSource, FunctionCapabilityContract};
pub use error::DriverError;
pub use pipeline::{
    check_file, check_source, compile_file_to_bytecode_with_analysis, compile_to_bytecode,
    compile_to_bytecode_with_analysis, diagnose_file, diagnose_source, diagnose_source_with_facts,
    emit_tir_file, emit_tir_source, extract_symbols_from_source, run_file, run_file_with_args,
    run_file_with_experimental_vm, run_file_with_experimental_vm_jit, run_source, run_source_hir,
    run_source_vm, run_source_vm_profiled, run_source_vm_with_options, run_source_with_args,
    run_source_with_experimental_vm, run_source_with_experimental_vm_jit, AnalysisFactsResult,
};
