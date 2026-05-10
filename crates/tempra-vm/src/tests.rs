use super::*;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::constant::BytecodeConstant;

#[test]
fn vm_const_return_i64() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(42));
    
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    let result = run_function(&module, "test", vec![]).unwrap();
    assert_eq!(result, VmValue::Int64(42));
}

#[test]
fn vm_add_i64() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let args = vec![VmValue::Int64(10), VmValue::Int64(20)];
    let result = run_function(&module, "add", args).unwrap();
    assert_eq!(result, VmValue::Int64(30));
}

#[test]
fn vm_bool_and() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("and".into(), vec![BytecodeType::Bool, BytecodeType::Bool], BytecodeType::Bool);
    func.locals = vec![BytecodeType::Bool, BytecodeType::Bool];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::And);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let result = run_function(&module, "and", vec![VmValue::Bool(true), VmValue::Bool(false)]).unwrap();
    assert_eq!(result, VmValue::Bool(false));
}

#[test]
fn vm_if_condbr() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c2 = module.add_constant(BytecodeConstant::Int64(2));
    
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Bool], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Bool];
    // 0: LoadLocal 0
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: JumpIfFalse 4
    func.code.push(BytecodeInstr::JumpIfFalse(tempra_bytecode::opcode::InstrOffset(4)));
    // 2: Const 1, Return
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::Return);
    // 4: Const 2, Return
    func.code.push(BytecodeInstr::ConstI64(c2));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    assert_eq!(run_function(&module, "test", vec![VmValue::Bool(true)]).unwrap(), VmValue::Int64(1));
    assert_eq!(run_function(&module, "test", vec![VmValue::Bool(false)]).unwrap(), VmValue::Int64(2));
}

#[test]
fn vm_slot_load_store() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    func.locals.push(BytecodeType::Int64); // param at 0
    func.locals.push(BytecodeType::Int64); // temp slot at 1
    
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    assert_eq!(run_function(&module, "test", vec![VmValue::Int64(99)]).unwrap(), VmValue::Int64(99));
}

#[test]
fn vm_call_simple() {
    let mut module = BytecodeModule::new();
    
    // fn double(x: Int64) -> Int64 { x + x }
    let mut double = BytecodeFunction::new("double".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    double.locals = vec![BytecodeType::Int64];
    double.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    double.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    double.code.push(BytecodeInstr::AddI64);
    double.code.push(BytecodeInstr::Return);
    double.max_stack = 2;
    let double_id = module.add_function(double);

    // fn main() -> Int64 { double(21) }
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    let c_21 = module.add_constant(BytecodeConstant::Int64(21));
    main.code.push(BytecodeInstr::ConstI64(c_21));
    main.code.push(BytecodeInstr::Call(double_id, 1));
    main.code.push(BytecodeInstr::Return);
    main.max_stack = 1;
    module.add_function(main);

    assert_eq!(run_function(&module, "main", vec![]).unwrap(), VmValue::Int64(42));
}

#[test]
fn vm_division_by_zero_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::DivI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let result = run_function(&module, "test", vec![]);
    assert_eq!(result, Err(VmError::DivisionByZero));
}

#[test]
fn vm_unreachable_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Unreachable);
    module.add_function(func);

    let result = run_function(&module, "test", vec![]);
    assert_eq!(result, Err(VmError::UnreachableExecuted));
}
