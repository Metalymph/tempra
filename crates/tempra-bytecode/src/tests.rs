use crate::bytecode::BytecodeModule;
use crate::constant::BytecodeConstant;
use crate::disasm::disassemble_function;
use crate::function::{BytecodeFunction, BytecodeType, FunctionId, LocalId};
use crate::opcode::{stack_effect, BytecodeInstr};
use crate::serialize::{
    decode_and_verify_artifact, decode_artifact, encode_module, BytecodeSerializeError,
};
use crate::verify::{verify_function, BytecodeVerifyError};

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
        BytecodeType::Int64,
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
    let target =
        BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
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
fn invalid_call_target_stack_effect_is_a_controlled_error() {
    let module = BytecodeModule::new();
    let caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    assert!(matches!(
        stack_effect(&BytecodeInstr::Call(FunctionId(999), 0), &module, &caller),
        Err(crate::error::BytecodeError::InvalidFunctionId(FunctionId(
            999
        )))
    ));
}

#[test]
fn malformed_parameter_local_layout_is_a_controlled_cache_error() {
    let mut module = BytecodeModule::new();
    let mut function =
        BytecodeFunction::new("main".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    function.code.push(BytecodeInstr::Return);
    module.add_function(function);
    let bytes = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&bytes),
        Err(BytecodeSerializeError::VerifyError(
            BytecodeVerifyError::InvalidLocalId(LocalId(0))
        ))
    ));
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
    assert_eq!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::StackUnderflow)
    );
}

#[test]
fn verify_type_mismatch_add_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::ConstBool(true));
    func.code.push(BytecodeInstr::ConstBool(false));
    func.code.push(BytecodeInstr::AddI64);
    let f_id = module.add_function(func);
    assert!(matches!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::StackTypeMismatch { .. })
    ));
}

#[test]
fn verify_load_uninitialized_local_error() {
    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".into(), vec![], BytecodeType::Unit);
    func.locals.push(BytecodeType::Int64);
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    let f_id = module.add_function(func);
    assert_eq!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::LocalNotInitialized(LocalId(0)))
    );
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
    func.code
        .push(BytecodeInstr::Jump(crate::opcode::InstrOffset(99)));
    let f_id = module.add_function(func);
    assert_eq!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::InvalidJumpTarget(99))
    );
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
    assert!(matches!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::ReturnTypeMismatch { .. })
    ));
}

