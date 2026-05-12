pub mod value;
pub mod error;
pub mod interpreter;
pub mod profile;

#[cfg(test)]
mod tests;

pub use value::VmValue;
pub use error::VmError;
pub use interpreter::{run_function, run_function_id, run_function_profiled, Interpreter};
pub use profile::{VmOptions, VmProfile, FunctionProfile};
