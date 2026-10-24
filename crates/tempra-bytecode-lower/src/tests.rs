use crate::lower::lower_tir_module;
use tempra_bytecode::disasm::disassemble_module;
use tempra_bytecode::verify::verify_module;
use tempra_tir::ir::*;

#[test]
fn test_lower_const_return_i64() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "main".into(),
        params: vec![],
        return_type: TirType::Int64,
        blocks: vec![],
        next_value_id: 0,
        next_block_id: 0,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    let b0 = TirBlockId(0);
    let v0 = TirValueId(0);
    let block = TirBlock {
        id: b0,
        instrs: vec![TirInstr::Const(v0, TirConst::Int64(42))],
        terminator: Some(TirTerminator::Ret(Some(v0))),
    };
    func.blocks.push(block);
    module.functions.push(func);

    let bc_module = lower_tir_module(&module).unwrap();
    verify_module(&bc_module).unwrap();
    let disasm = disassemble_module(&bc_module);

    assert!(disasm.contains("ConstI64 #0"));
    assert!(!disasm.contains("StoreLocal $0")); // Optimized away
    assert!(!disasm.contains("LoadLocal $0")); // Optimized away
    assert!(disasm.contains("Return"));
}

#[test]
fn test_lower_add_i64() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "add".into(),
        params: vec![
            (TirValueId(0), TirType::Int64),
            (TirValueId(1), TirType::Int64),
        ],
        return_type: TirType::Int64,
        blocks: vec![],
        next_value_id: 3,
        next_block_id: 0,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    let b0 = TirBlockId(0);
    let v2 = TirValueId(2);
    let block = TirBlock {
        id: b0,
        instrs: vec![TirInstr::Add(v2, TirValueId(0), TirValueId(1))],
        terminator: Some(TirTerminator::Ret(Some(v2))),
    };
    func.blocks.push(block);
    module.functions.push(func);
    let bc_module = lower_tir_module(&module).unwrap();
    verify_module(&bc_module).unwrap();
    let disasm = disassemble_module(&bc_module);

    assert!(disasm.contains("AddI64LL $0 $1"));
    assert!(!disasm.contains("StoreLocal $2")); // Optimized away
    assert!(!disasm.contains("LoadLocal $2")); // Optimized away
    assert!(disasm.contains("Return"));
}

#[test]
fn test_lower_if_condbr() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "test_if".into(),
        params: vec![(TirValueId(0), TirType::Bool)],
        return_type: TirType::Int64,
        blocks: vec![],
        next_value_id: 3,
        next_block_id: 3,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    // Block 0: CondBr %0, B1, B2
    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![],
        terminator: Some(TirTerminator::CondBr(
            TirValueId(0),
            TirBlockId(1),
            TirBlockId(2),
        )),
    });

    // Block 1: Ret 1
    let v1 = TirValueId(1);
    func.blocks.push(TirBlock {
        id: TirBlockId(1),
        instrs: vec![TirInstr::Const(v1, TirConst::Int64(1))],
        terminator: Some(TirTerminator::Ret(Some(v1))),
    });

    // Block 2: Ret 2
    let v2 = TirValueId(2);
    func.blocks.push(TirBlock {
        id: TirBlockId(2),
        instrs: vec![TirInstr::Const(v2, TirConst::Int64(2))],
        terminator: Some(TirTerminator::Ret(Some(v2))),
    });

    module.functions.push(func);
    let bc_module = lower_tir_module(&module).unwrap();
    verify_module(&bc_module).unwrap();
    let disasm = disassemble_module(&bc_module);

    // Verify jumps are patched
    assert!(disasm.contains("JumpIfFalse"));
    assert!(disasm.contains("Jump"));
    // Block 1 starts at some offset, Block 2 at another.
    // JumpIfFalse should point to Block 2.
}

