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
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(FunctionId(0), 0),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::MakeClosureInout(FunctionId(0), 0),
        BytecodeInstr::BorrowSliceMut(LocalId(0)),
        BytecodeInstr::InoutSliceGet(LocalId(0)),
        BytecodeInstr::InoutSliceSet(LocalId(0)),
        BytecodeInstr::InoutSliceLen(LocalId(0)),
        BytecodeInstr::AddI64LL(
            tempra_bytecode::function::LocalId(0),
            tempra_bytecode::function::LocalId(0),
        ),
        BytecodeInstr::AddI64LC(
            tempra_bytecode::function::LocalId(0),
            tempra_bytecode::constant::ConstId(0),
        ),
        BytecodeInstr::MakeClosure(FunctionId(0), 0),
        BytecodeInstr::CallClosure(tempra_bytecode::function::SignatureId(0), 0),
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

#[test]
fn vm_closure_no_captures_dispatches_and_returns() {
    use tempra_bytecode::function::BytecodeSignature;

    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let ten = module.add_constant(BytecodeConstant::Int64(10));

    let mut lambda = BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    lambda.locals = lambda.params.clone();
    lambda.max_stack = 2;
    lambda.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let target = module.add_function(lambda);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::MakeClosure(target, 0),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(11)
    );
}

#[test]
fn vm_closure_capture_order_and_repeat_calls() {
    use tempra_bytecode::function::BytecodeSignature;

    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let seven = module.add_constant(BytecodeConstant::Int64(7));
    let ten = module.add_constant(BytecodeConstant::Int64(10));

    let mut lambda = BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    lambda.locals = lambda.params.clone();
    lambda.max_stack = 2;
    lambda.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let target = module.add_function(lambda);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Callable(signature)];
    main.max_stack = 3;
    main.code = vec![
        BytecodeInstr::ConstI64(seven),
        BytecodeInstr::MakeClosure(target, 1),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(34)
    );
}

#[test]
fn vm_closure_as_function_parameter() {
    use tempra_bytecode::function::BytecodeSignature;

    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let ten = module.add_constant(BytecodeConstant::Int64(10));

    let mut lambda = BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    lambda.locals = lambda.params.clone();
    lambda.max_stack = 2;
    lambda.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let lambda_id = module.add_function(lambda);

    let mut apply = BytecodeFunction::new(
        "apply".into(),
        vec![BytecodeType::Callable(signature), BytecodeType::Int64],
        BytecodeType::Int64,
    );
    apply.locals = apply.params.clone();
    apply.max_stack = 2;
    apply.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::Return,
    ];
    let apply_id = module.add_function(apply);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::MakeClosure(lambda_id, 0),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::Call(apply_id, 2),
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(11)
    );
}

// VM Parity 2B.1e.2: MakeClosureRef retains a frame-local handle, not a
// captured copy. Loading the hidden capture materializes its current value.
#[test]
fn vm_ref_borrow_slot_identity_and_late_dereference() {
    use tempra_bytecode::function::BytecodeSignature;
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let five = module.add_constant(BytecodeConstant::Int64(5));

    let mut lambda = BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    lambda.locals = vec![BytecodeType::Int64; 3];
    lambda.max_stack = 2;
    lambda.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::StoreLocal(LocalId(2)),
        BytecodeInstr::LoadLocal(LocalId(2)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let target = module.add_function(lambda);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(signature)];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::ConstI64(five),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    tempra_bytecode::verify::verify_module(&module).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(15)
    );
    // Borrowed closures now survive encode -> verified decode -> VM execution.
    let encoded = tempra_bytecode::serialize::encode_module(&module).unwrap();
    let decoded = tempra_bytecode::serialize::decode_and_verify_artifact(&encoded).unwrap();
    assert_eq!(
        run_function(&decoded.module, "main", vec![]).unwrap(),
        VmValue::Int64(15)
    );
}

