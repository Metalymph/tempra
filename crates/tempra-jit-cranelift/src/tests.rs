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



#[test]
fn jit_int64_functions_still_eligible() {
    let mut module = BytecodeModule::new();
    let func = BytecodeFunction::new("test".into(), vec![BytecodeType::Int64], BytecodeType::Int64);
    let id = module.add_function(func);
    
    assert!(crate::engine::can_jit_function(&module, id));
}

#[test]
fn jit_helper_string_len_ascii() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_string("hello".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_ascii", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 5);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_string_len_unicode() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_string("hé".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_unicode", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 2);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_string_len_invalid_handle() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_invalid", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, 9999);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_INVALID_HANDLE);
}

#[test]
fn jit_helper_string_len_null_context() {
    use crate::helpers::tmpr_jit_string_len;
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_null_ctx", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(0, 1001); // Null context
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_INVALID_CONTEXT);
}

#[test]
fn jit_helper_string_size_ascii() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_size};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_string("hello".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_size as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_size_ascii", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 5); // "hello" bytes
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_string_size_unicode() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_size};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    
    // "👋" is 1 scalar, but 4 bytes in UTF-8
    let handle = ctx.alloc_string("👋".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_size as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_size_unicode", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 4); // "👋" bytes
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_array_len_int64() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![
        JitHeapScalar::Int64(10),
        JitHeapScalar::Int64(20),
        JitHeapScalar::Int64(30),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_array_len", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 3);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_array_len_type_error_on_string() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_array_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_string("oops".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_array_len_bad", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_TYPE);
}

#[test]
fn jit_helper_string_len_type_error_on_array() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_string_len};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![JitHeapScalar::Int64(1)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let arg_ctx = builder.block_params(block)[0];
    let arg_handle = builder.block_params(block)[1];

    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_string_len as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &[arg_ctx, arg_handle]);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_string_len_bad", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_TYPE);
}

#[test]
fn jit_helper_array_get_i64_first() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![
        JitHeapScalar::Int64(42),
        JitHeapScalar::Int64(84),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // index
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_get_first", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, i64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0, 0);
    assert_eq!(res.value, 42);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_array_get_i64_negative_value() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![
        JitHeapScalar::Int64(-7),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // index
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_get_neg", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, i64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0, 0);
    assert_eq!(res.value as i64, -7); // Bit-preserving roundtrip
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_array_get_i64_out_of_bounds() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![JitHeapScalar::Int64(10)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // index
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_get_oob", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, i64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    // Test index 1 (OOB for length 1)
    let res = native_fn(ctx_ptr as u64, handle.0, 1);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_OUT_OF_BOUNDS);

    // Test index -1 (OOB)
    let res = native_fn(ctx_ptr as u64, handle.0, -1);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_OUT_OF_BOUNDS);
}

#[test]
fn jit_helper_array_get_i64_type_error_on_non_i64() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![JitHeapScalar::Bool(true)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // index
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_get_type_bad", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, i64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0, 0);
    assert_eq!(res.status, crate::abi::JIT_STATUS_ERROR_TYPE);
}

