use crate::error::DriverError;
use std::fs;
use std::path::Path;
use tempra_hir::lower_file;
use tempra_hir::HirItem;
use tempra_interpreter::run;
use tempra_parser::parse_source;
use tempra_typeck::check_file as typeck_check_file;

pub fn run_source(source: &str) -> Result<(), DriverError> {
    run_source_with_args(source, &[])
}

pub fn run_source_with_experimental_vm(source: &str, args: &[String]) -> Result<(), DriverError> {
    if !args.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm("CLI arguments are not yet supported in experimental VM".into()));
    }

    let hash = crate::cache::compute_source_hash(source);
    let cache_path = crate::cache::get_cache_path(&hash);
    let fingerprint = format!("tempra-{}", env!("CARGO_PKG_VERSION"));

    let module = if let Ok(artifact) = crate::cache::read_cache_artifact(&cache_path) {
        if artifact.header.compiler_fingerprint == fingerprint && artifact.header.source_hash == hash {
            artifact.module
        } else {
            compile_and_cache_module(source, &hash, &cache_path, &fingerprint)?
        }
    } else {
        compile_and_cache_module(source, &hash, &cache_path, &fingerprint)?
    };

    let has_main = module.functions.iter().any(|f| f.name == "main");
    if !has_main {
        return Err(DriverError::UnsupportedExperimentalVm("experimental VM currently requires main()".into()));
    }

    match tempra_vm::run_function(&module, "main", vec![]) {
        Ok(_) => Ok(()),
        Err(e) => Err(DriverError::VmError(format!("{:?}", e))),
    }
}