#[test]
fn vm_borrowed_callback_cross_frame_transitive_and_cached() {
    use tempra_bytecode::function::BytecodeSignature;
    use tempra_bytecode::serialize::{decode_and_verify_artifact, encode_module};
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let base = module.add_constant(BytecodeConstant::Int64(7));
    let mut lambda = BytecodeFunction::new(
        "capture".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    lambda.locals = lambda.params.clone();
    lambda.max_stack = 1;
    lambda.code = vec![BytecodeInstr::LoadLocal(LocalId(0)), BytecodeInstr::Return];
    let capture_id = module.add_function(lambda);
    let mut invoke = BytecodeFunction::new(
        "invoke".into(),
        vec![BytecodeType::Callable(signature)],
        BytecodeType::Int64,
    );
    invoke.locals = invoke.params.clone();
    invoke.max_stack = 1;
    invoke.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::CallClosure(signature, 0),
        BytecodeInstr::Return,
    ];
    let invoke_id = module.add_function(invoke);
    let mut twice = BytecodeFunction::new(
        "twice".into(),
        vec![BytecodeType::Callable(signature)],
        BytecodeType::Int64,
    );
    twice.locals = twice.params.clone();
    twice.max_stack = 2;
    twice.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Call(invoke_id, 1),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Call(invoke_id, 1),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let twice_id = module.add_function(twice);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(signature)];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstI64(base),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(capture_id, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::Call(twice_id, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    tempra_bytecode::verify::verify_module(&module).unwrap();
    let bytes = encode_module(&module).unwrap();
    let decoded = decode_and_verify_artifact(&bytes).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(14)
    );
    assert_eq!(
        run_function(&decoded.module, "main", vec![]).unwrap(),
        VmValue::Int64(14)
    );
}

// 2B.5.A: the hidden mutable capture stores through its owner's frame slot.
#[test]
fn vm_inout_capture_updates_owner_across_calls_and_cache_roundtrip() {
    use tempra_bytecode::function::BytecodeSignature;
    use tempra_bytecode::serialize::{decode_and_verify_artifact, encode_module};
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Unit,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let five = module.add_constant(BytecodeConstant::Int64(5));
    let two = module.add_constant(BytecodeConstant::Int64(2));
    let mut child = BytecodeFunction::new(
        "mutate_capture".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Unit,
    );
    child.inout_params = vec![true, false];
    child.locals = child.params.clone();
    child.max_stack = 2;
    child.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::AddI64,
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::Return,
    ];
    let child_id = module.add_function(child);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(signature)];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::MakeClosureInout(child_id, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::ConstI64(five),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::ConstI64(two),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    tempra_bytecode::verify::verify_module(&module).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(17)
    );
    let bytes = encode_module(&module).unwrap();
    let restored = decode_and_verify_artifact(&bytes).unwrap();
    assert_eq!(
        run_function(&restored.module, "main", vec![]).unwrap(),
        VmValue::Int64(17)
    );
}

/// The inout signature is part of the bytecode contract and is checked again
/// after deserializing an untrusted module.
fn vm_inout_scalar_module() -> BytecodeModule {
    let mut module = BytecodeModule::new();
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let mut inc = BytecodeFunction::new(
        "increment".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Unit,
    );
    inc.inout_params = vec![true];
    inc.locals = inc.params.clone();
    inc.max_stack = 2;
    inc.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::AddI64,
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::Return,
    ];
    let inc_id = module.add_function(inc);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::Call(inc_id, 1),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::Call(inc_id, 1),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    module
}

#[test]
fn vm_inout_scalar_parameters_preserve_owner_and_release_between_calls() {
    use tempra_bytecode::serialize::{decode_and_verify_artifact, encode_module};
    let module = vm_inout_scalar_module();
    tempra_bytecode::verify::verify_module(&module).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(12)
    );
    let bytes = encode_module(&module).unwrap();
    let restored = decode_and_verify_artifact(&bytes).unwrap();
    assert_eq!(restored.module.functions[0].inout_params, vec![true]);
    assert_eq!(
        run_function(&restored.module, "main", vec![]).unwrap(),
        VmValue::Int64(12)
    );
}

