pub mod engine;
pub mod lower;
pub mod error;

pub use engine::{JitEngine, JitFunctionI64, can_jit_function, JitEligibility, explain_jit_eligibility};
pub use lower::compile_function_i64;
pub use error::JitCompileError;

#[cfg(test)]
mod tests;
