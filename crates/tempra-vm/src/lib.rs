pub mod value;
pub mod error;
pub mod interpreter;

#[cfg(test)]
mod tests;

pub use value::VmValue;
pub use error::VmError;
pub use interpreter::{run_function, run_function_id, Interpreter};
