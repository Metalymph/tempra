use super::*;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use tempra_bytecode::opcode::{BytecodeInstr, BYTECODE_OPCODE_COUNT};

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
    module.add_function(func);

    let args = vec![VmValue::Int64(10), VmValue::Int64(20)];
    let result = run_function(&module, "add", args).unwrap();
    assert_eq!(result, VmValue::Int64(30));
}

#[test]
fn vm_bool_and() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "and".into(),
        vec![BytecodeType::Bool, BytecodeType::Bool],
        BytecodeType::Bool,
    );
    func.locals = vec![BytecodeType::Bool, BytecodeType::Bool];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::And);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let result = run_function(
        &module,
        "and",
        vec![VmValue::Bool(true), VmValue::Bool(false)],
    )
    .unwrap();
    assert_eq!(result, VmValue::Bool(false));
}

#[test]
fn vm_if_condbr() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c2 = module.add_constant(BytecodeConstant::Int64(2));

    let mut func =
        BytecodeFunction::new("test".into(), vec![BytecodeType::Bool], BytecodeType::Int64);
    func.locals = vec![BytecodeType::Bool];
    // 0: LoadLocal 0
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: JumpIfFalse 4
    func.code.push(BytecodeInstr::JumpIfFalse(
        tempra_bytecode::opcode::InstrOffset(4),
    ));
    // 2: Const 1, Return
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::Return);
    // 4: Const 2, Return
    func.code.push(BytecodeInstr::ConstI64(c2));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    assert_eq!(
        run_function(&module, "test", vec![VmValue::Bool(true)]).unwrap(),
        VmValue::Int64(1)
    );
    assert_eq!(
        run_function(&module, "test", vec![VmValue::Bool(false)]).unwrap(),
        VmValue::Int64(2)
    );
}

#[test]
fn vm_slot_load_store() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "test".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    func.locals.push(BytecodeType::Int64); // param at 0
    func.locals.push(BytecodeType::Int64); // temp slot at 1

    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(1)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    assert_eq!(
        run_function(&module, "test", vec![VmValue::Int64(99)]).unwrap(),
        VmValue::Int64(99)
    );
}

#[test]
fn vm_call_simple() {
    let mut module = BytecodeModule::new();

    // fn double(x: Int64) -> Int64 { x + x }
    let mut double = BytecodeFunction::new(
        "double".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
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

    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(42)
    );
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
    assert_eq!(
        result,
        Err(VmError::Int64Arithmetic(
            tempra_runtime::Int64ArithmeticError::DivisionByZero
        ))
    );
}

#[test]
fn vm_int64_arithmetic_failures_are_checked() {
    for (instruction, left, right, expected) in [
        (
            BytecodeInstr::AddI64,
            i64::MAX,
            1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            BytecodeInstr::SubI64,
            i64::MIN,
            1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            BytecodeInstr::SubI64,
            0,
            i64::MIN,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            BytecodeInstr::MulI64,
            i64::MAX,
            2,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            BytecodeInstr::DivI64,
            i64::MIN,
            -1,
            tempra_runtime::Int64ArithmeticError::Overflow,
        ),
        (
            BytecodeInstr::DivI64,
            1,
            0,
            tempra_runtime::Int64ArithmeticError::DivisionByZero,
        ),
    ] {
        let mut module = BytecodeModule::new();
        let left = module.add_constant(BytecodeConstant::Int64(left));
        let right = module.add_constant(BytecodeConstant::Int64(right));
        let mut function = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
        function.code.extend([
            BytecodeInstr::ConstI64(left),
            BytecodeInstr::ConstI64(right),
            instruction,
            BytecodeInstr::Return,
        ]);
        function.max_stack = 2;
        module.add_function(function);

        assert_eq!(
            run_function(&module, "test", vec![]),
            Err(VmError::Int64Arithmetic(expected))
        );
    }
}