#[test]
fn verify_call_wrong_arity_error() {
    let mut module = BytecodeModule::new();
    let target = BytecodeFunction::new("sub".into(), vec![BytecodeType::Int64], BytecodeType::Unit);
    let target_id = module.add_function(target);

    let mut func = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    func.code.push(BytecodeInstr::Call(target_id, 0)); // Expected 1
    let f_id = module.add_function(func);
    assert!(matches!(
        verify_function(&module, f_id),
        Err(BytecodeVerifyError::CallArityMismatch { .. })
    ));
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

// `3.14` is an intentional serialization fixture, not an approximation of PI.
#[allow(clippy::approx_constant)]
#[test]
fn test_serialize_roundtrip_complex() {
    let mut module = BytecodeModule::new();
    let c_id = module.add_constant(BytecodeConstant::String("hello".into()));
    module.add_constant(BytecodeConstant::Float64(3.14));

    let mut func = BytecodeFunction::new(
        "test".into(),
        vec![BytecodeType::String],
        BytecodeType::Unit,
    );
    func.locals = vec![BytecodeType::String, BytecodeType::Int64];
    func.max_stack = 4;
    func.code.push(BytecodeInstr::ConstString(c_id));
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func.code
        .push(BytecodeInstr::JumpIfFalse(crate::opcode::InstrOffset(42)));
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let bytes = encode_module(&module).unwrap();
    let decoded = decode_artifact(&bytes).unwrap();

    let dec_mod = decoded.module;
    assert_eq!(dec_mod.constants.len(), 2);
    assert_eq!(
        dec_mod.constants[0],
        BytecodeConstant::String("hello".into())
    );
    assert_eq!(dec_mod.constants[1], BytecodeConstant::Float64(3.14));

    assert_eq!(dec_mod.functions.len(), 1);
    let dec_f = &dec_mod.functions[0];
    assert_eq!(dec_f.name, "test");
    assert_eq!(dec_f.params, vec![BytecodeType::String]);
    assert_eq!(dec_f.ret, BytecodeType::Unit);
    assert_eq!(
        dec_f.locals,
        vec![BytecodeType::String, BytecodeType::Int64]
    );
    assert_eq!(dec_f.max_stack, 4);
    assert_eq!(dec_f.code.len(), 4);
    assert_eq!(dec_f.code[0], BytecodeInstr::ConstString(c_id));
    assert_eq!(
        dec_f.code[2],
        BytecodeInstr::JumpIfFalse(crate::opcode::InstrOffset(42))
    );
}

#[test]
fn test_serialize_roundtrip_make_tuple() {
    let mut module = BytecodeModule::new();
    let c1 = module.add_constant(BytecodeConstant::Int64(1));
    let c2 = module.add_constant(BytecodeConstant::Int64(2));

    let mut func = BytecodeFunction::new("tuple".into(), vec![], BytecodeType::Any);
    func.max_stack = 2;
    func.code.push(BytecodeInstr::ConstI64(c1));
    func.code.push(BytecodeInstr::ConstI64(c2));
    func.code.push(BytecodeInstr::MakeTuple(2));
    func.code.push(BytecodeInstr::Return);
    module.add_function(func);

    let bytes = encode_module(&module).unwrap();
    let decoded = decode_and_verify_artifact(&bytes).unwrap();
    let dec_f = &decoded.module.functions[0];
    assert_eq!(dec_f.code[2], BytecodeInstr::MakeTuple(2));
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

#[test]
fn serialization_is_byte_stable_for_reproducibility_sensitive_values() {
    let mut module = BytecodeModule::new();
    let negative = module.add_constant(BytecodeConstant::Int64(i64::MIN));
    let unsigned = module.add_constant(BytecodeConstant::UInt64(u64::MAX));
    let negative_zero = module.add_constant(BytecodeConstant::Float64(-0.0));
    let nan_bits = 0x7ff8_0000_0000_0042;
    let nan = module.add_constant(BytecodeConstant::Float64(f64::from_bits(nan_bits)));
    let utf8 = module.add_constant(BytecodeConstant::String("Tempra λ 🔥".into()));
    let fields = module.add_constant(BytecodeConstant::FieldList(vec![
        "zebra".into(),
        "alpha".into(),
    ]));

    let mut function = BytecodeFunction::new(
        "module.stable".into(),
        vec![BytecodeType::Int64, BytecodeType::Float64],
        BytecodeType::Any,
    );
    function.locals = vec![BytecodeType::UInt64, BytecodeType::String];
    function.max_stack = 4;
    function.code = vec![
        BytecodeInstr::ConstI64(negative),
        BytecodeInstr::ConstU64(unsigned),
        BytecodeInstr::ConstF64(negative_zero),
        BytecodeInstr::ConstF64(nan),
        BytecodeInstr::ConstString(utf8),
        BytecodeInstr::MakeStruct(utf8, fields, 2),
        BytecodeInstr::MakeEnum(utf8, utf8, 1),
        BytecodeInstr::Tag,
        BytecodeInstr::Call(FunctionId(17), 3),
        BytecodeInstr::Return,
    ];
    module.add_function(function);

    let first = encode_module(&module).unwrap();
    let second = encode_module(&module).unwrap();
    assert_eq!(first, second);

    let decoded = decode_artifact(&first).unwrap().module;
    assert_eq!(encode_module(&decoded).unwrap(), first);
    assert_eq!(
        match decoded.constants[2] {
            BytecodeConstant::Float64(value) => value.to_bits(),
            _ => unreachable!(),
        },
        (-0.0f64).to_bits()
    );
    assert_eq!(
        match decoded.constants[3] {
            BytecodeConstant::Float64(value) => value.to_bits(),
            _ => unreachable!(),
        },
        nan_bits
    );
    assert_eq!(
        decoded.constants[4],
        BytecodeConstant::String("Tempra λ 🔥".into())
    );
    assert_eq!(
        decoded.functions[0].code[8],
        BytecodeInstr::Call(FunctionId(17), 3)
    );
}

#[test]
fn closure_isa_roundtrip_reserves_stable_opcode_ids() {
    use crate::serialize::decode_artifact;

    let mut module = BytecodeModule::new();
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    ));
    let signature_id = module.intern_signature(crate::function::BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let mut caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    caller.code.push(BytecodeInstr::MakeClosure(target, 0));
    caller
        .code
        .push(BytecodeInstr::CallClosure(signature_id, 1));
    module.add_function(caller);

    assert_eq!(BytecodeInstr::MakeClosure(target, 0).opcode_id(), 53);
    assert_eq!(BytecodeInstr::CallClosure(signature_id, 1).opcode_id(), 54);
    assert_eq!(crate::opcode::BYTECODE_OPCODE_COUNT, 64);
    let bytes = encode_module(&module).unwrap();
    assert_eq!(bytes, encode_module(&module).unwrap());
    let decoded = decode_artifact(&bytes).unwrap();
    assert_eq!(decoded.module.version, 4);
    assert_eq!(encode_module(&decoded.module).unwrap(), bytes);
    assert_eq!(
        decoded.module.functions[1].code[0],
        BytecodeInstr::MakeClosure(target, 0)
    );
    assert_eq!(
        decoded.module.functions[1].code[1],
        BytecodeInstr::CallClosure(signature_id, 1)
    );
}

#[test]
fn closure_isa_fails_closed_in_verifier() {
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![],
        BytecodeType::Int64,
    ));
    let signature_id = module.intern_signature(crate::function::BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let mut caller = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    caller.max_stack = 2;
    caller.code = vec![BytecodeInstr::MakeClosure(target, 0), BytecodeInstr::Return];
    module.add_function(caller);
    // Unit-returning function cannot leave an unused callable on its stack.
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::ReturnTypeMismatch { .. })
    ));

    module.functions[1].code = vec![
        BytecodeInstr::CallClosure(signature_id, 0),
        BytecodeInstr::Return,
    ];
    assert!(verify_module(&module).is_err());
    assert!(matches!(
        decode_and_verify_artifact(&encode_module(&module).unwrap()),
        Err(BytecodeSerializeError::VerifyError(_))
    ));
}

#[test]
fn previous_bytecode_cache_version_is_rejected() {
    let module = BytecodeModule::new();
    let mut bytes = encode_module(&module).unwrap();
    bytes[4..8].copy_from_slice(&2_u32.to_le_bytes());
    assert!(matches!(
        decode_artifact(&bytes),
        Err(BytecodeSerializeError::UnsupportedVersion)
    ));
}