#[test]
fn test_unsupported_make_error() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "bad".into(),
        params: vec![],
        return_type: TirType::Unit,
        blocks: vec![],
        next_value_id: 2,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    // Non-field Set remains unsupported; tuple/index reads lower through GetIndex.
    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![
            TirInstr::Make(
                TirValueId(0),
                TirType::Enum("Foo".into()),
                Some("Bar".into()),
                vec![],
            ),
            TirInstr::Set(
                TirValueId(1),
                TirValueId(0),
                TirKey::Index(0),
                TirValueId(0),
            ),
        ],
        terminator: Some(TirTerminator::Ret(None)),
    });
    module.functions.push(func);

    let result = lower_tir_module(&module);
    assert!(result.is_err());
}

#[test]
fn test_lower_make_tuple() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "tuple".into(),
        params: vec![],
        return_type: TirType::Tuple(vec![TirType::Int64, TirType::Int64]),
        blocks: vec![],
        next_value_id: 3,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![
            TirInstr::Const(TirValueId(0), TirConst::Int64(1)),
            TirInstr::Const(TirValueId(1), TirConst::Int64(2)),
            TirInstr::Make(
                TirValueId(2),
                TirType::Tuple(vec![TirType::Int64, TirType::Int64]),
                None,
                vec![TirValueId(0), TirValueId(1)],
            ),
        ],
        terminator: Some(TirTerminator::Ret(Some(TirValueId(2)))),
    });
    module.functions.push(func);

    let bc_module = lower_tir_module(&module).unwrap();
    verify_module(&bc_module).unwrap();
    let disasm = disassemble_module(&bc_module);
    assert!(disasm.contains("MakeTuple 2"));
}

#[test]
fn test_peephole_preserves_user_slots() {
    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "test".into(),
        params: vec![],
        return_type: TirType::Unit,
        blocks: vec![],
        next_value_id: 3,
        next_block_id: 1,
        next_slot_id: 1,
        debug_slots: Default::default(),
        is_extern: false,
    };

    let s0 = TirSlotId(0);
    let v0 = TirValueId(0);
    let v1 = TirValueId(1);

    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![
            TirInstr::Slot(s0, TirType::Int64),
            TirInstr::Const(v0, TirConst::Int64(10)),
            TirInstr::Store(s0, v0),
            TirInstr::Load(v1, s0),
        ],
        terminator: Some(TirTerminator::Ret(None)),
    });
    module.functions.push(func);

    let bc_module = lower_tir_module(&module).unwrap();
    let disasm = disassemble_module(&bc_module);

    // Slots are NOT optimized away because they are user-visible
    assert!(disasm.contains("StoreLocal $0"));
    assert!(disasm.contains("LoadLocal $0"));
}

#[test]
fn test_peephole_fuses_superinstructions() {
    use crate::lower_tir_module;
    use tempra_bytecode::disasm::disassemble_module;
    use tempra_bytecode::verify::verify_module;

    let mut module = TirModule::default();
    let mut func = TirFunction {
        id: TirFunctionId(0),
        name: "test_fusion".into(),
        params: vec![
            (TirValueId(0), TirType::Int64),
            (TirValueId(1), TirType::Int64),
        ],
        return_type: TirType::Int64,
        blocks: vec![],
        next_value_id: 3,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    };

    // v2 = v0 + v1
    // v3 = 10
    // v4 = v2 + v3
    let v0 = TirValueId(0);
    let v1 = TirValueId(1);
    let v2 = TirValueId(2);
    let v3 = TirValueId(3);
    let v4 = TirValueId(4);

    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![
            TirInstr::Add(v2, v0, v1),
            TirInstr::Const(v3, TirConst::Int64(10)),
            TirInstr::Add(v4, v2, v3),
        ],
        terminator: Some(TirTerminator::Ret(Some(v4))),
    });
    func.next_value_id = 5;

    module.functions.push(func);
    let bc_module = lower_tir_module(&module).unwrap();
    verify_module(&bc_module).unwrap();
    let disasm = disassemble_module(&bc_module);

    println!("{}", disasm);

    // Should contain superinstructions
    assert!(disasm.contains("AddI64LL"));
    assert!(disasm.contains("AddI64LC"));
}

