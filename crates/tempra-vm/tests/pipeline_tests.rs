use tempra_vm::*;
use tempra_parser::parse_source;
use tempra_hir::{lower_file, HirItem};
use tempra_typeck::check_file_with_context;
use tempra_tir_lower::lower::lower_function as lower_tir_function;
use tempra_tir::builder::ModuleBuilder;
use tempra_bytecode_lower::lower_tir_module;

fn run_source(source: &str, entry: &str, args: Vec<VmValue>) -> VmValue {
    let ast = parse_source(source).expect("Parse error");
    let mut hir = lower_file(source, &ast);
    let type_ctx = check_file_with_context(&mut hir).expect("Typecheck error");
    
    let mut tir_builder = ModuleBuilder::new();
    for item in &hir.items {
        if let HirItem::Function(func) = item {
            let tir_func = lower_tir_function(func, &type_ctx, &mut tir_builder).expect("TIR lower error");
            tir_builder.add_function(tir_func);
        }
    }
    
    let bc_module = lower_tir_module(&tir_builder.module).expect("Bytecode lower error");
    
    run_function(&bc_module, entry, args).expect("VM execution error")
}

#[test]
fn test_pipeline_add() {
    let source = "fn add(a: Int64, b: Int64) -> Int64 { return a + b; }";
    let res = run_source(source, "add", vec![VmValue::Int64(10), VmValue::Int64(20)]);
    assert_eq!(res, VmValue::Int64(30));
}

#[test]
fn test_pipeline_if() {
    let source = "fn abs(x: Int64) -> Int64 { if x < 0 { return -x; } else { return x; } }";
    assert_eq!(run_source(source, "abs", vec![VmValue::Int64(10)]), VmValue::Int64(10));
    assert_eq!(run_source(source, "abs", vec![VmValue::Int64(-5)]), VmValue::Int64(5));
}

#[test]
fn test_pipeline_vars() {
    let source = "fn test(x: Int64) -> Int64 { var y = x + 1; return y * 2; }";
    assert_eq!(run_source(source, "test", vec![VmValue::Int64(10)]), VmValue::Int64(22));
}
