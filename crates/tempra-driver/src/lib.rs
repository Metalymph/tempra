pub mod cache;
mod compilation;
mod config;
mod contracts;
mod effect_tooling;
mod enforcement;
pub mod error;
pub mod module_graph;
pub mod pipeline;
mod specialized_matching;

pub use compilation::VmCompilation;
pub use config::{discover_stdlib_from_executable, CompilationConfig};
pub use contracts::{
    canonicalize_specialized_denied, ContractBindingError, ContractSource,
    FunctionCapabilityContract, SpecializedDenyBindingError, SpecializedDenyClause,
    SpecializedDenyParameter, SpecializedDenyTarget, SpecializedDenyValue,
    UnsupportedSpecializedTarget,
};
pub use effect_tooling::{
    capability_name, project_effect_diagnostics, EffectContractOrigin, EffectDiagnostic,
    EffectDirectOriginKind, EffectFunctionIdentity, EffectInspection, EffectProjectionError,
    EffectQueryError, EffectRequirementInfo, EffectWitness, EffectWitnessCall, EffectWitnessOrigin,
    FunctionEffectInfo, CAPABILITY_DENIED_CODE,
};
pub use enforcement::{
    CapabilityContractInvariantError, CapabilityContractViolation, SpecializedContractViolation,
};
pub use error::DriverError;
pub use pipeline::{
    check_file, check_file_with_config, check_source, compile_file_to_bytecode_with_analysis,
    compile_file_to_bytecode_with_analysis_with_config, compile_to_bytecode,
    compile_to_bytecode_with_analysis, diagnose_file, diagnose_file_with_config, diagnose_source,
    diagnose_source_with_facts, emit_tir_file, emit_tir_source, extract_symbols_from_source,
    inspect_effects_file, inspect_effects_file_with_config, run_file, run_file_with_args,
    run_file_with_experimental_vm, run_file_with_experimental_vm_jit,
    run_file_with_experimental_vm_jit_entry, run_source, run_source_hir, run_source_vm,
    run_source_vm_profiled, run_source_vm_with_options, run_source_with_args,
    run_source_with_experimental_vm, run_source_with_experimental_vm_jit, AnalysisFactsResult,
};
pub use specialized_matching::{match_specialized_clause, SpecializedMatch};
