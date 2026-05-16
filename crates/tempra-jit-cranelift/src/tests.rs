use crate::engine::{JitEngine, can_jit_function};
use crate::lower::compile_function_i64;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId, FunctionId};
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
    
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
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
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
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
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
    
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
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
    
    // sum(10) = 0+1+2+3+4+5+6+7+8+9 = 45
    assert_eq!(jit_fn.call_i64(&[10]).unwrap(), 45);
}

#[test]
fn jit_bool_local_lt_branch() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c2 = module.add_constant(BytecodeConstant::Int64(2));
    
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64, BytecodeType::Bool];
    
    // 0: Load x
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: Load y
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    // 2: Lt
    func.code.push(BytecodeInstr::LtI64);
    // 3: Store b
    func.code.push(BytecodeInstr::StoreLocal(LocalId(2)));
    
    // 4: Load b
    func.code.push(BytecodeInstr::LoadLocal(LocalId(2)));
    // 5: JumpIfFalse to offset 8
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(8)));
    // 6: return 1
    func.code.push(BytecodeInstr::ConstI64(c1));
    // 7: return
    func.code.push(BytecodeInstr::Return);
    // 8: return 2
    func.code.push(BytecodeInstr::ConstI64(c2));
    // 9: return
    func.code.push(BytecodeInstr::Return);
    
    func.max_stack = 2;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
    
    assert_eq!(jit_fn.call_i64(&[10, 20]).unwrap(), 1);
    assert_eq!(jit_fn.call_i64(&[20, 10]).unwrap(), 2);
}

#[test]
fn jit_rejects_bool_param() {
    let mut module = BytecodeModule::new();
    let func = BytecodeFunction::new("test".into(), vec![BytecodeType::Bool], BytecodeType::Int64);
    let id = module.add_function(func);
    
    assert!(!can_jit_function(&module, id));
    let eligibility = crate::engine::explain_jit_eligibility(&module, id);
    assert!(eligibility.to_string().contains("Bool signatures are not supported"));
}

#[test]
fn jit_rejects_bool_return() {
    let mut module = BytecodeModule::new();
    let func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64], BytecodeType::Bool);
    let id = module.add_function(func);
    
    assert!(!can_jit_function(&module, id));
    let eligibility = crate::engine::explain_jit_eligibility(&module, id);
    assert!(eligibility.to_string().contains("Bool signatures are not supported"));
}

#[test]
fn jit_nested_if_else() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c2 = module.add_constant(BytecodeConstant::Int64(2));
    let c3 = module.add_constant(BytecodeConstant::Int64(3));
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    let c10 = module.add_constant(BytecodeConstant::Int64(10));
    
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64];
    
    // 0: Load x
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: Load 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    // 2: Gt
    func.code.push(BytecodeInstr::GtI64);
    // 3: JumpIfFalse to offset 12 (Outer Else)
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(12)));
    
    // 4: Load x
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 5: Load 10
    func.code.push(BytecodeInstr::ConstI64(c10));
    // 6: Gt
    func.code.push(BytecodeInstr::GtI64);
    // 7: JumpIfFalse to offset 10 (Inner Else)
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(10)));
    // 8: return 3
    func.code.push(BytecodeInstr::ConstI64(c3));
    // 9: return
    func.code.push(BytecodeInstr::Return);
    // 10: return 2
    func.code.push(BytecodeInstr::ConstI64(c2));
    // 11: return
    func.code.push(BytecodeInstr::Return);
    
    // Outer Else:
    // 12: return 1
    func.code.push(BytecodeInstr::ConstI64(c1));
    // 13: return
    func.code.push(BytecodeInstr::Return);
    
    func.max_stack = 2;
    let id = module.add_function(func);

    let mut engine = JitEngine::new();
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    let jit_fn = compile_function_i64(&mut engine, &module, id, &mut jit_cache, &mut compiling).unwrap();
    
    assert_eq!(jit_fn.call_i64(&[-5]).unwrap(), 1);
    assert_eq!(jit_fn.call_i64(&[5]).unwrap(), 2);
    assert_eq!(jit_fn.call_i64(&[15]).unwrap(), 3);
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

