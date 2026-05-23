pub mod error;
pub mod interpreter;
pub mod profile;
pub mod value;

#[cfg(test)]
mod jit_tests;
#[cfg(test)]
mod tests;

pub use error::VmError;
pub use interpreter::{
    run_function, run_function_id, run_function_profiled, run_function_with_options, run_function_with_resolver, Interpreter,
};
pub use profile::{FunctionProfile, VmOptions, VmProfile};
pub use value::VmValue;
