use std::fs;
use std::path::Path;
use crate::error::DriverError;
use fero_parser::parse_source;
use fero_hir::lower_file;
use fero_typeck::check_file as typeck_check_file;
use fero_interpreter::run;

pub fn run_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source)
        .map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    
    let hir = lower_file(source, &ast);
    
    typeck_check_file(&hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;
    
    let value = run(&hir)
        .map_err(|e| match e {
            fero_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
            fero_interpreter::RuntimeError::Panic(msg) => DriverError::RuntimeError(format!("panic: {}", msg)),
            fero_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
        })?;
    
    if let fero_interpreter::value::Value::Result(res) = value {
        if let fero_interpreter::value::ValueResult::Err(err_val) = *res {
            return Err(DriverError::RuntimeError(format!("Uncaught error: {}", err_val)));
        }
    }
    
    Ok(())
}

pub fn run_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    run_source(&source)
}

pub fn check_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source)
        .map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    
    let hir = lower_file(source, &ast);
    
    typeck_check_file(&hir)
        .map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;
    
    Ok(())
}

pub fn check_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    check_source(&source)
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

    #[test]
    fn test_check_source_success() {
        let source = "
        fn main() {
            let x = 1;
            x
        }
        ";
        let result = check_source(source);
        assert!(result.is_ok(), "Expected success, got {:?}", result.err().unwrap());
    }

    #[test]
    fn test_check_source_type_error() {
        let source = "
        fn main() {
            let x = read(123);
        }
        ";
        let result = check_source(source);
        assert!(matches!(result, Err(DriverError::TypeError(_))));
    }

    #[test]
    fn test_check_source_parse_error() {
        let source = "
        fn main( {
            let x = 1;
        }
        ";
        let result = check_source(source);
        assert!(matches!(result, Err(DriverError::ParseError(_))));
    }
}