#[test]
fn jit_helper_status_consumed_array_get_ok() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![
        JitHeapScalar::Int64(42),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // index
    sig.returns.push(AbiParam::new(types::I64)); // result (value if OK, else -1)
    engine.ctx.func.signature = sig.clone();

    // Helper signature
    let mut helper_sig = engine.module.make_signature();
    helper_sig.params.push(AbiParam::new(types::I64));
    helper_sig.params.push(AbiParam::new(types::I64));
    helper_sig.params.push(AbiParam::new(types::I64));
    helper_sig.returns.push(AbiParam::new(types::I64)); // value
    helper_sig.returns.push(AbiParam::new(types::I64)); // status

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(helper_sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_array_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    let val = results[0];
    let status = results[1];

    let success_block = builder.create_block();
    let error_block = builder.create_block();

    let is_ok = builder.ins().icmp_imm(IntCC::Equal, status, 0); // JIT_STATUS_SUCCESS = 0
    builder.ins().brif(is_ok, success_block, &[], error_block, &[]);

    builder.switch_to_block(success_block);
    builder.seal_block(success_block);
    builder.ins().return_(&[val]);

    builder.switch_to_block(error_block);
    builder.seal_block(error_block);
    let sentinel = builder.ins().iconst(types::I64, -1);
    builder.ins().return_(&[sentinel]);

    builder.finalize();

    let func_id = engine.module.declare_function("test_status_check", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, i64) -> i64 = unsafe { std::mem::transmute(code) };
    
    // 1. Success case
    assert_eq!(native_fn(ctx_ptr as u64, handle.0, 0), 42);

    // 2. OOB case -> -1
    assert_eq!(native_fn(ctx_ptr as u64, handle.0, 1), -1);

    // 3. Null context -> -1
    assert_eq!(native_fn(0, handle.0, 0), -1);

    // 4. Invalid handle -> -1
    assert_eq!(native_fn(ctx_ptr as u64, 9999, 0), -1);
}

#[test]
fn jit_runtime_error_from_status_mappings() {
    use crate::error::JitRuntimeError;
    use crate::abi::*;

    assert_eq!(JitRuntimeError::from_status(JIT_STATUS_SUCCESS), None);
    assert_eq!(JitRuntimeError::from_status(JIT_STATUS_ERROR_INVALID_CONTEXT), Some(JitRuntimeError::InvalidContext));
    assert_eq!(JitRuntimeError::from_status(JIT_STATUS_ERROR_INVALID_HANDLE), Some(JitRuntimeError::InvalidHandle));
    assert_eq!(JitRuntimeError::from_status(JIT_STATUS_ERROR_TYPE), Some(JitRuntimeError::TypeError));
    assert_eq!(JitRuntimeError::from_status(JIT_STATUS_ERROR_OUT_OF_BOUNDS), Some(JitRuntimeError::OutOfBounds));
    assert_eq!(JitRuntimeError::from_status(999), Some(JitRuntimeError::HelperFailed { status: 999 }));
}

#[test]
fn jit_helper_result_wrapper_api() {
    use crate::abi::*;
    use crate::error::JitRuntimeError;

    // 1. OK path
    let res = JitHelperResult { value: 42, status: JIT_STATUS_SUCCESS };
    assert_eq!(res.to_result(), Ok(42));
    assert_eq!(res.to_i64_result(), Ok(42));

    // 2. Negative i64 roundtrip
    let res = JitHelperResult { value: -7i64 as u64, status: JIT_STATUS_SUCCESS };
    assert_eq!(res.to_i64_result(), Ok(-7));

    // 3. Error path - OOB
    let res = JitHelperResult { value: 0, status: JIT_STATUS_ERROR_OUT_OF_BOUNDS };
    assert_eq!(res.to_result(), Err(JitRuntimeError::OutOfBounds));

    // 4. Error path - TypeError
    let res = JitHelperResult { value: 0, status: JIT_STATUS_ERROR_TYPE };
    assert_eq!(res.to_result(), Err(JitRuntimeError::TypeError));

    // 5. Error path - InvalidHandle
    let res = JitHelperResult { value: 0, status: JIT_STATUS_ERROR_INVALID_HANDLE };
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidHandle));

    // 6. Error path - InvalidContext
    let res = JitHelperResult { value: 0, status: JIT_STATUS_ERROR_INVALID_CONTEXT };
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidContext));

    // 7. Error path - Unknown
    let res = JitHelperResult { value: 0, status: 999 };
    assert_eq!(res.to_result(), Err(JitRuntimeError::HelperFailed { status: 999 }));
}

#[test]
fn jit_helper_struct_get_i64_first_field() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_struct_get_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_struct("Point", vec![
        ("x".to_string(), JitHeapScalar::Int64(10)),
        ("y".to_string(), JitHeapScalar::Int64(20)),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // field_id
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_struct_get_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_struct_get", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0, 0);
    assert_eq!(res.value, 10);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);

    let res = native_fn(ctx_ptr as u64, handle.0, 1);
    assert_eq!(res.value, 20);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_struct_get_i64_negative_value() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_struct_get_i64};
    
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_struct("S", vec![
        ("f".to_string(), JitHeapScalar::Int64(-42)),
    ]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;
    
    // Direct call probe to verify wrapper
    let res = tmpr_jit_struct_get_i64(ctx_ptr, handle.0, 0);
    let val = res.to_i64_result().expect("Should be Ok");
    assert_eq!(val, -42);
}

