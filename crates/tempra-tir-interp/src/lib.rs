pub mod error;
pub mod interp;
pub mod value;

pub use error::TirInterpError;
pub use interp::interpret_function;
pub use value::TirRuntimeValue;

#[cfg(test)]
mod tests;