#[test]
fn callable_signature_table_is_deterministic_and_roundtrips() {
    use crate::function::{BytecodeSignature, SignatureId};
    use crate::serialize::decode_artifact;
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let signature = BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    };
    let id = module.intern_signature(signature.clone());
    assert_eq!(id, SignatureId(0));
    assert_eq!(module.intern_signature(signature), id);
    assert_eq!(module.signatures.len(), 1);

    let mut func = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    func.locals = vec![BytecodeType::Callable(id)];
    func.code = vec![BytecodeInstr::Return];
    module.add_function(func);
    verify_module(&module).unwrap();

    let first = encode_module(&module).unwrap();
    assert_eq!(first, encode_module(&module).unwrap());
    let decoded = decode_artifact(&first).unwrap().module;
    assert_eq!(decoded.signatures, module.signatures);
    assert_eq!(decoded.functions[0].locals, module.functions[0].locals);
    assert_eq!(encode_module(&decoded).unwrap(), first);
}

#[test]
fn callable_signature_reference_is_verified_fail_closed() {
    use crate::function::SignatureId;
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    func.locals = vec![BytecodeType::Callable(SignatureId(77))];
    func.code = vec![BytecodeInstr::Return];
    module.add_function(func);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::InvalidSignatureId(SignatureId(77)))
    ));
    assert!(matches!(
        decode_and_verify_artifact(&encode_module(&module).unwrap()),
        Err(BytecodeSerializeError::VerifyError(
            BytecodeVerifyError::InvalidSignatureId(SignatureId(77))
        ))
    ));
}

#[test]
fn previous_closure_bytecode_cache_version_is_rejected() {
    use crate::serialize::decode_artifact;
    let module = BytecodeModule::new();
    let mut bytes = encode_module(&module).unwrap();
    bytes[4..8].copy_from_slice(&3_u32.to_le_bytes());
    assert!(matches!(
        decode_artifact(&bytes),
        Err(BytecodeSerializeError::UnsupportedVersion)
    ));
}

#[test]
fn closure_typed_bytecode_valid_call_and_capture_roundtrip() {
    use crate::function::BytecodeSignature;
    use crate::serialize::decode_and_verify_artifact;
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let signature_id = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let seven = module.add_constant(BytecodeConstant::Int64(7));
    let ten = module.add_constant(BytecodeConstant::Int64(10));

    let mut target = BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    );
    target.locals = target.params.clone();
    target.max_stack = 2;
    target.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let target_id = module.add_function(target);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(seven),
        BytecodeInstr::MakeClosure(target_id, 1),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::CallClosure(signature_id, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    verify_module(&module).unwrap();
    let first = encode_module(&module).unwrap();
    let second = encode_module(&module).unwrap();
    assert_eq!(first, second);
    let decoded = decode_and_verify_artifact(&first).unwrap();
    assert_eq!(encode_module(&decoded.module).unwrap(), first);
    assert_eq!(
        decoded.module.functions[1].code[3],
        BytecodeInstr::CallClosure(signature_id, 1)
    );
}

#[test]
fn closure_typed_bytecode_rejects_wrong_argument_type() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let signature_id = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let target_id = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::MakeClosure(target_id, 0),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::CallClosure(signature_id, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::StackTypeMismatch {
            expected: BytecodeType::Int64,
            found: BytecodeType::Bool,
        })
    ));
}

#[test]
fn closure_typed_bytecode_rejects_wrong_callee_signature() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let actual_signature = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let expected_signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let zero = module.add_constant(BytecodeConstant::Int64(0));
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![],
        BytecodeType::Int64,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::MakeClosure(target, 0),
        BytecodeInstr::ConstI64(zero),
        BytecodeInstr::CallClosure(expected_signature, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert_ne!(actual_signature, expected_signature);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::StackTypeMismatch {
            expected: BytecodeType::Callable(_),
            found: BytecodeType::Callable(_),
        })
    ));
}

#[test]
fn closure_typed_bytecode_rejects_invalid_signature_reference() {
    use crate::function::SignatureId;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::CallClosure(SignatureId(999), 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::InvalidSignatureId(SignatureId(999)))
    ));
}

#[test]
fn closure_typed_bytecode_rejects_missing_public_signature() {
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let target_id = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::MakeClosure(target_id, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::MissingClosureSignature(id)) if id == target_id
    ));
}

#[test]
fn closure_typed_bytecode_rejects_wrong_capture_type() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Int64,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::MakeClosure(target, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::StackTypeMismatch {
            expected: BytecodeType::Int64,
            found: BytecodeType::Bool,
        })
    ));
}

#[test]
fn closure_typed_bytecode_rejects_wrong_arity() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::CallClosure(signature, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(verify_module(&module).is_err());
}

#[test]
fn closure_typed_bytecode_rejects_previous_cache_format() {
    use crate::serialize::decode_artifact;
    let bytes = encode_module(&BytecodeModule::new()).unwrap();
    let mut old_version = bytes.clone();
    old_version[4..8].copy_from_slice(&4_u32.to_le_bytes());
    assert_eq!(
        decode_artifact(&old_version).unwrap_err(),
        BytecodeSerializeError::UnsupportedVersion
    );
}