#[test]
fn jit_helper_struct_get_i64_errors() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_struct_get_i64};
    use crate::error::JitRuntimeError;
    
    let mut ctx = JitRuntimeContext::new();
    let s_handle = ctx.alloc_struct("S", vec![("f".to_string(), JitHeapScalar::Bool(true))]);
    let a_handle = ctx.alloc_array(vec![]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;
    
    // 1. OOB field_id
    assert_eq!(tmpr_jit_struct_get_i64(ctx_ptr, s_handle.0, 1).to_i64_result(), Err(JitRuntimeError::OutOfBounds));
    
    // 2. Type error (accessing non-i64 field)
    assert_eq!(tmpr_jit_struct_get_i64(ctx_ptr, s_handle.0, 0).to_i64_result(), Err(JitRuntimeError::TypeError));
    
    // 3. Type error (calling struct helper on array)
    assert_eq!(tmpr_jit_struct_get_i64(ctx_ptr, a_handle.0, 0).to_i64_result(), Err(JitRuntimeError::TypeError));
    
    // 4. Invalid handle
    assert_eq!(tmpr_jit_struct_get_i64(ctx_ptr, 9999, 0).to_i64_result(), Err(JitRuntimeError::InvalidHandle));
    
    // 5. Null context
    assert_eq!(tmpr_jit_struct_get_i64(std::ptr::null_mut(), s_handle.0, 0).to_i64_result(), Err(JitRuntimeError::InvalidContext));
}

