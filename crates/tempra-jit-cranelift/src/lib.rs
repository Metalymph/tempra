pub mod abi;
pub mod engine;
pub mod error;
pub mod helpers;
pub mod lower;

pub use abi::{HeapHandle, JitHelperResult, JitStatus, JitValueAbi};
pub use engine::{
    can_jit_function, explain_jit_eligibility, JitEligibility, JitEngine, JitFunctionI64,
    JitFunctionI64HeapAware,
};
pub use error::JitCompileError;
pub use helpers::{JitHelperInfo, JitHelperRegistry, JitHelperSignature};
pub use lower::{compile_function_heap_aware, compile_function_i64, is_heap_requiring};

#[cfg(test)]
mod tests;
