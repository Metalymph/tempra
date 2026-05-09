use crate::bytecode::BytecodeModule;
use crate::constant::BytecodeConstant;
use crate::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use crate::opcode::{BytecodeInstr, stack_effect};
use crate::disasm::disassemble_function;

#[test]
fn test_create_module_with_constant() {
    let mut module = BytecodeModule::new();
    let id = module.add_constant(BytecodeConstant::Int64(42));
    assert_eq!(id.0, 0);
    assert_eq!(module.constants[0], BytecodeConstant::Int64(42));
}

#[test]
fn test_create_function_add_i64() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "add".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64
    );
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    
    let id = module.add_function(func);
    assert_eq!(id.0, 0);
    assert_eq!(module.functions[0].name, "add");
}

#[test]
fn test_stack_effect_add_i64() {
    let module = BytecodeModule::new();
    let func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::AddI64;
    let effect = stack_effect(&instr, &module, &func).unwrap();
    assert_eq!(effect.pops, 2);
    assert_eq!(effect.pushes, 1);
}

#[test]
fn test_stack_effect_call_unit() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    let target_id = module.add_function(target);
    
    let caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::Call(target_id, 1);
    let effect = stack_effect(&instr, &module, &caller).unwrap();
    assert_eq!(effect.pops, 1);
    assert_eq!(effect.pushes, 0);
}

#[test]
fn test_stack_effect_call_value() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    let target_id = module.add_function(target);
    
    let caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::Call(target_id, 1);
    let effect = stack_effect(&instr, &module, &caller).unwrap();
    assert_eq!(effect.pops, 1);
    assert_eq!(effect.pushes, 1);
}

#[test]
fn test_disassemble_simple_function() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(100));
    
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    let f_id = module.add_function(func);

    let disasm = disassemble_function(&module, f_id).unwrap();
    assert!(disasm.contains("fn test () -> Int64"));
    assert!(disasm.contains("ConstI64 #0"));
    assert!(disasm.contains("Return"));
}

#[test]
fn test_invalid_function_disassemble_error() {
    let module = BytecodeModule::new();
    let result = disassemble_function(&module, FunctionId(99));
    assert!(result.is_err());
}
