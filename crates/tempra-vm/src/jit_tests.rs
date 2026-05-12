use crate::interpreter::Interpreter;
use crate::profile::VmOptions;
use crate::value::VmValue;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId};
use tempra_bytecode::opcode::{BytecodeInstr, InstrOffset};
use tempra_bytecode::constant::BytecodeConstant;

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_integration_add() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let add_id = module.add_function(func);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    let c10 = module.add_constant(BytecodeConstant::Int64(10));
    let c20 = module.add_constant(BytecodeConstant::Int64(20));
    
    // Call add(10, 20) enough times to trigger JIT
    for _ in 0..5 {
        main.code.push(BytecodeInstr::ConstI64(c10));
        main.code.push(BytecodeInstr::ConstI64(c20));
        main.code.push(BytecodeInstr::Call(add_id, 2));
        main.code.push(BytecodeInstr::Pop);
    }
    
    main.code.push(BytecodeInstr::ConstI64(c10));
    main.code.push(BytecodeInstr::ConstI64(c20));
    main.code.push(BytecodeInstr::Call(add_id, 2));
    main.code.push(BytecodeInstr::Return);
    main.max_stack = 2;
    let main_id = module.add_function(main);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 3; // Trigger JIT after 3 calls
    
    let mut interp = Interpreter::with_options(&module, options);
    let res = interp.run_function_id(main_id, vec![]).unwrap();
    
    assert_eq!(res, VmValue::Int64(30));
    assert!(interp.jit_cache.contains_key(&add_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_integration_loop() {
    let mut module = BytecodeModule::new();
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    let c1 = module.add_constant(BytecodeConstant::Int64(1));

    let mut func = BytecodeFunction::new("sum_loop".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64, BytecodeType::Int64];
    // acc = 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(2)));
    // i = 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    
    // Header (IP 4)
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LtI64);
    func.code.push(BytecodeInstr::JumpIfFalse(InstrOffset(17)));
    
    func.code.push(BytecodeInstr::LoadLocal(LocalId(2)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(2)));
    
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    
    func.code.push(BytecodeInstr::Jump(InstrOffset(4)));
    
    // Break target
    func.code.push(BytecodeInstr::LoadLocal(LocalId(2)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_backedge_threshold = 10;
    
    let mut interp = Interpreter::with_options(&module, options);
    // Call with 100 to trigger JIT via backedge
    let res = interp.run_function_id(func_id, vec![VmValue::Int64(100)]).unwrap();
    
    // sum(100) = 4950
    assert_eq!(res, VmValue::Int64(4950));
    assert!(interp.jit_cache.contains_key(&func_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_observability_stats() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let add_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.profiling = true;
    
    let mut interp = Interpreter::with_options(&module, options);
    
    // Call 1: cold
    interp.run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)]).unwrap();
    // Call 2: reaches threshold, triggers JIT
    interp.run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)]).unwrap();
    // Call 3: JIT runtime call
    interp.run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)]).unwrap();
    
    let profile = interp.build_profile();
    let stats = &profile.function_stats[add_id.0 as usize];
    
    assert_eq!(stats.jit_compile_attempts, 1);
    assert_eq!(stats.jit_compile_successes, 1);
    assert_eq!(stats.jit_runtime_calls, 1);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_rejection_reason() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("unsupported".into(), vec![BytecodeType::String], BytecodeType::Int64);
    func.locals = vec![BytecodeType::String];
    func.code.push(BytecodeInstr::ConstI64(module.add_constant(BytecodeConstant::Int64(42))));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 1;
    options.profiling = true;
    
    let mut interp = Interpreter::with_options(&module, options);
    
    // Trigger JIT check
    interp.run_function_id(func_id, vec![VmValue::String("hi".into())]).unwrap();
    // Try again, should not increment attempts (cached rejection)
    interp.run_function_id(func_id, vec![VmValue::String("hi".into())]).unwrap();
    
    let profile = interp.build_profile();
    let stats = &profile.function_stats[func_id.0 as usize];
    
    assert_eq!(stats.jit_compile_attempts, 1);
    assert_eq!(stats.jit_compile_successes, 0);
    assert!(stats.jit_rejection_reason.is_some());
    assert!(stats.jit_rejection_reason.as_ref().unwrap().contains("Unsupported type"));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_disabled_has_zero_stats() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("add".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let add_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = false;
    options.profiling = true;
    
    let mut interp = Interpreter::with_options(&module, options);
    for _ in 0..10 {
        interp.run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)]).unwrap();
    }
    
    let profile = interp.build_profile();
    let stats = &profile.function_stats[add_id.0 as usize];
    
    assert_eq!(stats.jit_compile_attempts, 0);
    assert_eq!(stats.jit_runtime_calls, 0);
}