// VM Parity 2B.1e.1: the wire format never accepts unverified borrow opcodes.
#[test]
fn borrowed_closure_bytecode_opcode_identity_and_fail_closed() {
    use crate::verify::verify_module;

    let mut module = BytecodeModule::new();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);

    assert_eq!(BytecodeInstr::BorrowLocal(LocalId(0)).opcode_id(), 55);
    assert_eq!(
        BytecodeInstr::MakeClosureRef(FunctionId(0), 1).opcode_id(),
        56
    );
    assert_eq!(crate::opcode::BYTECODE_OPCODE_COUNT, 64);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
    let bytes = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&bytes),
        Err(BytecodeSerializeError::VerifyError(_))
    ));
}

// VM Parity 2B.1e.2: artifact-level non-escape gate.
#[test]
fn borrowed_closure_verifier_rejects_return_escape() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let public_sig = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Callable(public_sig));
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
}

#[test]
fn borrowed_closure_verifier_rejects_source_reassignment() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Unit,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let target = module.add_function(BytecodeFunction::new(
        "lambda".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Unit,
    ));
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
}

// VM Parity 2B.2: CFG-aware artifact proof. These tests deliberately keep
// the 2B.1 owner initialization restriction; source-level NLL is separate.
fn borrowed_cfg_fixture() -> (
    BytecodeModule,
    FunctionId,
    FunctionId,
    crate::function::SignatureId,
    crate::constant::ConstId,
) {
    use crate::function::BytecodeSignature;
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Unit,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let mut target = BytecodeFunction::new(
        "borrowed_body".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Unit,
    );
    target.locals = vec![BytecodeType::Int64];
    target.code = vec![BytecodeInstr::Return];
    let borrowed = module.add_function(target);

    let mut plain = BytecodeFunction::new("plain_body".into(), vec![], BytecodeType::Unit);
    plain.code = vec![BytecodeInstr::Return];
    let plain = module.add_function(plain);
    (module, borrowed, plain, signature, ten)
}

#[test]
fn borrowed_cfg_accepts_calls_in_separate_branches() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, sig, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(sig)];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),                // 0
        BytecodeInstr::StoreLocal(LocalId(0)),       // 1
        BytecodeInstr::BorrowLocal(LocalId(0)),      // 2
        BytecodeInstr::MakeClosureRef(target, 1),    // 3
        BytecodeInstr::StoreLocal(LocalId(1)),       // 4
        BytecodeInstr::ConstBool(true),              // 5
        BytecodeInstr::JumpIfFalse(InstrOffset(10)), // 6
        BytecodeInstr::LoadLocal(LocalId(1)),        // 7
        BytecodeInstr::CallClosure(sig, 0),          // 8
        BytecodeInstr::Jump(InstrOffset(12)),        // 9
        BytecodeInstr::LoadLocal(LocalId(1)),        // 10
        BytecodeInstr::CallClosure(sig, 0),          // 11
        BytecodeInstr::Return,                       // 12
    ];
    module.add_function(main);
    assert!(verify_module(&module).is_ok());
}

#[test]
fn borrowed_cfg_accepts_same_owner_join() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, sig, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(7)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Jump(InstrOffset(9)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::CallClosure(sig, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(verify_module(&module).is_ok());
}

#[test]
fn borrowed_cfg_accepts_owner_initialized_before_loop() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, sig, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(sig)];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(10)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::CallClosure(sig, 0),
        BytecodeInstr::Jump(InstrOffset(5)),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(verify_module(&module).is_ok());
}

#[test]
fn borrowed_cfg_rejects_initializer_that_repeats_in_loop() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, _, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(8)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Pop,
        BytecodeInstr::Jump(InstrOffset(0)),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(
        matches!(verify_module(&module), Err(BytecodeVerifyError::Unsupported(s))
        if s.contains("non-repeating initialization"))
    );
}

#[test]
fn borrowed_cfg_rejects_conditional_return_escape_after_join() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, plain, sig, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Callable(sig));
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(7)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Jump(InstrOffset(8)),
        BytecodeInstr::MakeClosure(plain, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(
        matches!(verify_module(&module), Err(BytecodeVerifyError::Unsupported(s))
        if s.contains("may not be returned"))
    );
}

#[test]
fn borrowed_cfg_rejects_incompatible_handle_origins_at_join() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, _, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(9)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::Jump(InstrOffset(10)),
        BytecodeInstr::Unreachable,
        BytecodeInstr::BorrowLocal(LocalId(1)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(
        matches!(verify_module(&module), Err(BytecodeVerifyError::Unsupported(s))
        if s.contains("incompatible borrowed-handle provenance"))
    );
}

#[test]
fn borrowed_cfg_rejects_branch_only_owner_initialization() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, _, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(4)),
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::LocalNotInitialized(LocalId(0)))
    ));
}

#[test]
fn borrowed_cfg_rejects_handle_drop_on_one_branch() {
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let (mut module, target, _, _, ten) = borrowed_cfg_fixture();
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(7)),
        BytecodeInstr::MakeClosureRef(target, 1),
        BytecodeInstr::Jump(InstrOffset(8)),
        BytecodeInstr::Pop,
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(verify_module(&module).is_err());
}