#[test]
fn jit_helper_enum_tag_option_some() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_enum_tag};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_enum("Option", "Some", 1, vec![JitHeapScalar::Int64(42)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.returns.push(AbiParam::new(types::I64)); // tag
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_enum_tag as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_enum_tag", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    let res = native_fn(ctx_ptr as u64, handle.0);
    assert_eq!(res.value, 1);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_enum_tag_errors_and_wrapper() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_enum_tag};
    use crate::error::JitRuntimeError;
    
    let mut ctx = JitRuntimeContext::new();
    let e_handle = ctx.alloc_enum("Option", "None", 0, vec![]);
    let a_handle = ctx.alloc_array(vec![]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;
    
    // 1. Success + Wrapper
    let res = tmpr_jit_enum_tag(ctx_ptr, e_handle.0);
    assert_eq!(res.to_result(), Ok(0));
    
    // 2. Type error (calling enum helper on array)
    assert_eq!(tmpr_jit_enum_tag(ctx_ptr, a_handle.0).to_result(), Err(JitRuntimeError::TypeError));
    
    // 3. Invalid handle
    assert_eq!(tmpr_jit_enum_tag(ctx_ptr, 9999).to_result(), Err(JitRuntimeError::InvalidHandle));
    
    // 4. Null context
    assert_eq!(tmpr_jit_enum_tag(std::ptr::null_mut(), e_handle.0).to_result(), Err(JitRuntimeError::InvalidContext));
}

#[test]
fn jit_helper_enum_unwrap_i64_option_some() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_enum_unwrap_i64};
    use cranelift::prelude::*;
    use cranelift_module::Module;

    let mut engine = JitEngine::new();
    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_enum("Option", "Some", 1, vec![JitHeapScalar::Int64(42)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    engine.ctx.clear();
    let mut sig = engine.module.make_signature();
    sig.params.push(AbiParam::new(types::I64)); // ctx
    sig.params.push(AbiParam::new(types::I64)); // handle
    sig.params.push(AbiParam::new(types::I64)); // expected_tag
    sig.params.push(AbiParam::new(types::I64)); // payload_index
    sig.returns.push(AbiParam::new(types::I64)); // value
    sig.returns.push(AbiParam::new(types::I64)); // status
    engine.ctx.func.signature = sig.clone();

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    let block = builder.create_block();
    builder.append_block_params_for_function_params(block);
    builder.switch_to_block(block);
    builder.seal_block(block);

    let args = builder.block_params(block).to_vec();
    let helper_sig_ref = builder.import_signature(sig);
    let helper_addr = builder.ins().iconst(types::I64, tmpr_jit_enum_unwrap_i64 as i64);
    let call = builder.ins().call_indirect(helper_sig_ref, helper_addr, &args);
    let results = builder.inst_results(call).to_vec();
    
    builder.ins().return_(&[results[0], results[1]]);
    builder.finalize();

    let func_id = engine.module.declare_function("test_enum_unwrap", cranelift_module::Linkage::Export, &engine.ctx.func.signature).unwrap();
    engine.module.define_function(func_id, &mut engine.ctx).unwrap();
    engine.module.finalize_definitions().unwrap();

    let code = engine.module.get_finalized_function(func_id);
    let native_fn: extern "C" fn(u64, u64, u64, u64) -> crate::abi::JitHelperResult = unsafe { std::mem::transmute(code) };
    
    // Tag 1 (Some), Index 0 -> 42
    let res = native_fn(ctx_ptr as u64, handle.0, 1, 0);
    assert_eq!(res.value, 42);
    assert_eq!(res.status, crate::abi::JIT_STATUS_SUCCESS);
}

#[test]
fn jit_helper_enum_unwrap_i64_result_and_negative() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_enum_unwrap_i64};
    use crate::error::JitRuntimeError;
    
    let mut ctx = JitRuntimeContext::new();
    // Result::Ok(-7) -> Tag 0, Payload [-7]
    let ok_handle = ctx.alloc_enum("Result", "Ok", 0, vec![JitHeapScalar::Int64(-7)]);
    // Result::Err(404) -> Tag 1, Payload [404]
    let err_handle = ctx.alloc_enum("Result", "Err", 1, vec![JitHeapScalar::Int64(404)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;
    
    // 1. Result::Ok(-7) success
    let res = tmpr_jit_enum_unwrap_i64(ctx_ptr, ok_handle.0, 0, 0);
    assert_eq!(res.to_i64_result(), Ok(-7));
    
    // 2. Wrong variant (unwrap Err as Ok)
    let res = tmpr_jit_enum_unwrap_i64(ctx_ptr, err_handle.0, 0, 0);
    assert_eq!(res.to_i64_result(), Err(JitRuntimeError::WrongVariant));
    
    // 3. OOB payload index
    let res = tmpr_jit_enum_unwrap_i64(ctx_ptr, ok_handle.0, 0, 1);
    assert_eq!(res.to_i64_result(), Err(JitRuntimeError::OutOfBounds));
    
    // 4. Type error (non-enum)
    let a_handle = ctx.alloc_array(vec![]);
    let res = tmpr_jit_enum_unwrap_i64(ctx_ptr, a_handle.0, 0, 0);
    assert_eq!(res.to_i64_result(), Err(JitRuntimeError::TypeError));
}

#[test]
fn jit_alloc_string_empty_returns_handle() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_alloc_string_empty};

    let mut ctx = JitRuntimeContext::new();
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    let res = tmpr_jit_alloc_string_empty(ctx_ptr);
    let handle1 = res.to_result().expect("Should allocate successfully");
    assert_ne!(handle1, 0, "Handle(0) is reserved and invalid");

    let res2 = tmpr_jit_alloc_string_empty(ctx_ptr);
    let handle2 = res2.to_result().expect("Should allocate successfully");
    assert_ne!(handle2, 0);
    assert_ne!(handle1, handle2, "Subsequent allocations must return distinct handles");
}

#[test]
fn jit_alloc_string_empty_handle_can_be_used_with_len() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_alloc_string_empty, tmpr_jit_string_len};

    let mut ctx = JitRuntimeContext::new();
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    let alloc_res = tmpr_jit_alloc_string_empty(ctx_ptr);
    let handle = alloc_res.to_result().expect("Should allocate successfully");

    // Pass the newly allocated handle to another helper
    let len_res = tmpr_jit_string_len(ctx_ptr, handle);
    let len = len_res.to_result().expect("Should get length of valid handle");
    assert_eq!(len, 0, "Empty string should have length 0");
}