#[test]
fn jit_call_add_function() {
    let mut module = BytecodeModule::new();
    
    // fn add(a: Int64, b: Int64) -> Int64 { a + b }
    let mut add_fn = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    add_fn.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    add_fn.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    add_fn.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    add_fn.code.push(BytecodeInstr::AddI64);
    add_fn.code.push(BytecodeInstr::Return);
    add_fn.max_stack = 2;
    let add_id = module.add_function(add_fn);

    // fn main() -> Int64 { add(20, 22) }
    let mut main_fn = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    let c20 = module.add_constant(BytecodeConstant::Int64(20));
    let c22 = module.add_constant(BytecodeConstant::Int64(22));
    main_fn.code.push(BytecodeInstr::ConstI64(c20));
    main_fn.code.push(BytecodeInstr::ConstI64(c22));
    main_fn.code.push(BytecodeInstr::Call(add_id, 2));
    main_fn.code.push(BytecodeInstr::Return);
    main_fn.max_stack = 2;
    let main_id = module.add_function(main_fn);

    let mut engine = JitEngine::new();
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    
    assert!(can_jit_function(&module, main_id));
    
    let jit_main = compile_function_i64(&mut engine, &module, main_id, &mut jit_cache, &mut compiling).unwrap();
    let res = jit_main.call_i64(&[]).unwrap();
    assert_eq!(res, 42);
    // Dependency should have been cached
    assert!(jit_cache.contains_key(&add_id));
}

#[test]
fn jit_call_chain() {
    let mut module = BytecodeModule::new();
    
    // fn inc(x: Int64) -> Int64 { x + 1 }
    let mut inc_fn = BytecodeFunction::new("inc".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    inc_fn.locals = vec![BytecodeType::Int64];
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    inc_fn.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    inc_fn.code.push(BytecodeInstr::ConstI64(c1));
    inc_fn.code.push(BytecodeInstr::AddI64);
    inc_fn.code.push(BytecodeInstr::Return);
    inc_fn.max_stack = 2;
    let inc_id = module.add_function(inc_fn);

    // fn twice(x: Int64) -> Int64 { inc(inc(x)) }
    let mut twice_fn = BytecodeFunction::new("twice".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    twice_fn.locals = vec![BytecodeType::Int64];
    twice_fn.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    twice_fn.code.push(BytecodeInstr::Call(inc_id, 1));
    twice_fn.code.push(BytecodeInstr::Call(inc_id, 1));
    twice_fn.code.push(BytecodeInstr::Return);
    twice_fn.max_stack = 2;
    let twice_id = module.add_function(twice_fn);

    let mut engine = JitEngine::new();
    let mut jit_cache = std::collections::HashMap::new();
    let mut compiling = std::collections::HashSet::new();
    
    assert!(can_jit_function(&module, twice_id));
    
    let jit_twice = compile_function_i64(&mut engine, &module, twice_id, &mut jit_cache, &mut compiling).unwrap();
    assert_eq!(jit_twice.call_i64(&[40]).unwrap(), 42);
}

#[test]
fn jit_rejects_call_to_string_function() {
    let mut module = BytecodeModule::new();
    
    // fn bad(s: String) -> Int64
    let bad_fn = BytecodeFunction::new("bad".into(), vec![BytecodeType::String], BytecodeType::Int64);
    let bad_id = module.add_function(bad_fn);

    // fn main() -> Int64 { bad("...") }
    let mut main_fn = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main_fn.code.push(BytecodeInstr::Call(bad_id, 1));
    main_fn.code.push(BytecodeInstr::Return);
    let main_id = module.add_function(main_fn);

    let eligibility = crate::engine::explain_jit_eligibility(&module, main_id);
    assert!(matches!(eligibility, crate::engine::JitEligibility::UnsupportedInstr { .. }));
    let err_str = eligibility.to_string();
    assert!(err_str.contains("Call target not JIT eligible"));
}

#[test]
fn jit_rejects_recursive_function_v1() {
    let mut module = BytecodeModule::new();
    
    // fn recurse(x: Int64) -> Int64 { recurse(x) }
    let mut rec_fn = BytecodeFunction::new("recurse".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    rec_fn.locals = vec![BytecodeType::Int64];
    rec_fn.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // We don't have the ID yet, we'll patch it later. Let's say function index 0.
    rec_fn.code.push(BytecodeInstr::Call(FunctionId(0), 1));
    rec_fn.code.push(BytecodeInstr::Return);
    rec_fn.max_stack = 2;
    let rec_id = module.add_function(rec_fn);
    
    // Ensure our patched ID is correct
    assert_eq!(rec_id.0, 0);

    let eligibility = crate::engine::explain_jit_eligibility(&module, rec_id);
    assert!(matches!(eligibility, crate::engine::JitEligibility::UnsupportedInstr { .. }));
    assert!(eligibility.to_string().contains("recursion"));
}