#[test]
fn vm_inout_verifier_rejects_by_value_for_exclusive_parameter() {
    let mut module = vm_inout_scalar_module();
    let main = module.functions.last_mut().unwrap();
    main.code[2] = BytecodeInstr::LoadLocal(LocalId(0));
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_inout_verifier_rejects_exclusive_handle_for_by_value_parameter() {
    let mut module = vm_inout_scalar_module();
    module.functions[0].inout_params = vec![false];
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_inout_verifier_rejects_invalid_signature_metadata() {
    let mut module = vm_inout_scalar_module();
    module.functions[0].inout_params.clear();
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_inout_verifier_rejects_invalid_empty_function_metadata() {
    let mut module = BytecodeModule::new();
    let mut empty = BytecodeFunction::new(
        "empty".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Unit,
    );
    empty.inout_params.clear();
    module.add_function(empty);
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_inout_verifier_rejects_owner_read_while_exclusive_argument_pending() {
    let mut module = vm_inout_scalar_module();
    let main = module.functions.last_mut().unwrap();
    // Before calling increment, an ordinary read of counter would alias the
    // still-pending exclusive borrow.
    main.code.insert(3, BytecodeInstr::LoadLocal(LocalId(0)));
    main.code.insert(4, BytecodeInstr::Pop);
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

/// Bytecode-level fixture: exclusive subview mutations detach owning copies,
/// and sequential calls release the loan without releasing the owner.
fn vm_exclusive_slice_module() -> BytecodeModule {
    let mut module = BytecodeModule::new();
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let twenty = module.add_constant(BytecodeConstant::Int64(20));
    let thirty = module.add_constant(BytecodeConstant::Int64(30));
    let zero = module.add_constant(BytecodeConstant::Int64(0));
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let three = module.add_constant(BytecodeConstant::Int64(3));
    let ninety_nine = module.add_constant(BytecodeConstant::Int64(99));
    let mut mutate = BytecodeFunction::new(
        "mutate".into(),
        vec![BytecodeType::ExclusiveSlice],
        BytecodeType::Unit,
    );
    mutate.inout_params = vec![true];
    mutate.locals = mutate.params.clone();
    mutate.max_stack = 2;
    mutate.code = vec![
        BytecodeInstr::ConstI64(zero),
        BytecodeInstr::ConstI64(ninety_nine),
        BytecodeInstr::InoutSliceSet(LocalId(0)),
        BytecodeInstr::Return,
    ];
    let target = module.add_function(mutate);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Any, BytecodeType::Any];
    main.max_stack = 3;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::ConstI64(twenty),
        BytecodeInstr::ConstI64(thirty),
        BytecodeInstr::MakeArray(3),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::ConstI64(three),
        BytecodeInstr::BorrowSliceMut(LocalId(0)),
        BytecodeInstr::Call(target, 1),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::ConstI64(three),
        BytecodeInstr::BorrowSliceMut(LocalId(0)),
        BytecodeInstr::Call(target, 1),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::GetIndex,
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::GetIndex,
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    module
}

#[test]
fn vm_exclusive_slice_roundtrip_and_copy_isolation() {
    use tempra_bytecode::serialize::{decode_and_verify_artifact, encode_module};
    let module = vm_exclusive_slice_module();
    tempra_bytecode::verify::verify_module(&module).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(119)
    );
    let restored = decode_and_verify_artifact(&encode_module(&module).unwrap()).unwrap();
    assert_eq!(
        restored.module.functions[0].params,
        vec![BytecodeType::ExclusiveSlice]
    );
    assert_eq!(
        run_function(&restored.module, "main", vec![]).unwrap(),
        VmValue::Int64(119)
    );
}

#[test]
fn vm_exclusive_slice_rejects_raw_handle_duplication() {
    let mut module = vm_exclusive_slice_module();
    module.functions[1].code.insert(10, BytecodeInstr::Dup);
    module.functions[1].code.insert(11, BytecodeInstr::Pop);
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_exclusive_slice_rejects_descriptor_materialization() {
    let mut module = vm_exclusive_slice_module();
    module.functions[0]
        .code
        .insert(0, BytecodeInstr::LoadLocal(LocalId(0)));
    module.functions[0].code.insert(1, BytecodeInstr::Pop);
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_exclusive_slice_rejects_ordinary_slice_argument() {
    let mut module = vm_exclusive_slice_module();
    module.functions[1]
        .code
        .insert(7, BytecodeInstr::LoadLocal(LocalId(0)));
    module.functions[1].code[10] = BytecodeInstr::Slice;
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_exclusive_slice_rejects_missing_inout_flag() {
    let mut module = vm_exclusive_slice_module();
    module.functions[0].inout_params[0] = false;
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn vm_exclusive_slice_rejects_simultaneous_owner_borrows() {
    let mut module = vm_exclusive_slice_module();
    let main = &mut module.functions[1];
    let second_borrow = main.code[7..10].to_vec();
    main.code.splice(10..10, second_borrow);
    main.code.insert(13, BytecodeInstr::Call(FunctionId(0), 1));
    assert!(tempra_bytecode::verify::verify_module(&module).is_err());
}

#[test]
fn a1_vm_inout_read_is_a_value_snapshot() {
    let mut module = vm_inout_scalar_module();
    let twenty = module.add_constant(BytecodeConstant::Int64(20));
    let child = &mut module.functions[0];
    child.ret = BytecodeType::Int64;
    child.locals.push(BytecodeType::Int64);
    child.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::ConstI64(twenty),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::Return,
    ];
    let main = &mut module.functions[1];
    main.code.drain(4..6);
    main.code.insert(5, BytecodeInstr::AddI64);
    tempra_bytecode::verify::verify_module(&module).unwrap();
    assert_eq!(
        run_function(&module, "main", vec![]).unwrap(),
        VmValue::Int64(30)
    );
}

#[test]
fn a1_verifier_rejects_plain_calls_even_without_any_borrow_opcode() {
    let mut module = vm_inout_scalar_module();
    let main = module.functions.last_mut().unwrap();
    main.code[2] = BytecodeInstr::LoadLocal(LocalId(0));
    main.code[4] = BytecodeInstr::LoadLocal(LocalId(0));
    let error = tempra_bytecode::verify::verify_module(&module).unwrap_err();
    assert!(error
        .to_string()
        .contains("violates the verified inout/non-escape contract"));
    let bytes = tempra_bytecode::serialize::encode_module(&module).unwrap();
    assert!(tempra_bytecode::serialize::decode_and_verify_artifact(&bytes).is_err());
}

#[test]
fn a1_verifier_rejects_pending_exclusive_fused_local_reads() {
    let module = vm_inout_scalar_module();
    for read in [
        BytecodeInstr::AddI64LL(LocalId(0), LocalId(0)),
        BytecodeInstr::AddI64LC(LocalId(0), tempra_bytecode::constant::ConstId(0)),
    ] {
        let mut invalid = module.clone();
        let main = invalid.functions.last_mut().unwrap();
        main.code.insert(3, read);
        main.code.insert(4, BytecodeInstr::Pop);
        let error = tempra_bytecode::verify::verify_module(&invalid).unwrap_err();
        assert!(error.to_string().contains("exclusive argument is pending"));
    }
}

#[test]
fn a3_2a_read_slice_boundary_checks_descriptor() {
    use std::rc::Rc;
    let mut module = BytecodeModule::new();
    let mut function = BytecodeFunction::new(
        "observe".into(),
        vec![BytecodeType::ReadSlice],
        BytecodeType::Int64,
    );
    function.locals = function.params.clone();
    function.max_stack = 1;
    function.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Len,
        BytecodeInstr::Return,
    ];
    module.add_function(function);
    let parent = Rc::new(vec![VmValue::Int64(7), VmValue::Int64(8)]);
    let valid = VmValue::Slice {
        parent: parent.clone(),
        start: 1,
        end: 2,
    };
    assert_eq!(
        run_function(&module, "observe", vec![valid]).unwrap(),
        VmValue::Int64(1)
    );
    let wrong_type = VmValue::Array(parent.clone());
    assert!(matches!(
        run_function(&module, "observe", vec![wrong_type]),
        Err(VmError::TypeMismatch { .. })
    ));
    let wrong_bounds = VmValue::Slice {
        parent,
        start: 1,
        end: 3,
    };
    assert!(matches!(
        run_function(&module, "observe", vec![wrong_bounds]),
        Err(VmError::InvalidSliceRange { .. })
    ));
}