#[test]
fn jit_runtime_error_from_status_allocation_failed() {
    use crate::error::JitRuntimeError;
    use crate::abi::JIT_STATUS_ERROR_ALLOCATION_FAILED;

    let err = JitRuntimeError::from_status(JIT_STATUS_ERROR_ALLOCATION_FAILED);
    assert_eq!(err, Some(JitRuntimeError::AllocationFailed));
}

#[test]
fn jit_helper_string_concat_ascii_and_errors() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_concat, tmpr_jit_string_len, tmpr_jit_string_size};
    use crate::error::JitRuntimeError;

    let mut ctx = JitRuntimeContext::new();
    let left = ctx.alloc_string("hello".to_string());
    let right = ctx.alloc_string("world".to_string());
    let a_handle = ctx.alloc_array(vec![]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    // 1. Success case: "hello" + "world"
    let res = tmpr_jit_string_concat(ctx_ptr, left.0, right.0);
    let new_handle_val = res.to_result().expect("Should concatenate successfully");
    
    assert_ne!(new_handle_val, 0);
    assert_ne!(new_handle_val, left.0);
    assert_ne!(new_handle_val, right.0);

    // Verify length of new string (10)
    let len_res = tmpr_jit_string_len(ctx_ptr, new_handle_val);
    assert_eq!(len_res.to_result().unwrap(), 10);

    // Verify size of new string (10)
    let size_res = tmpr_jit_string_size(ctx_ptr, new_handle_val);
    assert_eq!(size_res.to_result().unwrap(), 10);

    // 2. Type error (left is not a string)
    let res = tmpr_jit_string_concat(ctx_ptr, a_handle.0, right.0);
    assert_eq!(res.to_result(), Err(JitRuntimeError::TypeError));

    // 3. Type error (right is not a string)
    let res = tmpr_jit_string_concat(ctx_ptr, left.0, a_handle.0);
    assert_eq!(res.to_result(), Err(JitRuntimeError::TypeError));

    // 4. Invalid handle
    let res = tmpr_jit_string_concat(ctx_ptr, 9999, right.0);
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidHandle));

    // 5. Null context
    let res = tmpr_jit_string_concat(std::ptr::null_mut(), left.0, right.0);
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidContext));
}

#[test]
fn jit_helper_string_concat_unicode() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_string_concat, tmpr_jit_string_len, tmpr_jit_string_size};

    let mut ctx = JitRuntimeContext::new();
    let left = ctx.alloc_string("hé".to_string());
    let right = ctx.alloc_string("👋".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    let res = tmpr_jit_string_concat(ctx_ptr, left.0, right.0);
    let new_handle_val = res.to_result().expect("Should concatenate unicode successfully");
    
    // len("hé") = 2, size("hé") = 3
    // len("👋") = 1, size("👋") = 4
    // len("hé👋") = 3, size("hé👋") = 7
    let len_res = tmpr_jit_string_len(ctx_ptr, new_handle_val);
    assert_eq!(len_res.to_result().unwrap(), 3);

    let size_res = tmpr_jit_string_size(ctx_ptr, new_handle_val);
    assert_eq!(size_res.to_result().unwrap(), 7);
}

#[test]
fn jit_helper_array_push_i64_empty_and_non_empty() {
    use crate::helpers::{JitRuntimeContext, JitHeapScalar, tmpr_jit_array_push_i64, tmpr_jit_array_len, tmpr_jit_array_get_i64};

    let mut ctx = JitRuntimeContext::new();
    let empty_handle = ctx.alloc_array(vec![]);
    let non_empty_handle = ctx.alloc_array(vec![JitHeapScalar::Int64(10), JitHeapScalar::Int64(20)]);
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    // 1. Push to empty array
    let res = tmpr_jit_array_push_i64(ctx_ptr, empty_handle.0, 42);
    let new_handle1 = res.to_result().expect("Should push successfully");
    
    assert_ne!(new_handle1, 0);
    assert_ne!(new_handle1, empty_handle.0);
    
    // Check lengths
    assert_eq!(tmpr_jit_array_len(ctx_ptr, empty_handle.0).to_result().unwrap(), 0, "Original array unchanged");
    assert_eq!(tmpr_jit_array_len(ctx_ptr, new_handle1).to_result().unwrap(), 1, "New array length is 1");
    
    // Check value
    assert_eq!(tmpr_jit_array_get_i64(ctx_ptr, new_handle1, 0).to_i64_result().unwrap(), 42);

    // 2. Push to non-empty array (negative value)
    let res = tmpr_jit_array_push_i64(ctx_ptr, non_empty_handle.0, -99);
    let new_handle2 = res.to_result().expect("Should push successfully");
    
    assert_ne!(new_handle2, 0);
    assert_ne!(new_handle2, non_empty_handle.0);
    
    // Check lengths
    assert_eq!(tmpr_jit_array_len(ctx_ptr, non_empty_handle.0).to_result().unwrap(), 2, "Original array unchanged");
    assert_eq!(tmpr_jit_array_len(ctx_ptr, new_handle2).to_result().unwrap(), 3, "New array length is 3");
    
    // Check value (last index)
    assert_eq!(tmpr_jit_array_get_i64(ctx_ptr, new_handle2, 2).to_i64_result().unwrap(), -99);
}