fn returning_i64_function(id: usize, name: &str, value: i64) -> TirFunction {
    TirFunction {
        id: TirFunctionId(id),
        name: name.to_string(),
        params: vec![],
        return_type: TirType::Int64,
        blocks: vec![TirBlock {
            id: TirBlockId(0),
            instrs: vec![TirInstr::Const(TirValueId(0), TirConst::Int64(value))],
            terminator: Some(TirTerminator::Ret(Some(TirValueId(0)))),
        }],
        next_value_id: 1,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    }
}

fn calling_function(id: usize, name: &str, target: usize) -> TirFunction {
    TirFunction {
        id: TirFunctionId(id),
        name: name.to_string(),
        params: vec![],
        return_type: TirType::Int64,
        blocks: vec![TirBlock {
            id: TirBlockId(0),
            instrs: vec![TirInstr::Call(TirValueId(0), TirFunctionId(target), vec![])],
            terminator: Some(TirTerminator::Ret(Some(TirValueId(0)))),
        }],
        next_value_id: 1,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
        is_extern: false,
    }
}

#[test]
fn call_mapping_is_independent_of_physical_tir_order() {
    let module = TirModule {
        functions: vec![
            calling_function(0, "main", 1),
            returning_i64_function(2, "wrong", 0),
            returning_i64_function(1, "target", 41),
        ],
    };

    let bytecode = lower_tir_module(&module).unwrap();
    verify_module(&bytecode).unwrap();

    assert_eq!(
        bytecode
            .functions
            .iter()
            .map(|function| function.name.as_str())
            .collect::<Vec<_>>(),
        vec!["main", "target", "wrong"]
    );
    assert!(bytecode.functions[0].code.iter().any(|instruction| {
        matches!(
            instruction,
            tempra_bytecode::BytecodeInstr::Call(tempra_bytecode::function::FunctionId(1), 0)
        )
    }));
}

#[test]
fn duplicate_tir_function_id_is_rejected() {
    let module = TirModule {
        functions: vec![
            returning_i64_function(1, "first", 1),
            returning_i64_function(1, "second", 2),
        ],
    };

    assert!(matches!(
        lower_tir_module(&module),
        Err(crate::error::BytecodeLowerError::DuplicateTirFunctionId {
            function: TirFunctionId(1)
        })
    ));
}

#[test]
fn missing_tir_function_mapping_is_rejected_before_emission() {
    let module = TirModule {
        functions: vec![calling_function(3, "caller", 99)],
    };

    match lower_tir_module(&module) {
        Err(crate::error::BytecodeLowerError::TirVerification(message)) => {
            assert!(
                message.contains("FunctionNotFound"),
                "expected verifier function error, got {message}"
            );
        }
        other => panic!("expected TIR verification failure, got {other:?}"),
    }
}

fn lower_numeric_binary(
    operand_type: TirType,
    result_type: TirType,
    instruction: fn(TirValueId, TirValueId, TirValueId) -> TirInstr,
) -> tempra_bytecode::bytecode::BytecodeModule {
    let module = TirModule {
        functions: vec![TirFunction {
            id: TirFunctionId(0),
            name: "numeric".into(),
            params: vec![
                (TirValueId(0), operand_type.clone()),
                (TirValueId(1), operand_type),
            ],
            return_type: result_type,
            blocks: vec![TirBlock {
                id: TirBlockId(0),
                instrs: vec![instruction(TirValueId(2), TirValueId(0), TirValueId(1))],
                terminator: Some(TirTerminator::Ret(Some(TirValueId(2)))),
            }],
            next_value_id: 3,
            next_block_id: 1,
            next_slot_id: 0,
            debug_slots: Default::default(),
            is_extern: false,
        }],
    };
    let bytecode = lower_tir_module(&module).unwrap();
    verify_module(&bytecode).unwrap();
    bytecode
}

