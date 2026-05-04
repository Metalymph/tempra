use crate::error::DriverError;
use std::fs;
use std::path::Path;
use tempra_hir::lower_file;
use tempra_interpreter::run;
use tempra_parser::parse_source;
use tempra_typeck::check_file as typeck_check_file;

pub fn run_source(source: &str) -> Result<(), DriverError> {
    run_source_with_args(source, &[])
}

pub fn run_source_with_args(source: &str, args: &[String]) -> Result<(), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let value = tempra_interpreter::run_with_args(&hir, args.to_vec()).map_err(|e| match e {
        tempra_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
        tempra_interpreter::RuntimeError::Crash(msg) => {
            DriverError::RuntimeError(format!("crash: {}", msg))
        }
        tempra_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
    })?;

    if let tempra_interpreter::value::Value::Result(res) = value {
        if let tempra_interpreter::value::ValueResult::Err(err_val) = *res {
            return Err(DriverError::RuntimeError(format!(
                "Uncaught error: {}",
                err_val
            )));
        }
    }

    Ok(())
}

pub fn run_file(path: &Path) -> Result<(), DriverError> {
    run_file_with_args(path, &[])
}

pub fn run_file_with_args(path: &Path, args: &[String]) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    run_source_with_args(&source, args)
}

pub fn check_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    Ok(())
}

pub fn check_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    check_source(&source)
}

pub fn test_source(source: &str) -> Result<(), DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    typeck_check_file(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let value = run(&hir).map_err(|e| match e {
        tempra_interpreter::RuntimeError::Exit(code) => DriverError::Exit(code),
        tempra_interpreter::RuntimeError::Crash(msg) => {
            DriverError::RuntimeError(format!("test failed: {}", msg))
        }
        tempra_interpreter::RuntimeError::Message(s) => DriverError::RuntimeError(s),
    })?;

    if let tempra_interpreter::value::Value::Result(res) = value {
        if let tempra_interpreter::value::ValueResult::Err(err_val) = *res {
            return Err(DriverError::RuntimeError(format!(
                "Uncaught error: {}",
                err_val
            )));
        }
    }

    Ok(())
}

pub fn test_file(path: &Path) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    test_source(&source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_source_basic() {
        let source = "
        fn main() {
            let x = 1;
            print(x)
        }
        ";
        let result = run_source(source);
        assert!(
            result.is_ok(),
            "Expected success, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_check_source_success() {
        let source = "
        fn main() {
            let x = 1;
            print(x)
        }
        ";
        let result = check_source(source);
        assert!(
            result.is_ok(),
            "Expected success, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_check_source_type_error() {
        let source = "
        fn main() {
            let x = fs.read(123);
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

    #[test]
    fn test_test_source_passing() {
        let source = "
        fn main() {
            expect(2 + 2 == 4)
            expect(fmt(\"a\", 1) == \"a1\")
        }
        ";
        let result = test_source(source);
        assert!(
            result.is_ok(),
            "Expected test to pass, got {:?}",
            result.err().unwrap()
        );
    }

    #[test]
    fn test_test_source_failing() {
        let source = "
        fn main() {
            expect(false, \"boom\")
        }
        ";
        let result = test_source(source);
        assert!(matches!(result, Err(DriverError::RuntimeError(msg)) if msg.contains("boom")));
    }
}
