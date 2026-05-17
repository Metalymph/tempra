use crate::interpreter::Interpreter;
use crate::profile::VmOptions;
use crate::value::VmValue;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId};
use tempra_bytecode::opcode::{BytecodeInstr, InstrOffset};

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_integration_add() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "add".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
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
    options.jit_min_instruction_count = 0; // unit test uses tiny hand-crafted bytecode

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

    let mut func = BytecodeFunction::new(
        "sum_loop".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    func.locals = vec![
        BytecodeType::Int64,
        BytecodeType::Int64,
        BytecodeType::Int64,
    ];
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
    let res = interp
        .run_function_id(func_id, vec![VmValue::Int64(100)])
        .unwrap();

    // sum(100) = 4950
    assert_eq!(res, VmValue::Int64(4950));
    assert!(interp.jit_cache.contains_key(&func_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_observability_stats() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "add".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
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
    options.jit_min_instruction_count = 0; // unit test uses tiny hand-crafted bytecode

    let mut interp = Interpreter::with_options(&module, options);

    // Call 1: cold
    interp
        .run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)])
        .unwrap();
    // Call 2: reaches threshold, triggers JIT
    interp
        .run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)])
        .unwrap();
    // Call 3: JIT runtime call
    interp
        .run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)])
        .unwrap();

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
    let mut func = BytecodeFunction::new(
        "unsupported".into(),
        vec![BytecodeType::Bool],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Bool];
    func.code.push(BytecodeInstr::ConstI64(
        module.add_constant(BytecodeConstant::Int64(42)),
    ));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 1;
    options.profiling = true;
    options.jit_min_instruction_count = 0; // unit test uses tiny hand-crafted bytecode

    let mut interp = Interpreter::with_options(&module, options);

    // Trigger JIT check
    interp
        .run_function_id(func_id, vec![VmValue::Bool(true)])
        .unwrap();
    // Try again, should not increment attempts (cached rejection)
    interp
        .run_function_id(func_id, vec![VmValue::Bool(true)])
        .unwrap();

    let profile = interp.build_profile();
    let stats = &profile.function_stats[func_id.0 as usize];

    assert_eq!(stats.jit_compile_attempts, 1);
    assert_eq!(stats.jit_compile_successes, 0);
    assert!(stats.jit_rejection_reason.is_some());
    assert!(stats
        .jit_rejection_reason
        .as_ref()
        .unwrap()
        .contains("Unsupported type"));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_disabled_has_zero_stats() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "add".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
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
        interp
            .run_function_id(add_id, vec![VmValue::Int64(10), VmValue::Int64(20)])
            .unwrap();
    }

    let profile = interp.build_profile();
    let stats = &profile.function_stats[add_id.0 as usize];

    assert_eq!(stats.jit_compile_attempts, 0);
    assert_eq!(stats.jit_runtime_calls, 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Phase 17.1: Heap-aware JIT VM integration tests
// ═══════════════════════════════════════════════════════════════════════════════

fn build_string_len_func(module: &mut BytecodeModule) -> tempra_bytecode::function::FunctionId {
    let mut func = BytecodeFunction::new(
        "string_len".into(),
        vec![BytecodeType::String],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::String];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Len);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func)
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_string_len_correctness() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 3;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    for _ in 0..3 {
        let res = interp
            .run_function_id(func_id, vec![VmValue::String("hello".into())])
            .unwrap();
        assert_eq!(res, VmValue::Int64(5));
    }

    assert!(
        interp.jit_cache_heap.contains_key(&func_id),
        "heap JIT not compiled"
    );
    assert!(
        !interp.jit_cache.contains_key(&func_id),
        "should NOT be in pure-i64 cache"
    );

    let res = interp
        .run_function_id(func_id, vec![VmValue::String("tempra".into())])
        .unwrap();
    assert_eq!(res, VmValue::Int64(6));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_string_len_unicode() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    for _ in 0..2 {
        interp
            .run_function_id(func_id, vec![VmValue::String("ab".into())])
            .unwrap();
    }

    // "👋 hi" = 4 unicode scalars
    let res = interp
        .run_function_id(func_id, vec![VmValue::String("👋 hi".into())])
        .unwrap();
    assert_eq!(res, VmValue::Int64(4));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_string_size() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "string_size".into(),
        vec![BytecodeType::String],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::String];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Size);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    for _ in 0..2 {
        interp
            .run_function_id(func_id, vec![VmValue::String("abc".into())])
            .unwrap();
    }

    // "👋 tempra" = 4 (emoji) + 1 (space) + 6 (tempra) = 11 bytes
    let res = interp
        .run_function_id(func_id, vec![VmValue::String("👋 tempra".into())])
        .unwrap();
    assert_eq!(res, VmValue::Int64(11));
    assert!(interp.jit_cache_heap.contains_key(&func_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_array_len() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "array_len".into(),
        vec![BytecodeType::Any],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Any];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Len);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    use std::rc::Rc;
    let arr3 = VmValue::Array(Rc::new(vec![
        VmValue::Int64(1),
        VmValue::Int64(2),
        VmValue::Int64(3),
    ]));
    let arr5 = VmValue::Array(Rc::new(vec![
        VmValue::Int64(10),
        VmValue::Int64(20),
        VmValue::Int64(30),
        VmValue::Int64(40),
        VmValue::Int64(50),
    ]));

    for _ in 0..2 {
        interp.run_function_id(func_id, vec![arr3.clone()]).unwrap();
    }

    let res = interp.run_function_id(func_id, vec![arr5.clone()]).unwrap();
    assert_eq!(res, VmValue::Int64(5));
    assert!(interp.jit_cache_heap.contains_key(&func_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_array_get_i64() {
    let mut module = BytecodeModule::new();
    // fn get(arr: Any, idx: Int64) -> Int64 { return arr[idx]; }
    let mut func = BytecodeFunction::new(
        "array_get".into(),
        vec![BytecodeType::Any, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Any, BytecodeType::Int64];
    // GetIndex pops index first, then array → push array, then index
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0))); // array
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1))); // index
    func.code.push(BytecodeInstr::GetIndex);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    let func_id = module.add_function(func);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    use std::rc::Rc;
    let arr = VmValue::Array(Rc::new(vec![
        VmValue::Int64(10),
        VmValue::Int64(20),
        VmValue::Int64(30),
    ]));

    for _ in 0..2 {
        interp
            .run_function_id(func_id, vec![arr.clone(), VmValue::Int64(0)])
            .unwrap();
    }

    let r1 = interp
        .run_function_id(func_id, vec![arr.clone(), VmValue::Int64(1)])
        .unwrap();
    assert_eq!(r1, VmValue::Int64(20));

    let r2 = interp
        .run_function_id(func_id, vec![arr.clone(), VmValue::Int64(2)])
        .unwrap();
    assert_eq!(r2, VmValue::Int64(30));

    assert!(interp.jit_cache_heap.contains_key(&func_id));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_stats_tracking() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.profiling = true;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    // 2 interpreter calls → triggers JIT compilation at threshold
    interp
        .run_function_id(func_id, vec![VmValue::String("hi".into())])
        .unwrap();
    interp
        .run_function_id(func_id, vec![VmValue::String("hi".into())])
        .unwrap();
    // 1 call via JIT
    interp
        .run_function_id(func_id, vec![VmValue::String("hello".into())])
        .unwrap();

    let profile = interp.build_profile();
    let stats = &profile.function_stats[func_id.0 as usize];

    assert_eq!(stats.jit_compile_attempts, 1, "should attempt once");
    assert_eq!(stats.jit_compile_successes, 1, "should succeed once");
    assert_eq!(stats.jit_runtime_calls, 1, "one JIT call after compilation");
    assert_eq!(stats.jit_compile_failures, 0, "no failures expected");
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_context_object_count_increases_on_handles() {
    let mut ctx = tempra_jit_cranelift::helpers::JitRuntimeContext::new();
    assert_eq!(ctx.object_count(), 0);
    assert_eq!(ctx.allocation_count(), 0);

    ctx.alloc_string("hello".to_string());
    assert_eq!(ctx.object_count(), 1);
    assert_eq!(ctx.allocation_count(), 1);

    ctx.alloc_array(vec![]);
    assert_eq!(ctx.object_count(), 2);
    assert_eq!(ctx.allocation_count(), 2);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_context_clear_resets_objects() {
    let mut ctx = tempra_jit_cranelift::helpers::JitRuntimeContext::new();
    ctx.alloc_string("hello".to_string());
    ctx.alloc_array(vec![]);
    assert_eq!(ctx.object_count(), 2);
    assert_eq!(ctx.allocation_count(), 2);

    ctx.clear();
    assert_eq!(ctx.object_count(), 0);
    assert_eq!(ctx.allocation_count(), 0);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_repeated_calls_do_not_error() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    options.profiling = true;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    // Compile JIT
    interp
        .run_function_id(func_id, vec![VmValue::String("a".into())])
        .unwrap();
    interp
        .run_function_id(func_id, vec![VmValue::String("b".into())])
        .unwrap();

    // Repeated runs
    for _ in 0..100 {
        let r = interp
            .run_function_id(func_id, vec![VmValue::String("tempra".into())])
            .unwrap();
        assert_eq!(r, VmValue::Int64(6));
    }

    let profile = interp.build_profile();
    let stats = &profile.function_stats[func_id.0 as usize];
    assert_eq!(stats.jit_runtime_calls, 100);
    assert_eq!(stats.jit_heap_runtime_calls, 100);
    assert_eq!(stats.jit_heap_helper_calls, 100);
    assert_eq!(stats.jit_heap_handle_allocations, 0); // No new handles allocated during native JIT run
    assert_eq!(stats.jit_heap_context_objects, 100);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_heap_warm_run_reuses_compiled_function() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 1;
    options.profiling = true;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    // First run triggers JIT compile
    interp
        .run_function_id(func_id, vec![VmValue::String("one".into())])
        .unwrap();

    let p1 = interp.build_profile();
    let attempts_1 = p1.function_stats[func_id.0 as usize].jit_compile_attempts;
    let successes_1 = p1.function_stats[func_id.0 as usize].jit_compile_successes;
    assert_eq!(attempts_1, 1);
    assert_eq!(successes_1, 1);

    // Run again (warm execution)
    interp
        .run_function_id(func_id, vec![VmValue::String("two".into())])
        .unwrap();

    let p2 = interp.build_profile();
    let attempts_2 = p2.function_stats[func_id.0 as usize].jit_compile_attempts;
    let successes_2 = p2.function_stats[func_id.0 as usize].jit_compile_successes;
    assert_eq!(attempts_2, 1);
    assert_eq!(successes_2, 1);
    assert_eq!(p2.function_stats[func_id.0 as usize].jit_runtime_calls, 1);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_heap_jit_string_len_matches_vm() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_len_func(&mut module);

    let mut interp_no_jit = Interpreter::new(&module);
    let r_vm = interp_no_jit
        .run_function_id(func_id, vec![VmValue::String("🚀 tempra JIT 🚀".into())])
        .unwrap();

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 0; // immediate compile
    options.jit_min_instruction_count = 0;
    let mut interp_jit = Interpreter::with_options(&module, options);
    let r_jit = interp_jit
        .run_function_id(func_id, vec![VmValue::String("🚀 tempra JIT 🚀".into())])
        .unwrap();

    assert_eq!(r_vm, r_jit);
}

fn build_string_size_func(module: &mut BytecodeModule) -> tempra_bytecode::function::FunctionId {
    let mut func = BytecodeFunction::new(
        "string_size".into(),
        vec![BytecodeType::String],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::String];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Size);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func)
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_heap_jit_string_size_unicode_matches_vm() {
    let mut module = BytecodeModule::new();
    let func_id = build_string_size_func(&mut module);

    let s = "👋 ciao";
    let mut interp_no_jit = Interpreter::new(&module);
    let r_vm = interp_no_jit
        .run_function_id(func_id, vec![VmValue::String(s.into())])
        .unwrap();

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;
    let mut interp_jit = Interpreter::with_options(&module, options);
    let r_jit = interp_jit
        .run_function_id(func_id, vec![VmValue::String(s.into())])
        .unwrap();

    assert_eq!(r_vm, r_jit);
}