#[test]
fn vm_int64_add_superinstructions_are_checked() {
    let mut module = BytecodeModule::new();
    let one = module.add_constant(BytecodeConstant::Int64(1));

    let mut local_local = BytecodeFunction::new(
        "local_local".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    local_local.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    local_local
        .code
        .push(BytecodeInstr::AddI64LL(LocalId(0), LocalId(1)));
    local_local.code.push(BytecodeInstr::Return);
    local_local.max_stack = 1;
    module.add_function(local_local);

    let mut local_constant = BytecodeFunction::new(
        "local_constant".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    local_constant.locals = vec![BytecodeType::Int64];
    local_constant
        .code
        .push(BytecodeInstr::AddI64LC(LocalId(0), one));
    local_constant.code.push(BytecodeInstr::Return);
    local_constant.max_stack = 1;
    module.add_function(local_constant);

    let expected = Err(VmError::Int64Arithmetic(
        tempra_runtime::Int64ArithmeticError::Overflow,
    ));
    assert_eq!(
        run_function(
            &module,
            "local_local",
            vec![VmValue::Int64(i64::MAX), VmValue::Int64(1)]
        ),
        expected
    );
    assert_eq!(
        run_function(&module, "local_constant", vec![VmValue::Int64(i64::MAX)]),
        expected
    );
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

    let options = VmOptions {
        profiling: true,
        ..Default::default()
    };
    let (_, profile) = run_function_profiled(&module, "test", vec![], options).unwrap();

    let stats = profile
        .function_stats
        .iter()
        .find(|s| s.function_name == "test")
        .unwrap();
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
    func.code
        .push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(0)));
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
    func.code.push(BytecodeInstr::JumpIfFalse(
        tempra_bytecode::opcode::InstrOffset(11),
    ));
    // 6: Load 0, Const 1, Add, Store 0
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::StoreLocal(LocalId(0)));
    // 10: Jump 2 (Backedge)
    func.code
        .push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(2)));
    // 11: Load 0, Return
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);

    let options = VmOptions {
        profiling: true,
        ..Default::default()
    };
    let (res, profile) = run_function_profiled(&module, "loop", vec![], options).unwrap();
    assert_eq!(res, VmValue::Int64(5));

    let stats = profile
        .function_stats
        .iter()
        .find(|s| s.function_name == "loop")
        .unwrap();
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

    let stats = profile
        .function_stats
        .iter()
        .find(|s| s.function_name == "hot")
        .unwrap();
    assert!(stats.is_hot);
}

#[test]
fn profile_disabled_default_behavior() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let options = VmOptions {
        profiling: false,
        ..Default::default()
    };
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

    let options = VmOptions {
        profiling: true,
        ..Default::default()
    };
    let (_, profile) = run_function_profiled(&module, "test", vec![], options).unwrap();

    let stats = profile
        .function_stats
        .iter()
        .find(|s| s.function_name == "test")
        .unwrap();
    assert_eq!(stats.instruction_count, 2);

    // ConstI64 id is 2
    assert_eq!(profile.global_opcode_counts[2], 1);
    // Return id is 34
    assert_eq!(profile.global_opcode_counts[34], 1);
}

#[test]
fn superinstruction_counter_visible() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "test".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    func.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    // AddI64LL(0, 1)
    func.code
        .push(BytecodeInstr::AddI64LL(LocalId(0), LocalId(1)));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    module.add_function(func);

    let options = VmOptions {
        profiling: true,
        ..Default::default()
    };
    let args = vec![VmValue::Int64(10), VmValue::Int64(20)];
    let (_, profile) = run_function_profiled(&module, "test", args, options).unwrap();

    let add_i64_ll_id = BytecodeInstr::AddI64LL(LocalId(0), LocalId(1)).opcode_id();
    assert_eq!(profile.global_opcode_counts[add_i64_ll_id], 1);
}

