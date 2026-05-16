pub mod engine;
pub mod lower;
pub mod error;
pub mod abi;
pub mod helpers;

pub use engine::{JitEngine, JitFunctionI64, can_jit_function, JitEligibility, explain_jit_eligibility};
pub use lower::compile_function_i64;
pub use error::JitCompileError;
pub use abi::{HeapHandle, JitStatus, JitValueAbi, JitHelperResult};
pub use helpers::{JitHelperRegistry, JitHelperSignature, JitHelperInfo};

#[cfg(test)]
mod tests;