#[test]
fn verifier_rechecks_widened_stack_types_after_cfg_merge() {
    use crate::function::BytecodeSignature;
    use crate::opcode::InstrOffset;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let sig = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Unit,
    });
    let variant = module.add_constant(BytecodeConstant::String("Case".into()));
    let enum_name = module.add_constant(BytecodeConstant::String("Choice".into()));
    let mut target = BytecodeFunction::new("plain".into(), vec![], BytecodeType::Unit);
    target.code.push(BytecodeInstr::Return);
    let target = module.add_function(target);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.max_stack = 2;
    // Visit the callable path first. A second predecessor then widens its
    // input to Any; the call site must be verified again and rejected.
    main.code = vec![
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::JumpIfFalse(InstrOffset(5)),
        BytecodeInstr::MakeEnum(enum_name, variant, 0),
        BytecodeInstr::Jump(InstrOffset(6)),
        BytecodeInstr::Unreachable,
        BytecodeInstr::MakeClosure(target, 0),
        BytecodeInstr::CallClosure(sig, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::StackTypeMismatch {
            expected: BytecodeType::Callable(_),
            found: BytecodeType::Any,
        })
    ));
}

// VM borrowed callback completion: a bytecode artifact must prove that every
// direct forwarding edge invokes (rather than returns/stores/exports) its
// borrowed callback, independently of source-level callback summaries.
fn borrowed_callback_module() -> BytecodeModule {
    use crate::function::BytecodeSignature;
    let mut module = BytecodeModule::new();
    let signature = module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Int64,
    });
    let base = module.add_constant(BytecodeConstant::Int64(10));
    let step = module.add_constant(BytecodeConstant::Int64(5));

    let mut lambda = BytecodeFunction::new(
        "borrowed_body".into(),
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
    let body_id = module.add_function(lambda);

    let mut invoke = BytecodeFunction::new(
        "invoke".into(),
        vec![BytecodeType::Callable(signature), BytecodeType::Int64],
        BytecodeType::Int64,
    );
    invoke.locals = invoke.params.clone();
    invoke.max_stack = 2;
    invoke.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::CallClosure(signature, 1),
        BytecodeInstr::Return,
    ];
    let invoke_id = module.add_function(invoke);

    let mut twice = BytecodeFunction::new(
        "twice".into(),
        vec![BytecodeType::Callable(signature), BytecodeType::Int64],
        BytecodeType::Int64,
    );
    twice.locals = twice.params.clone();
    twice.max_stack = 3;
    twice.code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::Call(invoke_id, 2),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::Call(invoke_id, 2),
        BytecodeInstr::AddI64,
        BytecodeInstr::Return,
    ];
    let twice_id = module.add_function(twice);

    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(signature)];
    main.max_stack = 2;
    main.code = vec![
        BytecodeInstr::ConstI64(base),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureRef(body_id, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::ConstI64(step),
        BytecodeInstr::Call(twice_id, 2),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    module
}

#[test]
fn borrowed_callback_transitive_bytecode_roundtrip_and_determinism() {
    use crate::verify::verify_module;
    let module = borrowed_callback_module();
    verify_module(&module).unwrap();
    let encoded = encode_module(&module).unwrap();
    let decoded = decode_and_verify_artifact(&encoded).unwrap();
    assert_eq!(encode_module(&decoded.module).unwrap(), encoded);
    assert_eq!(encode_module(&module).unwrap(), encoded);
    assert!(decoded.module.functions[3]
        .code
        .iter()
        .any(|op| matches!(op, BytecodeInstr::BorrowLocal(LocalId(0)))));
}

#[test]
fn borrowed_callback_rejects_aggregation_in_direct_callee() {
    use crate::verify::verify_module;
    let mut module = borrowed_callback_module();
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    // A callable parameter is placed inside an aggregate. This would hide
    // the borrowed frame dependency from the direct call boundary.
    module.functions[1].code = vec![
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::MakeTuple(1),
        BytecodeInstr::Pop,
        BytecodeInstr::ConstI64(ten),
        BytecodeInstr::Return,
    ];
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
    let encoded = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&encoded),
        Err(BytecodeSerializeError::VerifyError(_))
    ));
}

#[test]
fn borrowed_callback_rejects_unverified_indirect_forward() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = borrowed_callback_module();
    let callback_type = module.functions[1].params[0];
    let higher_order_signature = module.intern_signature(BytecodeSignature {
        params: vec![callback_type],
        ret: BytecodeType::Int64,
    });
    let ten = module.add_constant(BytecodeConstant::Int64(10));
    let mut unknown = BytecodeFunction::new(
        "indirect_target".into(),
        vec![callback_type],
        BytecodeType::Int64,
    );
    unknown.locals = unknown.params.clone();
    unknown.max_stack = 1;
    unknown.code = vec![BytecodeInstr::ConstI64(ten), BytecodeInstr::Return];
    let unknown_id = module.add_function(unknown);
    // invoke() passes the incoming callback as an argument of an indirect
    // callable. It cannot be proven non-escaping merely because this test's
    // current closure target happens to be benign.
    module.functions[1].code = vec![
        BytecodeInstr::MakeClosure(unknown_id, 0),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::CallClosure(higher_order_signature, 1),
        BytecodeInstr::Return,
    ];
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
}

#[test]
fn borrowed_callback_rejects_invalid_serialized_handle_and_target() {
    let mut module = borrowed_callback_module();
    module.functions[3].code[2] = BytecodeInstr::BorrowLocal(LocalId(u32::MAX));
    let encoded = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&encoded),
        Err(BytecodeSerializeError::VerifyError(_))
    ));

    let mut module = borrowed_callback_module();
    module.functions[3].code[3] = BytecodeInstr::MakeClosureRef(FunctionId(u32::MAX), 1);
    let encoded = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&encoded),
        Err(BytecodeSerializeError::VerifyError(_))
    ));
}