fn compile_and_cache_module(source: &str, hash: &[u8; 32], cache_path: &std::path::Path, fingerprint: &str) -> Result<tempra_bytecode::BytecodeModule, DriverError> {
    let ast = tempra_parser::parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    let mut hir = tempra_hir::lower_file(source, &ast);
    let type_ctx = tempra_typeck::check_file_with_context(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut errors = Vec::new();
    for item in &hir.items {
        if let tempra_hir::HirItem::Function(func) = item {
            match tempra_tir_lower::lower::lower_function(func, &type_ctx, &mut module_builder) {
                Ok(tir_func) => { module_builder.add_function(tir_func); }
                Err(e) => { errors.push(format!("unsupported in fn '{}': {}", func.name, e)); }
            }
        }
    }
    if !errors.is_empty() {
        return Err(DriverError::UnsupportedExperimentalVm(errors.join("\n")));
    }

    let bc_module = tempra_bytecode_lower::lower_tir_module(&module_builder.module)
        .map_err(|e| DriverError::UnsupportedExperimentalVm(format!("bytecode lowering error: {:?}", e)))?;

    tempra_bytecode::verify::verify_module(&bc_module)
        .map_err(|e| DriverError::UnsupportedExperimentalVm(format!("verification error: {:?}", e)))?;

    let mut header = tempra_bytecode::BytecodeCacheHeader::default();
    header.compiler_fingerprint = fingerprint.to_string();
    header.source_hash = *hash;

    let artifact = tempra_bytecode::BytecodeCacheArtifact {
        header,
        module: bc_module.clone(),
    };

    if let Err(e) = crate::cache::write_cache_artifact(cache_path, &artifact) {
        eprintln!("Warning: failed to write cache artifact: {}", e);
    }

    Ok(bc_module)
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

pub fn run_file_with_experimental_vm(path: &Path, args: &[String]) -> Result<(), DriverError> {
    let source = fs::read_to_string(path)?;
    run_source_with_experimental_vm(&source, args)
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

pub fn emit_tir_source(source: &str) -> Result<String, DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;

    let mut hir = lower_file(source, &ast);

    let type_ctx = tempra_typeck::check_file_with_context(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();

    let mut errors = Vec::new();

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            match tempra_tir_lower::lower::lower_function(func, &type_ctx, &mut module_builder) {
                Ok(tir_func) => {
                    module_builder.add_function(tir_func);
                }
                Err(e) => {
                    errors.push(format!("emit-tir unsupported in fn '{}': {}", func.name, e));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(DriverError::TirLowerError(errors.join("\n")));
    }

    let module = module_builder.module;

    if let Err(verifier_errors) = tempra_tir::verifier::verify_module(&module) {
        let msgs: Vec<String> = verifier_errors.iter().map(|e| format!("internal TIR verifier error: {:?}", e)).collect();
        return Err(DriverError::TirLowerError(msgs.join("\n")));
    }

    Ok(tempra_tir::pretty::format_tir_module(&module))
}

pub fn emit_tir_file(path: &Path) -> Result<String, DriverError> {
    let source = fs::read_to_string(path)?;
    emit_tir_source(&source)
}

pub fn interpret_tir_source(
    source: &str,
    function_name: &str,
    args: Vec<tempra_tir_interp::TirRuntimeValue>,
) -> Result<tempra_tir_interp::TirRuntimeValue, DriverError> {
    let ast = parse_source(source).map_err(|e| DriverError::ParseError(format!("{:?}", e)))?;
    let mut hir = lower_file(source, &ast);
    let type_ctx = tempra_typeck::check_file_with_context(&mut hir).map_err(|errs| DriverError::TypeError(format!("{:?}", errs)))?;

    let mut module_builder = tempra_tir::builder::ModuleBuilder::new();
    let mut errors = Vec::new();

    for item in &hir.items {
        if let HirItem::Function(func) = item {
            match tempra_tir_lower::lower::lower_function(func, &type_ctx, &mut module_builder) {
                Ok(tir_func) => {
                    module_builder.add_function(tir_func);
                }
                Err(e) => {
                    errors.push(format!("lower error in fn '{}': {}", func.name, e));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(DriverError::TirLowerError(errors.join("\n")));
    }

    let module = module_builder.module;
    tempra_tir_interp::interpret_function(&module, function_name, args)
        .map_err(DriverError::TirInterpError)
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

    #[test]
    fn test_emit_tir_simple_add() {
        let source = "
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        ";
        let result = emit_tir_source(source);
        assert!(result.is_ok(), "Expected TIR output, got {:?}", result.err().unwrap());
        let tir = result.unwrap();
        assert!(tir.contains("fn add"));
        assert!(tir.contains("add %"));
        assert!(tir.contains("ret %"));
    }

    #[test]
    fn test_emit_tir_unsupported_for_error() {
        let source = "
        fn main() {
            for i in [1, 2, 3] {
                print(i)
            }
        }
        ";
        let result = emit_tir_source(source);
        assert!(matches!(result, Err(DriverError::TirLowerError(_))));
    }

    #[test]
    fn test_tir_interp_source_const_return() {
        let source = "fn main() -> Int64 { return 42 }";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(42));
    }

    #[test]
    fn test_tir_interp_source_add_function() {
        let source = "
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            return add(10, 32)
        }
        ";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(42));
    }

    #[test]
    fn test_tir_interp_source_param_identity() {
        let source = "fn id(x: Int64) -> Int64 { return x }";
        let result = interpret_tir_source(source, "id", vec![tempra_tir_interp::TirRuntimeValue::Int64(99)]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(99));
    }

    #[test]
    fn test_tir_interp_source_if_return() {
        let source = "
        fn test(cond: Bool) -> Int64 {
            if cond {
                return 1
            } else {
                return 0
            }
        }
        ";
        let res_t = interpret_tir_source(source, "test", vec![tempra_tir_interp::TirRuntimeValue::Bool(true)]);
        assert_eq!(res_t.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(1));
        
        let res_f = interpret_tir_source(source, "test", vec![tempra_tir_interp::TirRuntimeValue::Bool(false)]);
        assert_eq!(res_f.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(0));
    }

    #[test]
    fn test_tir_interp_source_var_assignment() {
        let source = "
        fn test() -> Int64 {
            var x = 10
            x = 20
            return x
        }
        ";
        let result = interpret_tir_source(source, "test", vec![]);
        assert_eq!(result.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(20));
    }

    #[test]
    fn test_tir_interp_source_call_simple() {
        let source = "
        fn double(x: Int64) -> Int64 { return x * 2 }
        fn test() -> Int64 { return double(21) }
        ";
        let result = interpret_tir_source(source, "test", vec![]);
        assert_eq!(result.unwrap(), tempra_tir_interp::TirRuntimeValue::Int64(42));
    }

    #[test]
    fn test_tir_interp_source_unsupported_for_error() {
        let source = "
        fn main() {
            for i in [1, 2, 3] {}
        }
        ";
        let result = interpret_tir_source(source, "main", vec![]);
        assert!(matches!(result, Err(DriverError::TirLowerError(_))));
    }

    #[test]
    fn test_tir_interp_unknown_function_error() {
        let source = "fn main() {}";
        let result = interpret_tir_source(source, "ghost", vec![]);
        assert!(matches!(result, Err(DriverError::TirInterpError(tempra_tir_interp::TirInterpError::UnknownFunction(_)))));
    }
}
