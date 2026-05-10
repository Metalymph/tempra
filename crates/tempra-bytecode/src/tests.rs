use crate::bytecode::BytecodeModule;
use crate::constant::BytecodeConstant;
use crate::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use crate::opcode::{BytecodeInstr, stack_effect};
use crate::disasm::disassemble_function;
use crate::verify::{verify_function, BytecodeVerifyError};

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

#[test]
fn verify_const_return_i64_ok() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(42));
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let f_id = module.add_function(func);
    assert!(verify_function(&module, f_id).is_ok());
}

#[test]
fn verify_stack_underflow_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::AddI64);
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::StackUnderflow));
}

#[test]
fn verify_type_mismatch_add_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::ConstBool(true));
    func.code.push(BytecodeInstr::ConstBool(false));
    func.code.push(BytecodeInstr::AddI64);
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::StackTypeMismatch { .. })));
}

#[test]
fn verify_load_uninitialized_local_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.locals.push(BytecodeType::Int64);
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::LocalNotInitialized(LocalId(0))));
}

#[test]
fn verify_store_initializes_local_ok() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(10));
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.locals.push(BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Pop);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let f_id = module.add_function(func);
    assert!(verify_function(&module, f_id).is_ok());
}

#[test]
fn verify_invalid_jump_target_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Jump(crate::opcode::InstrOffset(99)));
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::InvalidJumpTarget(99)));
}

/*
#[test]
fn verify_stack_height_merge_mismatch_error() {
    // ...
}
*/

#[test]
fn verify_return_type_mismatch_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstBool(true));
    func.code.push(BytecodeInstr::Return);
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::ReturnTypeMismatch { .. })));
}

#[test]
fn verify_call_wrong_arity_error() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    let target_id = module.add_function(target);
    
    let mut func = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Call(target_id, 0)); // Expected 1
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::CallArityMismatch { .. })));
}