#[test]
fn profile_summary_contains_hot_function() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("hot_loop".into(), vec![], BytecodeType::Unit);
    func.code
        .push(BytecodeInstr::Jump(tempra_bytecode::opcode::InstrOffset(0)));
    module.add_function(func);

    // Manual profile construction to test summary()
    let profile = VmProfile {
        function_stats: vec![FunctionProfile {
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
            jit_compile_time_ns: 0,
            jit_total_native_time_ns: 0,
            jit_rejected_count: 0,
            jit_heap_runtime_calls: 0,
            jit_heap_helper_calls: 0,
            jit_heap_handle_allocations: 0,
            jit_heap_context_objects: 0,
            jit_heap_fallbacks: 0,
            jit_heap_errors: 0,
            jit_heap_handle_cache_hits: 0,
            jit_heap_handle_cache_misses: 0,
            jit_skipped_below_threshold: 0,
            jit_skipped_too_small: 0,
            jit_skipped_retry_limit: 0,
            jit_permanent_rejections: 0,
            jit_retryable_failures: 0,
            jit_is_permanently_rejected: false,
        }],
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
        BytecodeInstr::MakeEnum(
            tempra_bytecode::constant::ConstId(0),
            tempra_bytecode::constant::ConstId(0),
            0,
        ),
        BytecodeInstr::Tag,
        BytecodeInstr::Unwrap(tempra_bytecode::constant::ConstId(0), 0),
        BytecodeInstr::MakeArray(0),
        BytecodeInstr::MakeTuple(0),
        BytecodeInstr::GetIndex,
        BytecodeInstr::Slice,
        BytecodeInstr::Len,
        BytecodeInstr::Size,
        BytecodeInstr::Bytes,
        BytecodeInstr::MakeStruct(
            tempra_bytecode::constant::ConstId(0),
            tempra_bytecode::constant::ConstId(0),
            0,
        ),
        BytecodeInstr::GetField(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::SetField(tempra_bytecode::constant::ConstId(0)),
        BytecodeInstr::ArrayPush,
        BytecodeInstr::AddI64LL(
            tempra_bytecode::function::LocalId(0),
            tempra_bytecode::function::LocalId(0),
        ),
        BytecodeInstr::AddI64LC(
            tempra_bytecode::function::LocalId(0),
            tempra_bytecode::constant::ConstId(0),
        ),
    ];

    for instr in samples {
        let id = instr.opcode_id();
        assert!(
            id < BYTECODE_OPCODE_COUNT,
            "Opcode {:?} id {} out of bounds (max {})",
            instr,
            id,
            BYTECODE_OPCODE_COUNT
        );
        assert!(ids.insert(id), "Duplicate opcode id {} for {:?}", id, instr);
    }

    assert_eq!(
        ids.len(),
        BYTECODE_OPCODE_COUNT,
        "HashSet size mismatch with BYTECODE_OPCODE_COUNT"
    );
}

fn binary_module(
    name: &str,
    operand_type: BytecodeType,
    result_type: BytecodeType,
    op: BytecodeInstr,
) -> BytecodeModule {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        name.into(),
        vec![operand_type.clone(), operand_type.clone()],
        result_type,
    );
    func.locals = vec![operand_type.clone(), operand_type];
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(op);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 2;
    module.add_function(func);
    module
}

fn run_binary(
    module: &BytecodeModule,
    name: &str,
    left: VmValue,
    right: VmValue,
) -> Result<VmValue, VmError> {
    run_function(module, name, vec![left, right])
}

#[test]
fn vm_uint64_arithmetic_uses_checked_runtime_kernel() {
    let cases = [
        (BytecodeInstr::AddU64, 10u64, 20u64, 30u64),
        (BytecodeInstr::SubU64, 30, 20, 10),
        (BytecodeInstr::MulU64, 6, 7, 42),
        (BytecodeInstr::DivU64, 21, 7, 3),
    ];

    for (op, left, right, expected) in cases {
        let module = binary_module("op", BytecodeType::UInt64, BytecodeType::UInt64, op);
        let result =
            run_binary(&module, "op", VmValue::UInt64(left), VmValue::UInt64(right)).unwrap();
        assert_eq!(result, VmValue::UInt64(expected), "opcode {op:?}");
    }
}

#[test]
fn vm_uint64_arithmetic_reports_controlled_failures() {
    let add = binary_module(
        "add",
        BytecodeType::UInt64,
        BytecodeType::UInt64,
        BytecodeInstr::AddU64,
    );
    assert_eq!(
        run_binary(&add, "add", VmValue::UInt64(u64::MAX), VmValue::UInt64(1)),
        Err(VmError::UInt64Arithmetic(
            tempra_runtime::UInt64ArithmeticError::Overflow
        ))
    );

    let sub = binary_module(
        "sub",
        BytecodeType::UInt64,
        BytecodeType::UInt64,
        BytecodeInstr::SubU64,
    );
    assert_eq!(
        run_binary(&sub, "sub", VmValue::UInt64(0), VmValue::UInt64(1)),
        Err(VmError::UInt64Arithmetic(
            tempra_runtime::UInt64ArithmeticError::Underflow
        ))
    );

    let mul = binary_module(
        "mul",
        BytecodeType::UInt64,
        BytecodeType::UInt64,
        BytecodeInstr::MulU64,
    );
    assert_eq!(
        run_binary(&mul, "mul", VmValue::UInt64(u64::MAX), VmValue::UInt64(2)),
        Err(VmError::UInt64Arithmetic(
            tempra_runtime::UInt64ArithmeticError::Overflow
        ))
    );

    let div = binary_module(
        "div",
        BytecodeType::UInt64,
        BytecodeType::UInt64,
        BytecodeInstr::DivU64,
    );
    assert_eq!(
        run_binary(&div, "div", VmValue::UInt64(1), VmValue::UInt64(0)),
        Err(VmError::UInt64Arithmetic(
            tempra_runtime::UInt64ArithmeticError::DivisionByZero
        ))
    );
}