// VM Parity 2B.5.A: exclusive borrows cannot be upgraded from immutable handles.
#[test]
fn inout_closure_rejects_immutable_handle_upgrade() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let sig = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let init = module.add_constant(BytecodeConstant::Int64(5));
    let mut child = BytecodeFunction::new(
        "child".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    child.inout_params = vec![true];
    child.locals = child.params.clone();
    child.max_stack = 1;
    child.code = vec![BytecodeInstr::LoadLocal(LocalId(0)), BytecodeInstr::Return];
    let child_id = module.add_function(child);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(sig)];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstI64(init),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocal(LocalId(0)),
        BytecodeInstr::MakeClosureInout(child_id, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::CallClosure(sig, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
    let bytes = encode_module(&module).unwrap();
    assert!(decode_and_verify_artifact(&bytes).is_err());
}

#[test]
fn inout_closure_rejects_repeated_exclusive_borrow_sites() {
    use crate::function::BytecodeSignature;
    use crate::verify::verify_module;
    let mut module = BytecodeModule::new();
    let sig = module.intern_signature(BytecodeSignature {
        params: vec![],
        ret: BytecodeType::Int64,
    });
    let init = module.add_constant(BytecodeConstant::Int64(5));
    let mut child = BytecodeFunction::new(
        "child".into(),
        vec![BytecodeType::Int64],
        BytecodeType::Int64,
    );
    child.inout_params = vec![true];
    child.locals = child.params.clone();
    child.max_stack = 1;
    child.code = vec![BytecodeInstr::LoadLocal(LocalId(0)), BytecodeInstr::Return];
    let child_id = module.add_function(child);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Int64);
    main.locals = vec![BytecodeType::Int64, BytecodeType::Callable(sig)];
    main.max_stack = 1;
    main.code = vec![
        BytecodeInstr::ConstI64(init),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::MakeClosureInout(child_id, 1),
        BytecodeInstr::StoreLocal(LocalId(1)),
        BytecodeInstr::BorrowLocalMut(LocalId(0)),
        BytecodeInstr::MakeClosureInout(child_id, 1),
        BytecodeInstr::Pop,
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::CallClosure(sig, 0),
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    assert!(matches!(
        verify_module(&module),
        Err(BytecodeVerifyError::Unsupported(_))
    ));
}

#[test]
fn mutable_slice_isa_reserves_stable_opcode_ids() {
    assert_eq!(BytecodeInstr::BorrowSliceMut(LocalId(0)).opcode_id(), 59);
    assert_eq!(BytecodeInstr::InoutSliceGet(LocalId(0)).opcode_id(), 60);
    assert_eq!(BytecodeInstr::InoutSliceSet(LocalId(0)).opcode_id(), 61);
    assert_eq!(BytecodeInstr::InoutSliceLen(LocalId(0)).opcode_id(), 62);
    assert_eq!(crate::opcode::BYTECODE_OPCODE_COUNT, 64);
}

/// Construct a typed closure with independently chosen target access metadata.
fn a1_callable_access_fixture(mode: u8, access: Vec<bool>) -> BytecodeModule {
    use crate::function::BytecodeSignature;
    let mut module = BytecodeModule::new();
    module.intern_signature(BytecodeSignature {
        params: vec![BytecodeType::Int64],
        ret: BytecodeType::Unit,
    });
    let init = module.add_constant(BytecodeConstant::Int64(10));
    let mut target = BytecodeFunction::new(
        "target".into(),
        vec![BytecodeType::Int64, BytecodeType::Int64],
        BytecodeType::Unit,
    );
    target.inout_params = access;
    target.locals = target.params.clone();
    target.code = vec![BytecodeInstr::Return];
    let target_id = module.add_function(target);
    let mut main = BytecodeFunction::new("main".into(), vec![], BytecodeType::Unit);
    main.locals = vec![BytecodeType::Int64];
    main.max_stack = 1;
    let (source, construct) = match mode {
        0 => (
            BytecodeInstr::LoadLocal(LocalId(0)),
            BytecodeInstr::MakeClosure(target_id, 1),
        ),
        1 => (
            BytecodeInstr::BorrowLocal(LocalId(0)),
            BytecodeInstr::MakeClosureRef(target_id, 1),
        ),
        2 => (
            BytecodeInstr::BorrowLocalMut(LocalId(0)),
            BytecodeInstr::MakeClosureInout(target_id, 1),
        ),
        _ => unreachable!(),
    };
    main.code = vec![
        BytecodeInstr::ConstI64(init),
        BytecodeInstr::StoreLocal(LocalId(0)),
        source,
        construct,
        BytecodeInstr::Pop,
        BytecodeInstr::Return,
    ];
    module.add_function(main);
    module
}

#[test]
fn a1_callable_access_metadata_must_match_constructor() {
    use crate::verify::verify_module;
    for (mode, access, valid) in [
        (0, vec![false, false], true),
        (1, vec![false, false], true),
        (2, vec![true, false], true),
        (0, vec![true, false], false),
        (1, vec![true, false], false),
        (2, vec![false, false], false),
        (0, vec![false, true], false),
        (1, vec![false, true], false),
        (2, vec![true, true], false),
    ] {
        let module = a1_callable_access_fixture(mode, access);
        let result = verify_module(&module);
        if valid {
            result.unwrap();
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("closure capture access"));
        }
        let bytes = encode_module(&module).unwrap();
        assert_eq!(decode_and_verify_artifact(&bytes).is_ok(), valid);
    }
}

