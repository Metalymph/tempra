use super::*;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, LocalId, FunctionId};
use tempra_bytecode::opcode::{BytecodeInstr, BYTECODE_OPCODE_COUNT};
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

#[test]
fn profile_counts_function_call() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let options = VmOptions { profiling: true, ..Default::default() };
    let (_, profile) = run_function_profiled(&module, "test", vec![], options).unwrap();
    
    let stats = profile.function_stats.iter().find(|s| s.function_name == "test").unwrap();
    assert_eq!(stats.call_count, 1);
    assert_eq!(stats.instruction_count, 1);
}

#[test]
fn profile_counts_loop_backedge() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("loop".into(), vec![], BytecodeType::Unit);
    
    // 0: ConstUnit
    // 1: Jump 0 (Backedge)
    // 2: Return
    func.code.push(BytecodeInstr::ConstUnit);
    func.code.push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(0)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    // We only run a few iterations to verify the counter
    // But how to stop? We can't easily stop a VM loop in a test without a condition.
    // Let's use a condition.
}

#[test]
fn profile_counts_loop_with_cond() {
    let mut module = BytecodeModule::new();
    let c0 = module.add_constant(BytecodeConstant::Int64(0));
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c5 = module.add_constant(BytecodeConstant::Int64(5));

    let mut func = BytecodeFunction::new("loop".into(), vec![], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64]; // i
    
    // 0: Const 0, Store 0
    func.code.push(BytecodeInstr::ConstI64(c0));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(0)));
    // 2: Load 0, Const 5, Lt, JumpIfFalse 10
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::ConstI64(c5));
    func.code.push(BytecodeInstr::LtI64);
    func.code.push(BytecodeInstr::JumpIfFalse(tempra_bytecode::opcode::InstrOffset(11)));
    // 6: Load 0, Const 1, Add, Store 0
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(0)));
    // 10: Jump 2 (Backedge)
    func.code.push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(2)));
    // 11: Load 0, Return
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let options = VmOptions { profiling: true, ..Default::default() };
    let (res, profile) = run_function_profiled(&module, "loop", vec![], options).unwrap();
    assert_eq!(res, VmValue::Int64(5));

    let stats = profile.function_stats.iter().find(|s| s.function_name == "loop").unwrap();
    assert_eq!(stats.call_count, 1);
    assert_eq!(stats.backedge_count, 5);
}

#[test]
fn profile_marks_hot_function() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("hot".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let options = VmOptions { 
        profiling: true, 
        hot_call_threshold: 1, 
        ..Default::default() 
    };
    let (_, profile) = run_function_profiled(&module, "hot", vec![], options).unwrap();
    
    let stats = profile.function_stats.iter().find(|s| s.function_name == "hot").unwrap();
    assert!(stats.is_hot);
}

#[test]
fn profile_disabled_default_behavior() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let options = VmOptions { profiling: false, ..Default::default() };
    let (_, profile) = run_function_profiled(&module, "test", vec![], options).unwrap();
    
    assert!(profile.function_stats.is_empty());
}

#[test]
fn opcode_counter_increments() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    let options = VmOptions { profiling: true, ..Default::default() };
    let (_, profile) = run_function_profiled(&module, "test", vec![], options).unwrap();
    
    let stats = profile.function_stats.iter().find(|s| s.function_name == "test").unwrap();
    assert_eq!(stats.instruction_count, 2);
    
    // ConstI64 id is 2
    assert_eq!(profile.global_opcode_counts[2], 1);
    // Return id is 34
    assert_eq!(profile.global_opcode_counts[34], 1);
}