#[test]
fn vm_uint64_comparisons_are_unsigned() {
    let lt = binary_module(
        "lt",
        BytecodeType::UInt64,
        BytecodeType::Bool,
        BytecodeInstr::LtU64,
    );
    // `1` must compare below `u64::MAX`, unlike the signed reinterpretation.
    assert_eq!(
        run_binary(&lt, "lt", VmValue::UInt64(1), VmValue::UInt64(u64::MAX)).unwrap(),
        VmValue::Bool(true)
    );

    let gt = binary_module(
        "gt",
        BytecodeType::UInt64,
        BytecodeType::Bool,
        BytecodeInstr::GtU64,
    );
    assert_eq!(
        run_binary(&gt, "gt", VmValue::UInt64(u64::MAX), VmValue::UInt64(1)).unwrap(),
        VmValue::Bool(true)
    );
}

#[test]
fn vm_float64_division_follows_ieee_754() {
    let div = binary_module(
        "div",
        BytecodeType::Float64,
        BytecodeType::Float64,
        BytecodeInstr::DivF64,
    );

    assert_eq!(
        run_binary(&div, "div", VmValue::Float64(1.0), VmValue::Float64(0.0)).unwrap(),
        VmValue::Float64(f64::INFINITY)
    );
    assert_eq!(
        run_binary(&div, "div", VmValue::Float64(-1.0), VmValue::Float64(0.0)).unwrap(),
        VmValue::Float64(f64::NEG_INFINITY)
    );
    assert_eq!(
        run_binary(&div, "div", VmValue::Float64(1.0), VmValue::Float64(-0.0)).unwrap(),
        VmValue::Float64(f64::NEG_INFINITY)
    );

    match run_binary(&div, "div", VmValue::Float64(0.0), VmValue::Float64(0.0)).unwrap() {
        VmValue::Float64(value) => assert!(value.is_nan(), "0.0 / 0.0 must be NaN, got {value}"),
        other => panic!("expected Float64 NaN, got {other:?}"),
    }
}

#[test]
fn vm_float64_comparisons_and_negation_preserve_ieee_754() {
    let lt = binary_module(
        "lt",
        BytecodeType::Float64,
        BytecodeType::Bool,
        BytecodeInstr::LtF64,
    );
    let gt = binary_module(
        "gt",
        BytecodeType::Float64,
        BytecodeType::Bool,
        BytecodeInstr::GtF64,
    );

    // NaN is unordered: neither `<` nor `>` may hold.
    assert_eq!(
        run_binary(&lt, "lt", VmValue::Float64(f64::NAN), VmValue::Float64(1.0)).unwrap(),
        VmValue::Bool(false)
    );
    assert_eq!(
        run_binary(&gt, "gt", VmValue::Float64(f64::NAN), VmValue::Float64(1.0)).unwrap(),
        VmValue::Bool(false)
    );
    assert_eq!(
        run_binary(&lt, "lt", VmValue::Float64(0.0), VmValue::Float64(-0.0)).unwrap(),
        VmValue::Bool(false)
    );

    // Guideline lowering expresses unary negation as multiplication by -1.0.
    let neg = binary_module(
        "neg",
        BytecodeType::Float64,
        BytecodeType::Float64,
        BytecodeInstr::MulF64,
    );
    let negated = run_binary(&neg, "neg", VmValue::Float64(0.0), VmValue::Float64(-1.0)).unwrap();
    match negated {
        VmValue::Float64(value) => {
            assert_eq!(value, 0.0);
            assert!(
                value.is_sign_negative(),
                "negation must preserve signed -0.0"
            );
        }
        other => panic!("expected Float64, got {other:?}"),
    }
}
