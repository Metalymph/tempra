pub mod error;
pub mod pipeline;

pub use error::DriverError;
pub use pipeline::{
    check_file, check_source, run_file, run_file_with_args, run_source, run_source_with_args,
};