fn assert_numeric_opcode(
    operand_type: TirType,
    result_type: TirType,
    instruction: fn(TirValueId, TirValueId, TirValueId) -> TirInstr,
    expected: tempra_bytecode::opcode::BytecodeInstr,
    forbidden: &[tempra_bytecode::opcode::BytecodeInstr],
) {
    let bytecode = lower_numeric_binary(operand_type, result_type, instruction);
    let code = &bytecode.functions[0].code;
    // The peephole pass may fuse `LoadLocal a; LoadLocal b; AddI64` into the
    // `AddI64LL` superinstruction, which still proves the Int64 opcode was
    // selected. No fused variants exist for the other numeric opcodes.
    let present = code.iter().any(|instr| match (instr, &expected) {
        (
            tempra_bytecode::opcode::BytecodeInstr::AddI64LL(..),
            tempra_bytecode::opcode::BytecodeInstr::AddI64,
        ) => true,
        (instr, expected) => instr == expected,
    });
    assert!(
        present,
        "expected {expected:?} in lowered code, got {code:?}"
    );
    for instr in forbidden {
        assert!(
            !code.contains(instr),
            "numeric lowering must not contain {instr:?}: {code:?}"
        );
    }
}

#[test]
fn numeric_bytecode_opcodes_are_selected_from_verified_operand_types() {
    use tempra_bytecode::opcode::BytecodeInstr;

    for (operand_type, result_type, add, sub, mul, div) in [
        (
            TirType::Int64,
            TirType::Int64,
            BytecodeInstr::AddI64,
            BytecodeInstr::SubI64,
            BytecodeInstr::MulI64,
            BytecodeInstr::DivI64,
        ),
        (
            TirType::UInt64,
            TirType::UInt64,
            BytecodeInstr::AddU64,
            BytecodeInstr::SubU64,
            BytecodeInstr::MulU64,
            BytecodeInstr::DivU64,
        ),
        (
            TirType::Float64,
            TirType::Float64,
            BytecodeInstr::AddF64,
            BytecodeInstr::SubF64,
            BytecodeInstr::MulF64,
            BytecodeInstr::DivF64,
        ),
    ] {
        let arithmetic: [(
            fn(TirValueId, TirValueId, TirValueId) -> TirInstr,
            BytecodeInstr,
            BytecodeInstr,
        ); 4] = [
            (TirInstr::Add, add, BytecodeInstr::AddI64),
            (TirInstr::Sub, sub, BytecodeInstr::SubI64),
            (TirInstr::Mul, mul, BytecodeInstr::MulI64),
            (TirInstr::Div, div, BytecodeInstr::DivI64),
        ];
        for (instruction, expected, int64_opcode) in arithmetic {
            let forbidden: &[BytecodeInstr] = if expected == int64_opcode {
                &[]
            } else {
                std::slice::from_ref(&int64_opcode)
            };
            assert_numeric_opcode(
                operand_type.clone(),
                result_type.clone(),
                instruction,
                expected,
                forbidden,
            );
        }
    }

    for (operand_type, lt, gt) in [
        (TirType::Int64, BytecodeInstr::LtI64, BytecodeInstr::GtI64),
        (TirType::UInt64, BytecodeInstr::LtU64, BytecodeInstr::GtU64),
        (TirType::Float64, BytecodeInstr::LtF64, BytecodeInstr::GtF64),
    ] {
        let comparisons: [(
            fn(TirValueId, TirValueId, TirValueId) -> TirInstr,
            BytecodeInstr,
            BytecodeInstr,
        ); 2] = [
            (TirInstr::Lt, lt, BytecodeInstr::LtI64),
            (TirInstr::Gt, gt, BytecodeInstr::GtI64),
        ];
        for (instruction, expected, int64_opcode) in comparisons {
            let forbidden: &[BytecodeInstr] = if expected == int64_opcode {
                &[]
            } else {
                std::slice::from_ref(&int64_opcode)
            };
            assert_numeric_opcode(
                operand_type.clone(),
                TirType::Bool,
                instruction,
                expected,
                forbidden,
            );
        }
        assert_numeric_opcode(
            operand_type,
            TirType::Bool,
            TirInstr::Eq,
            BytecodeInstr::Eq,
            &[
                BytecodeInstr::LtI64,
                BytecodeInstr::GtI64,
                BytecodeInstr::LtU64,
                BytecodeInstr::GtU64,
                BytecodeInstr::LtF64,
                BytecodeInstr::GtF64,
            ],
        );
    }
}

