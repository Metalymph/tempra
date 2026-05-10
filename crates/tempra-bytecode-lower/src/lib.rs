pub mod error;
pub mod lower;

pub use error::BytecodeLowerError;
pub use lower::lower_tir_module;

#[cfg(test)]
mod tests;
