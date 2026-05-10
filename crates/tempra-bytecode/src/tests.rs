use crate::bytecode::BytecodeModule;
use crate::constant::BytecodeConstant;
use crate::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use crate::opcode::{BytecodeInstr, stack_effect};
use crate::disasm::disassemble_function;
use crate::verify::{verify_function, BytecodeVerifyError};
use crate::serialize::{encode_module, encode_artifact, decode_artifact, decode_and_verify_artifact, BytecodeCacheArtifact, BytecodeSerializeError};

#[test]
fn test_create_module_with_constant() {
    let mut module = BytecodeModule::new();
    let id = module.add_constant(BytecodeConstant::Int64(42));
    assert_eq!(id.0, 0);
    assert_eq!(module.constants[0], BytecodeConstant::Int64(42));
}

#[test]
fn test_create_function_add_i64() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new(
        "add".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64
    );
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func.code.push(BytecodeInstr::AddI64);
    func.code.push(BytecodeInstr::Return);
    
    let id = module.add_function(func);
    assert_eq!(id.0, 0);
    assert_eq!(module.functions[0].name, "add");
}

#[test]
fn test_stack_effect_add_i64() {
    let module = BytecodeModule::new();
    let func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::AddI64;
    let effect = stack_effect(&instr, &module, &func).unwrap();
    assert_eq!(effect.pops, 2);
    assert_eq!(effect.pushes, 1);
}

#[test]
fn test_stack_effect_call_unit() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    let target_id = module.add_function(target);
    
    let caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::Call(target_id, 1);
    let effect = stack_effect(&instr, &module, &caller).unwrap();
    assert_eq!(effect.pops, 1);
    assert_eq!(effect.pushes, 0);
}

#[test]
fn test_stack_effect_call_value() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    let target_id = module.add_function(target);
    
    let caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    let instr = BytecodeInstr::Call(target_id, 1);
    let effect = stack_effect(&instr, &module, &caller).unwrap();
    assert_eq!(effect.pops, 1);
    assert_eq!(effect.pushes, 1);
}

#[test]
fn test_disassemble_simple_function() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(100));
    
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    let f_id = module.add_function(func);

    let disasm = disassemble_function(&module, f_id).unwrap();
    assert!(disasm.contains("fn test () -> Int64"));
    assert!(disasm.contains("ConstI64 #0"));
    assert!(disasm.contains("Return"));
}

#[test]
fn test_invalid_function_disassemble_error() {
    let module = BytecodeModule::new();
    let result = disassemble_function(&module, FunctionId(99));
    assert!(result.is_err());
}

#[test]
fn verify_const_return_i64_ok() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(42));
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let f_id = module.add_function(func);
    assert!(verify_function(&module, f_id).is_ok());
}

#[test]
fn verify_stack_underflow_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::AddI64);
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::StackUnderflow));
}

#[test]
fn verify_type_mismatch_add_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::ConstBool(true));
    func.code.push(BytecodeInstr::ConstBool(false));
    func.code.push(BytecodeInstr::AddI64);
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::StackTypeMismatch { .. })));
}

#[test]
fn verify_load_uninitialized_local_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.locals.push(BytecodeType::Int64);
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::LocalNotInitialized(LocalId(0))));
}

#[test]
fn verify_store_initializes_local_ok() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::Int64(10));
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.locals.push(BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstI64(c_id));
    func.code.push(BytecodeInstr::StoreLocal(LocalId(0)));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::Pop);
    func.code.push(BytecodeInstr::Return);
    func.max_stack = 1;
    let f_id = module.add_function(func);
    assert!(verify_function(&module, f_id).is_ok());
}

#[test]
fn verify_invalid_jump_target_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Jump(crate::opcode::InstrOffset(99)));
    let f_id = module.add_function(func);
    assert_eq!(verify_function(&module, f_id), Err(BytecodeVerifyError::InvalidJumpTarget(99)));
}

/*
#[test]
fn verify_stack_height_merge_mismatch_error() {
    // ...
}
*/

