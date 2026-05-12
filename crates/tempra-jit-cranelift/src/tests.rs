use crate::engine::{JitEngine, can_jit_function};
use crate::lower::compile_function_i64;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId};
use tempra_bytecode::opcode::{BytecodeInstr, InstrOffset};
use tempra_bytecode::constant::BytecodeConstant;

#[test]
fn jit_const_return_i64() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(42));
    
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    assert!(can_jit_function(&module, id));
    
    let jit_fn = compile_function_i64(&mut engine, &module, id).unwrap();
    let res = jit_fn.call_i64(&[]).unwrap();
    assert_eq!(res, 42);
}

#[test]
fn jit_add_two_params() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id).unwrap();
    let res = jit_fn.call_i64(&[10, 20]).unwrap();
    assert_eq!(res, 30);
}

#[test]
fn jit_if_else() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(100));
    let c2 = module.add_constant(BytecodeConstant::Int64(200));
    
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64];
    
    // if x == 0 { 100 } else { 200 }
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::Eq);
    // Jump to else if false
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(6)));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::Return);
    // Else:
    func.code.push(BytecodeInstr::ConstI64(c2));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id).unwrap();
    
    assert_eq!(jit_fn.call_i64(&[0]).unwrap(), 100);
    assert_eq!(jit_fn.call_i64(&[1]).unwrap(), 200);
}

#[test]
fn jit_loop_sum() {
    let mut module = BytecodeModule::new();
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    
    let mut func = BytecodeFunction::new("sum".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64, BytecodeType::Int64]; 
    // locals: 0=n, 1=i, 2=acc
    
    // acc = 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(2)));
    // i = 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    
    // Loop header (IP 4)
    // if i < n { continue } else { break }
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LtI64);
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(17)));
    
    // acc = acc + i
    func.code.push(BytecodeInstr::LoadLocal(LocalId(2)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(2)));
    
    // i = i + 1
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    
    // Jump back to header
    func.code.push(BytecodeInstr::Jump(InstrOffset(4)));
    
    // Break target (IP 17)
    func.code.push(BytecodeInstr::LoadLocal(LocalId(2)));
    func.code.push(BytecodeInstr::Return);
    
    func.max_stack = 2;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id).unwrap();
    
    // sum(10) = 0+1+2+3+4+5+6+7+8+9 = 45
    assert_eq!(jit_fn.call_i64(&[10]).unwrap(), 45);
}

#[test]
fn jit_rejects_string_function() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::String], BytecodeType::Int64);
    func.locals = vec![BytecodeType::String];
    func.code.push(BytecodeInstr::Return);
    let id = module.add_function(func);

    assert!(!can_jit_function(&module, id));
}