#[test]
fn superinstruction_counter_visible() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64, BytecodeType::Int64], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    // AddI64LL(0, 1)
    func.code.push(BytecodeInstr::AddI64LL(LocalId(0), LocalId(1)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    let options = VmOptions { profiling: true, ..Default::default() };
    let args = vec![VmValue::Int64(10), VmValue::Int64(20)];
    let (_, profile) = run_function_profiled(&module, "test", args, options).unwrap();
    
    // AddI64LL id is 50
    assert_eq!(profile.global_opcode_counts[50], 1);
}

#[test]
fn profile_summary_contains_hot_function() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("hot_loop".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(0)));
    module.add_function(func);

    // Manual profile construction to test summary()
    let profile = VmProfile {
        function_stats: vec![
            FunctionProfile {
                function_id: FunctionId(0),
                function_name: "hot_loop".to_string(),
                call_count: 1,
                instruction_count: 10000,
                backedge_count: 9999,
                is_hot: true,
                top_opcodes: vec![("Jump", 9999)],
                jit_compile_attempts: 0,
                jit_compile_successes: 0,
                jit_compile_failures: 0,
                jit_runtime_calls: 0,
                jit_fallback_calls: 0,
                jit_rejection_reason: None,
            }
        ],
        global_opcode_counts: vec![0; BYTECODE_OPCODE_COUNT],
    };

    let summary = profile.summary();
    assert!(summary.contains("hot_loop"));
    assert!(summary.contains("calls=1 instrs=10000 backedges=9999 hot=true"));
}

#[test]
fn opcode_ids_are_unique_and_within_bounds() {
    use std::collections::HashSet;
    let mut ids = HashSet::new();
    
    // We manually list a representative instance of each opcode
    // to verify that opcode_id() and BYTECODE_OPCODE_COUNT are in sync.
    let samples = vec![
        BytecodeInstr::ConstUnit,
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::ConstI64(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::ConstU64(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::ConstF64(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::ConstString(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::LoadLocal(tempra_bytecode::function::LocalId(0)),
        BytecodeInstr::StoreLocal(tempra_bytecode::function::LocalId(0)),
        BytecodeInstr::Pop,
        BytecodeInstr::Dup,
        BytecodeInstr::AddI64,
        BytecodeInstr::SubI64,
        BytecodeInstr::MulI64,
        BytecodeInstr::DivI64,
        BytecodeInstr::AddU64,
        BytecodeInstr::SubU64,
        BytecodeInstr::MulU64,
        BytecodeInstr::DivU64,
        BytecodeInstr::AddF64,
        BytecodeInstr::SubF64,
        BytecodeInstr::MulF64,
        BytecodeInstr::DivF64,
        BytecodeInstr::Eq,
        BytecodeInstr::LtI64,
        BytecodeInstr::GtI64,
        BytecodeInstr::LtU64,
        BytecodeInstr::GtU64,
        BytecodeInstr::LtF64,
        BytecodeInstr::GtF64,
        BytecodeInstr::Not,
        BytecodeInstr::And,
        BytecodeInstr::Or,
        BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(0)),
        BytecodeInstr::JumpIfFalse(tempra_bytecode::opcode::InstrOffset(0)),
        BytecodeInstr::Return,
        BytecodeInstr::Unreachable,
        BytecodeInstr::Call(tempra_bytecode::function::FunctionId(0), 0),
        BytecodeInstr::MakeEnum(tempra_bytecode::constant::ConstId(0), tempra_bytecode::constant::ConstId(0), 0),
        BytecodeInstr::Tag,
        BytecodeInstr::Unwrap(tempra_bytecode::constant::ConstId(0), 0),
        BytecodeInstr::MakeArray(0),
        BytecodeInstr::GetIndex,
        BytecodeInstr::Slice,
        BytecodeInstr::Len,
        BytecodeInstr::Size,
        BytecodeInstr::Bytes,
        BytecodeInstr::MakeStruct(tempra_bytecode::constant::ConstId(0), tempra_bytecode::constant::ConstId(0), 0),
        BytecodeInstr::GetField(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::SetField(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::ArrayPush,
        BytecodeInstr::AddI64LL(tempra_bytecode::function::LocalId(0), tempra_bytecode::function::LocalId(0)),
        BytecodeInstr::AddI64LC(tempra_bytecode::function::LocalId(0), tempra_bytecode::constant::ConstId(0)),
    ];

    for instr in samples {
        let id = instr.opcode_id();
        assert!(id < BYTECODE_OPCODE_COUNT, "Opcode {:?} id {} out of bounds (max {})", instr, id, BYTECODE_OPCODE_COUNT);
        assert!(ids.insert(id), "Duplicate opcode id {} for {:?}", id, instr);
    }
    
    assert_eq!(ids.len(), BYTECODE_OPCODE_COUNT, "HashSet size mismatch with BYTECODE_OPCODE_COUNT");
}
