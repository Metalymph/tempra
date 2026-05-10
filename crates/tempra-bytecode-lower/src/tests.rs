use tempra_tir::ir::*;
use tempra_bytecode::disasm::disassemble_module;
use crate::lower::lower_tir_module;

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
    };
    
    let b0 = TirBlockId(0);
    let v0 = TirValueId(0);
    let block = TirBlock {
        id: b0,
        instrs: vec![
            TirInstr::Const(v0, TirConst::Int64(42)),
        ],
        terminator: Some(TirTerminator::Ret(Some(v0))),
    };
    func.blocks.push(block);
    module.functions.push(func);

    let bc_module = lower_tir_module(&module).unwrap();
    let disasm = disassemble_module(&bc_module);
    
    assert!(disasm.contains("ConstI64 #0"));
    assert!(disasm.contains("StoreLocal $0"));
    assert!(disasm.contains("LoadLocal $0"));
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
    };

    let b0 = TirBlockId(0);
    let v2 = TirValueId(2);
    let block = TirBlock {
        id: b0,
        instrs: vec![
            TirInstr::Add(v2, TirValueId(0), TirValueId(1)),
        ],
        terminator: Some(TirTerminator::Ret(Some(v2))),
    };
    func.blocks.push(block);
    module.functions.push(func);

    let bc_module = lower_tir_module(&module).unwrap();
    let disasm = disassemble_module(&bc_module);

    assert!(disasm.contains("LoadLocal $0"));
    assert!(disasm.contains("LoadLocal $1"));
    assert!(disasm.contains("AddI64"));
    assert!(disasm.contains("StoreLocal $2"));
    assert!(disasm.contains("LoadLocal $2"));
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
    };

    // Block 0: CondBr %0, B1, B2
    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![],
        terminator: Some(TirTerminator::CondBr(TirValueId(0), TirBlockId(1), TirBlockId(2))),
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
        next_value_id: 1,
        next_block_id: 1,
        next_slot_id: 0,
        debug_slots: Default::default(),
    };
    
    func.blocks.push(TirBlock {
        id: TirBlockId(0),
        instrs: vec![TirInstr::Make(TirValueId(0), TirType::Unit, vec![])],
        terminator: Some(TirTerminator::Ret(None)),
    });
    module.functions.push(func);

    let result = lower_tir_module(&module);
    assert!(result.is_err());
}
