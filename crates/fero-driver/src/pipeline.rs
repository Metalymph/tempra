use std::fs;
use std::path::Path;
use crate::error::DriverError;
use fero_parser::parse_source;
use fero_hir::lower_file;
use fero_typeck::check_file;
use fero_interpreter::run;

pub fn run_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source)
        .map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    
    let hir = lower_file(source, &ast);
    
    check_file(&hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;
    
    let _value = run(&hir)
        .map_err(|e| DriverError::RuntimeError(e))?;
    
    Ok(())
}

pub fn run_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    run_source(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_source_basic() {
        let source = "
        fn main() {
            let x = 1;
            x
        }
        ";
        let result = run_source(source);
        assert!(result.is_ok(), "Expected success, got {:?}", result.err().unwrap());
    }
}