#[test]
fn invalid_numeric_tir_is_rejected_before_bytecode_emission() {
    let module = TirModule {
        functions: vec![TirFunction {
            id: TirFunctionId(0),
            name: "invalid_add".into(),
            params: vec![
                (TirValueId(0), TirType::String),
                (TirValueId(1), TirType::String),
            ],
            return_type: TirType::String,
            blocks: vec![TirBlock {
                id: TirBlockId(0),
                instrs: vec![TirInstr::Add(TirValueId(2), TirValueId(0), TirValueId(1))],
                terminator: Some(TirTerminator::Ret(Some(TirValueId(2)))),
            }],
            next_value_id: 3,
            next_block_id: 1,
            next_slot_id: 0,
            debug_slots: Default::default(),
            is_extern: false,
        }],
    };

    match lower_tir_module(&module) {
        Err(crate::error::BytecodeLowerError::TirVerification(message)) => {
            assert!(
                message.contains("InvalidArithmeticType"),
                "expected typed TIR rejection, got {message}"
            );
        }
        other => panic!("expected TIR verification failure, got {other:?}"),
    }
}

#[test]
fn conflicting_struct_field_types_are_rejected_before_bytecode_emission() {
    // Two constructions disagree on Counter.value. The numeric use is written
    // against Float64, which is the last construction's type. Before the fix
    // the verifier retained Float64, accepted the module, and emitted AddF64.
    let module = TirModule {
        functions: vec![
            TirFunction {
                id: TirFunctionId(0),
                name: "make_i64".into(),
                params: vec![],
                return_type: TirType::Struct("Counter".into()),
                blocks: vec![TirBlock {
                    id: TirBlockId(0),
                    instrs: vec![
                        TirInstr::Const(TirValueId(0), TirConst::Int64(7)),
                        TirInstr::MakeStruct(
                            TirValueId(1),
                            "Counter".into(),
                            vec![("value".into(), TirValueId(0))],
                        ),
                    ],
                    terminator: Some(TirTerminator::Ret(Some(TirValueId(1)))),
                }],
                next_value_id: 2,
                next_block_id: 1,
                next_slot_id: 0,
                debug_slots: Default::default(),
                is_extern: false,
            },
            TirFunction {
                id: TirFunctionId(1),
                name: "make_f64".into(),
                params: vec![],
                return_type: TirType::Struct("Counter".into()),
                blocks: vec![TirBlock {
                    id: TirBlockId(0),
                    instrs: vec![
                        TirInstr::Const(TirValueId(0), TirConst::Float64(1.5)),
                        TirInstr::MakeStruct(
                            TirValueId(1),
                            "Counter".into(),
                            vec![("value".into(), TirValueId(0))],
                        ),
                    ],
                    terminator: Some(TirTerminator::Ret(Some(TirValueId(1)))),
                }],
                next_value_id: 2,
                next_block_id: 1,
                next_slot_id: 0,
                debug_slots: Default::default(),
                is_extern: false,
            },
            TirFunction {
                id: TirFunctionId(2),
                name: "use".into(),
                params: vec![(TirValueId(0), TirType::Struct("Counter".into()))],
                return_type: TirType::Float64,
                blocks: vec![TirBlock {
                    id: TirBlockId(0),
                    instrs: vec![
                        TirInstr::Get(TirValueId(1), TirValueId(0), TirKey::Field("value".into())),
                        TirInstr::Const(TirValueId(2), TirConst::Float64(2.5)),
                        TirInstr::Add(TirValueId(3), TirValueId(1), TirValueId(2)),
                    ],
                    terminator: Some(TirTerminator::Ret(Some(TirValueId(3)))),
                }],
                next_value_id: 4,
                next_block_id: 1,
                next_slot_id: 0,
                debug_slots: Default::default(),
                is_extern: false,
            },
        ],
    };

    match lower_tir_module(&module) {
        Err(crate::error::BytecodeLowerError::TirVerification(message)) => {
            assert!(
                message.contains("StructFieldTypeConflict"),
                "expected struct field conflict rejection, got {message}"
            );
        }
        other => panic!("expected TIR verification failure, got {other:?}"),
    }
}