#[test]
fn verify_return_type_mismatch_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Int64);
    func.code.push(BytecodeInstr::ConstBool(true));
    func.code.push(BytecodeInstr::Return);
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::ReturnTypeMismatch { .. })));
}

#[test]
fn verify_call_wrong_arity_error() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    let target_id = module.add_function(target);
    
    let mut func = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Call(target_id, 0)); // Expected 1
    let f_id = module.add_function(func);
    assert!(matches!(verify_function(&module, f_id), Err(BytecodeVerifyError::CallArityMismatch { .. })));
}

// --- Serialization Tests ---

#[test]
fn test_serialize_roundtrip_minimal() {
    let module = BytecodeModule::new();
    let bytes = encode_module(&module).unwrap();
    let decoded = decode_artifact(&bytes).unwrap();
    
    assert_eq!(decoded.module.version, module.version);
    assert!(decoded.module.constants.is_empty());
    assert!(decoded.module.functions.is_empty());
}

#[test]
fn test_serialize_roundtrip_complex() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::String("hello".into()));
    module.add_constant(BytecodeConstant::Float64(3.14));
    
    let mut func = BytecodeFunction::new("test".into(), vec![BytecodeType::String], BytecodeType::Unit);
    func.locals = vec![BytecodeType::String, BytecodeType::Int64];
    func.max_stack = 4;
    func.code.push(BytecodeInstr::ConstString(c_id));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code.push(BytecodeInstr::JumpIfFalse(crate::opcode::InstrOffset(42)));
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let bytes = encode_module(&module).unwrap();
    let decoded = decode_artifact(&bytes).unwrap();

    let dec_mod = decoded.module;
    assert_eq!(dec_mod.constants.len(), 2);
    assert_eq!(dec_mod.constants[0], BytecodeConstant::String("hello".into()));
    assert_eq!(dec_mod.constants[1], BytecodeConstant::Float64(3.14));

    assert_eq!(dec_mod.functions.len(), 1);
    let dec_f = &dec_mod.functions[0];
    assert_eq!(dec_f.name, "test");
    assert_eq!(dec_f.params, vec![BytecodeType::String]);
    assert_eq!(dec_f.ret, BytecodeType::Unit);
    assert_eq!(dec_f.locals, vec![BytecodeType::String, BytecodeType::Int64]);
    assert_eq!(dec_f.max_stack, 4);
    assert_eq!(dec_f.code.len(), 4);
    assert_eq!(dec_f.code[0], BytecodeInstr::ConstString(c_id));
    assert_eq!(dec_f.code[2], BytecodeInstr::JumpIfFalse(crate::opcode::InstrOffset(42)));
}

#[test]
fn test_serialize_invalid_magic() {
    let bytes = vec![0, 0, 0, 0, 1, 0, 0, 0];
    let res = decode_artifact(&bytes);
    assert_eq!(res.unwrap_err(), BytecodeSerializeError::InvalidMagic);
}

#[test]
fn test_serialize_truncated() {
    let module = BytecodeModule::new();
    let mut bytes = encode_module(&module).unwrap();
    bytes.pop(); // truncate 1 byte
    
    let res = decode_artifact(&bytes);
    assert_eq!(res.unwrap_err(), BytecodeSerializeError::UnexpectedEof);
}

#[test]
fn test_serialize_trailing_bytes() {
    let module = BytecodeModule::new();
    let mut bytes = encode_module(&module).unwrap();
    bytes.push(0); // trailing byte
    
    let res = decode_artifact(&bytes);
    assert_eq!(res.unwrap_err(), BytecodeSerializeError::TrailingBytes);
}

#[test]
fn test_decode_and_verify_ok() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let bytes = encode_module(&module).unwrap();
    let res = decode_and_verify_artifact(&bytes);
    assert!(res.is_ok());
}

#[test]
fn test_decode_and_verify_fail() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Pop); // Invalid: pop empty stack
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let bytes = encode_module(&module).unwrap();
    let res = decode_and_verify_artifact(&bytes);
    assert!(matches!(res, Err(BytecodeSerializeError::VerifyError(_))));
}