#[test]
fn a1_ref_hidden_parameter_cannot_be_written() {
    let mut module = a1_callable_access_fixture(1, vec![false, false]);
    let target = &mut module.functions[0];
    target.max_stack = 1;
    target.code = vec![
        BytecodeInstr::ConstI64(crate::constant::ConstId(0)),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::Return,
    ];
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error
        .to_string()
        .contains("ref closure target cannot mutate"));
}

#[test]
fn a1_raw_borrow_cannot_be_stored_as_ordinary_value() {
    let mut module = a1_callable_access_fixture(2, vec![true, false]);
    let main = &mut module.functions[1];
    main.locals.push(BytecodeType::Int64);
    main.code.truncate(3);
    main.code
        .extend([BytecodeInstr::StoreLocal(LocalId(1)), BytecodeInstr::Return]);
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("borrow handle cannot be stored"));
}

#[test]
fn a1_previous_loadlocal_format_is_rejected() {
    let module = a1_callable_access_fixture(2, vec![true, false]);
    let mut bytes = encode_module(&module).unwrap();
    assert_eq!(&bytes[4..8], &12_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&9_u32.to_le_bytes());
    assert!(matches!(
        decode_artifact(&bytes),
        Err(BytecodeSerializeError::UnsupportedVersion)
    ));
}

fn a3_2a_read_return_join_fixture(other_owner: bool) -> BytecodeModule {
    use crate::opcode::InstrOffset;
    let mut module = BytecodeModule::new();
    let mut function = BytecodeFunction::new(
        "select".into(),
        vec![
            BytecodeType::ReadSlice,
            BytecodeType::ReadSlice,
            BytecodeType::Bool,
        ],
        BytecodeType::ReadSlice,
    );
    function.return_borrow = Some(0);
    function.locals = function.params.clone();
    function.locals.push(BytecodeType::ReadSlice);
    function.max_stack = 1;
    function.code = vec![
        BytecodeInstr::LoadLocal(LocalId(2)),
        BytecodeInstr::JumpIfFalse(InstrOffset(5)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::StoreLocal(LocalId(3)),
        BytecodeInstr::Jump(InstrOffset(7)),
        BytecodeInstr::LoadLocal(LocalId(if other_owner { 1 } else { 0 })),
        BytecodeInstr::StoreLocal(LocalId(3)),
        BytecodeInstr::LoadLocal(LocalId(3)),
        BytecodeInstr::Return,
    ];
    module.add_function(function);
    module
}

#[test]
fn a3_2a_read_return_contract_roundtrip_and_format() {
    let module = a3_2a_read_return_join_fixture(false);
    let encoded = encode_module(&module).unwrap();
    let artifact = decode_and_verify_artifact(&encoded).unwrap();
    let function = &artifact.module.functions[0];
    assert_eq!(function.return_borrow, Some(0));
    assert_eq!(function.ret, BytecodeType::ReadSlice);
    assert_eq!(function.params[0], BytecodeType::ReadSlice);
    assert_eq!(function.locals[3], BytecodeType::ReadSlice);
    assert_eq!(encode_module(&artifact.module).unwrap(), encoded);
    assert_eq!(&encoded[4..8], &12_u32.to_le_bytes());
    let mut old = encoded;
    old[4..8].copy_from_slice(&11_u32.to_le_bytes());
    assert!(matches!(
        decode_artifact(&old),
        Err(BytecodeSerializeError::UnsupportedVersion)
    ));
}

#[test]
fn a3_2a_read_return_different_origin_at_join_is_rejected() {
    let module = a3_2a_read_return_join_fixture(true);
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("read slice return must derive"));
    let bytes = encode_module(&module).unwrap();
    assert!(matches!(
        decode_and_verify_artifact(&bytes),
        Err(BytecodeSerializeError::VerifyError(_))
    ));
}

#[test]
fn a3_2a_read_return_missing_or_forged_metadata_is_rejected() {
    let mut module = a3_2a_read_return_join_fixture(false);
    for contract in [None, Some(1), Some(2), Some(u32::MAX)] {
        module.functions[0].return_borrow = contract;
        let error = crate::verify::verify_module(&module).unwrap_err();
        assert!(error.to_string().contains("read slice return must derive"));
    }
}

#[test]
fn a3_2a_read_return_any_cannot_manufacture_origin() {
    let mut module = a3_2a_read_return_join_fixture(false);
    let function = &mut module.functions[0];
    function.params[1] = BytecodeType::Any;
    function.locals[1] = BytecodeType::Any;
    function.code = vec![BytecodeInstr::LoadLocal(LocalId(1)), BytecodeInstr::Return];
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("read slice return must derive"));
}

#[test]
fn a3_2a_read_return_local_array_cannot_manufacture_origin() {
    let mut module = a3_2a_read_return_join_fixture(false);
    let zero = module.add_constant(BytecodeConstant::Int64(0));
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let function = &mut module.functions[0];
    function.max_stack = 3;
    function.code = vec![
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::MakeArray(1),
        BytecodeInstr::ConstI64(zero),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::Slice,
        BytecodeInstr::Return,
    ];
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("read slice return must derive"));
}