#[test]
fn jit_helper_array_push_i64_errors() {
    use crate::helpers::{JitRuntimeContext, tmpr_jit_array_push_i64};
    use crate::error::JitRuntimeError;

    let mut ctx = JitRuntimeContext::new();
    let string_handle = ctx.alloc_string("hello".to_string());
    let ctx_ptr = &mut ctx as *mut JitRuntimeContext;

    // 1. Type error (pushing to a string)
    let res = tmpr_jit_array_push_i64(ctx_ptr, string_handle.0, 42);
    assert_eq!(res.to_result(), Err(JitRuntimeError::TypeError));

    // 2. Invalid handle
    let res = tmpr_jit_array_push_i64(ctx_ptr, 9999, 42);
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidHandle));

    // 3. Null context
    let res = tmpr_jit_array_push_i64(std::ptr::null_mut(), string_handle.0, 42);
    assert_eq!(res.to_result(), Err(JitRuntimeError::InvalidContext));
}

// =====================================================================
// Phase 17.0: First Heap-aware JIT Eligibility Slice
// =====================================================================

#[test]
fn jit_heap_string_len_source_or_bytecode() {
    use tempra_bytecode::bytecode::BytecodeModule;
    use tempra_bytecode::function::{BytecodeFunction, BytecodeType, FunctionId};
    use tempra_bytecode::opcode::BytecodeInstr;
    use tempra_bytecode::function::LocalId;
    use crate::engine::{JitEngine, explain_jit_eligibility, JitEligibility};
    use crate::lower::compile_function_heap_aware;
    use crate::helpers::JitRuntimeContext;
    use std::collections::HashMap;

    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".to_string(), vec![BytecodeType::String], BytecodeType::Int64);
    func.locals.push(BytecodeType::String);
    
    // 0: LoadLocal(0) -> handle
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: StringLen
    func.code.push(BytecodeInstr::Len);
    // 2: Return
    func.code.push(BytecodeInstr::Return);
    
    module.functions.push(func);
    let func_id = FunctionId(0);

    // Check eligibility
    let eligibility = explain_jit_eligibility(&module, func_id);
    assert_eq!(eligibility, JitEligibility::Eligible);

    let mut engine = JitEngine::new();
    let mut jit_cache = HashMap::new();
    let mut compiling = std::collections::HashSet::new();

    let compiled = compile_function_heap_aware(&mut engine, &module, func_id, &mut jit_cache, &mut compiling).unwrap();

    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_string("hello".to_string());
    
    let res = compiled.call_with_ctx(&mut ctx, &[handle.0 as i64]);
    assert_eq!(res, Ok(5));
}