fn build_array_len_func(module: &mut BytecodeModule) -> tempra_bytecode::function::FunctionId {
    let mut func = BytecodeFunction::new(
        "array_len".into(),
        vec![BytecodeType::Any],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Any];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Len);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func)
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_heap_jit_array_len_matches_vm() {
    let mut module = BytecodeModule::new();
    let func_id = build_array_len_func(&mut module);

    let arr = VmValue::Array(std::rc::Rc::new(vec![
        VmValue::Int64(10),
        VmValue::Int64(20),
    ]));

    let mut interp_no_jit = Interpreter::new(&module);
    let r_vm = interp_no_jit
        .run_function_id(func_id, vec![arr.clone()])
        .unwrap();

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;
    let mut interp_jit = Interpreter::with_options(&module, options);
    let r_jit = interp_jit.run_function_id(func_id, vec![arr]).unwrap();

    assert_eq!(r_vm, r_jit);
}

fn build_array_get_func(module: &mut BytecodeModule) -> tempra_bytecode::function::FunctionId {
    let mut func = BytecodeFunction::new(
        "array_get".into(),
        vec![BytecodeType::Any, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Any, BytecodeType::Int64];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::GetIndex);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func)
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_heap_jit_array_get_matches_vm() {
    let mut module = BytecodeModule::new();
    let func_id = build_array_get_func(&mut module);

    let arr = VmValue::Array(std::rc::Rc::new(vec![
        VmValue::Int64(100),
        VmValue::Int64(200),
    ]));

    let mut interp_no_jit = Interpreter::new(&module);
    let r_vm = interp_no_jit
        .run_function_id(func_id, vec![arr.clone(), VmValue::Int64(1)])
        .unwrap();

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;
    let mut interp_jit = Interpreter::with_options(&module, options);
    let r_jit = interp_jit
        .run_function_id(func_id, vec![arr, VmValue::Int64(1)])
        .unwrap();

    assert_eq!(r_vm, r_jit);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_heap_jit_array_get_oob_fallback_or_error() {
    let mut module = BytecodeModule::new();
    let func_id = build_array_get_func(&mut module);

    let arr = VmValue::Array(std::rc::Rc::new(vec![
        VmValue::Int64(100),
        VmValue::Int64(200),
    ]));

    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 0;
    options.profiling = true;
    options.jit_min_instruction_count = 0;

    let mut interp = Interpreter::with_options(&module, options);

    // First call: compiles JIT immediately, runs interpretively
    interp
        .run_function_id(func_id, vec![arr.clone(), VmValue::Int64(0)])
        .unwrap();

    // Second call: JIT is warm. Call with OOB index.
    // The JIT execution returns JIT_STATUS_ERROR_OUT_OF_BOUNDS, triggering VM fallback.
    let res = interp.run_function_id(func_id, vec![arr, VmValue::Int64(5)]);
    assert!(res.is_err()); // VM should return OOB error

    let p = interp.build_profile();
    let stats = &p.function_stats[func_id.0 as usize];
    assert_eq!(stats.jit_heap_fallbacks, 1);
    assert_eq!(stats.jit_heap_errors, 1);
}