#[test]
fn a3_2a_read_return_direct_call_uses_declared_argument() {
    let mut module = a3_2a_read_return_join_fixture(false);
    let mut forward = BytecodeFunction::new(
        "forward".into(),
        vec![BytecodeType::ReadSlice, BytecodeType::ReadSlice],
        BytecodeType::ReadSlice,
    );
    forward.return_borrow = Some(1);
    forward.locals = forward.params.clone();
    forward.max_stack = 3;
    forward.code = vec![
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::ConstBool(true),
        BytecodeInstr::Call(FunctionId(0), 3),
        BytecodeInstr::Return,
    ];
    let id = module.add_function(forward);
    crate::verify::verify_module(&module).unwrap();
    module.functions[id.0 as usize].return_borrow = Some(0);
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("read slice return must derive"));
}

fn a3_2b1_downgrade_fixture() -> BytecodeModule {
    let mut module = BytecodeModule::new();
    let zero = module.add_constant(BytecodeConstant::Int64(0));
    let one = module.add_constant(BytecodeConstant::Int64(1));
    let value = module.add_constant(BytecodeConstant::Int64(99));
    let mut normalize = BytecodeFunction::new(
        "normalize".into(),
        vec![BytecodeType::ExclusiveSlice],
        BytecodeType::ReadSlice,
    );
    normalize.inout_params[0] = true;
    normalize.return_borrow = Some(0);
    normalize.locals = normalize.params.clone();
    normalize.max_stack = 3;
    normalize.code = vec![
        BytecodeInstr::ConstI64(zero),
        BytecodeInstr::ConstI64(value),
        BytecodeInstr::InoutSliceSet(LocalId(0)),
        BytecodeInstr::ConstI64(zero),
        BytecodeInstr::ConstI64(one),
        BytecodeInstr::ReadInoutSlice(LocalId(0)),
        BytecodeInstr::Return,
    ];
    module.add_function(normalize);
    module
}

#[test]
fn a3_2b1_downgrade_opcode_roundtrip() {
    let module = a3_2b1_downgrade_fixture();
    assert_eq!(BytecodeInstr::ReadInoutSlice(LocalId(0)).opcode_id(), 63);
    let bytes = encode_module(&module).unwrap();
    assert_eq!(&bytes[4..8], &12_u32.to_le_bytes());
    let decoded = decode_and_verify_artifact(&bytes).unwrap();
    assert_eq!(decoded.module.functions[0].return_borrow, Some(0));
    assert_eq!(decoded.module.functions[0].code, module.functions[0].code);
    assert_eq!(encode_module(&decoded.module).unwrap(), bytes);
}

#[test]
fn a3_2b1_downgrade_rejects_forged_parameter_provenance() {
    let mut module = a3_2b1_downgrade_fixture();
    let f = &mut module.functions[0];
    f.params.push(BytecodeType::ExclusiveSlice);
    f.locals.push(BytecodeType::ExclusiveSlice);
    f.inout_params.push(true);
    f.code[5] = BytecodeInstr::ReadInoutSlice(LocalId(1));
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("read slice return must derive"));
}

#[test]
fn a3_2b1_verifier_rejects_write_while_read_operand_is_live() {
    let mut module = a3_2b1_downgrade_fixture();
    let f = &mut module.functions[0];
    f.code[..6].rotate_left(3);
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("frame read loan is live"));
    let bytes = encode_module(&module).unwrap();
    assert!(decode_and_verify_artifact(&bytes).is_err());
}

#[test]
fn a3_2b1_verifier_rejects_opaque_aggregate_of_frame_read_loan() {
    let mut module = a3_2b1_downgrade_fixture();
    let f = &mut module.functions[0];
    f.ret = BytecodeType::Unit;
    f.return_borrow = None;
    f.code.insert(6, BytecodeInstr::MakeTuple(1));
    f.code.insert(7, BytecodeInstr::Pop);
    let error = crate::verify::verify_module(&module).unwrap_err();
    assert!(error.to_string().contains("unsupported operation or ABI"));
}

#[test]
fn a3_2b1_local_liveness_handles_branches_and_overwrite() {
    use crate::opcode::InstrOffset;
    let mut f = BytecodeFunction::new(
        "liveness".into(),
        vec![
            BytecodeType::ReadSlice,
            BytecodeType::ReadSlice,
            BytecodeType::Bool,
        ],
        BytecodeType::Unit,
    );
    f.locals = f.params.clone();
    f.max_stack = 1;
    // The current value is read on one reachable branch.
    f.code = vec![
        BytecodeInstr::LoadLocal(LocalId(2)),
        BytecodeInstr::JumpIfFalse(InstrOffset(6)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Len,
        BytecodeInstr::Pop,
        BytecodeInstr::Jump(InstrOffset(6)),
        BytecodeInstr::Return,
    ];
    assert!(crate::verify::local_value_is_live(&f, 0, LocalId(0)));
    assert!(!crate::verify::local_value_is_live(&f, 6, LocalId(0)));
    // Rebinding before the only read kills the old value.
    f.code = vec![
        BytecodeInstr::LoadLocal(LocalId(2)),
        BytecodeInstr::JumpIfFalse(InstrOffset(8)),
        BytecodeInstr::LoadLocal(LocalId(1)),
        BytecodeInstr::StoreLocal(LocalId(0)),
        BytecodeInstr::LoadLocal(LocalId(0)),
        BytecodeInstr::Len,
        BytecodeInstr::Pop,
        BytecodeInstr::Jump(InstrOffset(8)),
        BytecodeInstr::Return,
    ];
    assert!(!crate::verify::local_value_is_live(&f, 0, LocalId(0)));
}