#[test]
fn jit_heap_array_get_i64_source_or_bytecode() {
    use tempra_bytecode::bytecode::BytecodeModule;
    use tempra_bytecode::function::{BytecodeFunction, BytecodeType, FunctionId};
    use tempra_bytecode::opcode::BytecodeInstr;
    use tempra_bytecode::function::LocalId;
    use crate::engine::{JitEngine, explain_jit_eligibility, JitEligibility};
    use crate::lower::compile_function_heap_aware;
    use crate::helpers::{JitRuntimeContext, JitHeapScalar};
    use std::collections::HashMap;

    let mut module = BytecodeModule::new();
    let mut func = BytecodeFunction::new("test".to_string(), vec![BytecodeType::Any, BytecodeType::Int64], BytecodeType::Int64);
    func.locals.push(BytecodeType::Any);
    func.locals.push(BytecodeType::Int64);
    
    // 0: LoadLocal(0) -> handle
    func.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    // 1: LoadLocal(1) -> index
    func.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    // 2: GetIndex
    func.code.push(BytecodeInstr::GetIndex);
    // 3: Return
    func.code.push(BytecodeInstr::Return);
    
    module.functions.push(func);
    let func_id = FunctionId(0);

    let eligibility = explain_jit_eligibility(&module, func_id);
    assert_eq!(eligibility, JitEligibility::Eligible);

    let mut engine = JitEngine::new();
    let mut jit_cache = HashMap::new();
    let mut compiling = std::collections::HashSet::new();

    let compiled = compile_function_heap_aware(&mut engine, &module, func_id, &mut jit_cache, &mut compiling).unwrap();

    let mut ctx = JitRuntimeContext::new();
    let handle = ctx.alloc_array(vec![
        JitHeapScalar::Int64(10),
        JitHeapScalar::Int64(20),
        JitHeapScalar::Int64(-5)
    ]);
    
    // OK cases
    assert_eq!(compiled.call_with_ctx(&mut ctx, &[handle.0 as i64, 0]), Ok(10));
    assert_eq!(compiled.call_with_ctx(&mut ctx, &[handle.0 as i64, 1]), Ok(20));
    assert_eq!(compiled.call_with_ctx(&mut ctx, &[handle.0 as i64, 2]), Ok(-5));
    
    // Error cases are mapped by the wrapper!
    let err_res = compiled.call_with_ctx(&mut ctx, &[handle.0 as i64, 99]);
    assert_eq!(err_res, Err(crate::error::JitRuntimeError::OutOfBounds));
}

#[test]
fn jit_heap_rejects_unsupported_heap_ops_source_level() {
    use tempra_bytecode::bytecode::BytecodeModule;
    use tempra_bytecode::function::{BytecodeFunction, BytecodeType, FunctionId};
    use tempra_bytecode::opcode::BytecodeInstr;
    use tempra_bytecode::function::LocalId;
    use crate::engine::{explain_jit_eligibility, JitEligibility};

    let mut module = BytecodeModule::new();
    
    // 1. Array Push (mutation/production)
    let mut func_push = BytecodeFunction::new("push".to_string(), vec![BytecodeType::Any, BytecodeType::Int64], BytecodeType::Any);
    func_push.locals.push(BytecodeType::Any);
    func_push.locals.push(BytecodeType::Int64);
    func_push.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func_push.code.push(BytecodeInstr::LoadLocal(LocalId(1)));
    func_push.code.push(BytecodeInstr::ArrayPush);
    func_push.code.push(BytecodeInstr::Return);
    module.functions.push(func_push);
    let id_push = FunctionId(0);
    
    match explain_jit_eligibility(&module, id_push) {
        JitEligibility::UnsupportedType(msg) => assert!(msg.contains("Returning heap values is not supported")),
        _ => panic!("Should reject returning array"),
    }

    // 2. Struct GetField (not supported in 17.0)
    let mut func_struct = BytecodeFunction::new("getfield".to_string(), vec![BytecodeType::Any], BytecodeType::Int64);
    func_struct.locals.push(BytecodeType::Any);
    func_struct.code.push(BytecodeInstr::LoadLocal(LocalId(0)));
    func_struct.code.push(BytecodeInstr::GetField(tempra_bytecode::constant::ConstId(0)));
    func_struct.code.push(BytecodeInstr::Return);
    module.functions.push(func_struct);
    let id_struct = FunctionId(1);

    match explain_jit_eligibility(&module, id_struct) {
        JitEligibility::UnsupportedInstr { instr, .. } => assert!(instr.contains("Struct heap operation requires Phase 17.2+")),
        _ => panic!("Should reject GetField"),
    }
}
