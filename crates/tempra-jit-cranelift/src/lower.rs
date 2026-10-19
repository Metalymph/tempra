use crate::engine::{JitEngine, JitFunctionI64};
use crate::error::JitCompileError;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use std::collections::HashMap;
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::constant::BytecodeConstant;
use tempra_bytecode::function::FunctionId;
use tempra_bytecode::opcode::BytecodeInstr;

#[derive(Clone, Copy)]
enum CheckedI64Op {
    Add,
    Sub,
    Mul,
}

fn return_status_if(
    builder: &mut FunctionBuilder<'_>,
    condition: Value,
    status: crate::abi::JitStatus,
) {
    let status = builder.ins().iconst(types::I64, status);
    return_status_value_if(builder, condition, status);
}

fn return_status_value_if(builder: &mut FunctionBuilder<'_>, condition: Value, status: Value) {
    let error_block = builder.create_block();
    let success_block = builder.create_block();
    builder
        .ins()
        .brif(condition, error_block, &[], success_block, &[]);

    builder.switch_to_block(error_block);
    builder.seal_block(error_block);
    let zero = builder.ins().iconst(types::I64, 0);
    builder.ins().return_(&[zero, status]);

    builder.switch_to_block(success_block);
    builder.seal_block(success_block);
}

fn emit_checked_i64_binary(
    builder: &mut FunctionBuilder<'_>,
    left: Value,
    right: Value,
    operation: CheckedI64Op,
) -> Value {
    let (value, overflowed) = match operation {
        CheckedI64Op::Add => builder.ins().sadd_overflow(left, right),
        CheckedI64Op::Sub => builder.ins().ssub_overflow(left, right),
        CheckedI64Op::Mul => builder.ins().smul_overflow(left, right),
    };
    return_status_if(
        builder,
        overflowed,
        crate::abi::JIT_STATUS_ERROR_INT64_OVERFLOW,
    );
    value
}

fn emit_checked_i64_div(builder: &mut FunctionBuilder<'_>, left: Value, right: Value) -> Value {
    let divisor_is_zero = builder.ins().icmp_imm(IntCC::Equal, right, 0);
    return_status_if(
        builder,
        divisor_is_zero,
        crate::abi::JIT_STATUS_ERROR_INT64_DIVISION_BY_ZERO,
    );

    let minimum = builder.ins().iconst(types::I64, i64::MIN);
    let negative_one = builder.ins().iconst(types::I64, -1);
    let dividend_is_minimum = builder.ins().icmp(IntCC::Equal, left, minimum);
    let divisor_is_negative_one = builder.ins().icmp(IntCC::Equal, right, negative_one);
    let division_overflow = builder
        .ins()
        .band(dividend_is_minimum, divisor_is_negative_one);
    return_status_if(
        builder,
        division_overflow,
        crate::abi::JIT_STATUS_ERROR_INT64_OVERFLOW,
    );

    builder.ins().sdiv(left, right)
}

pub fn is_instr_supported(instr: &BytecodeInstr) -> bool {
    match instr {
        BytecodeInstr::ConstI64(_) => true,
        BytecodeInstr::LoadLocal(_) => true,
        BytecodeInstr::StoreLocal(_) => true,
        BytecodeInstr::AddI64
        | BytecodeInstr::SubI64
        | BytecodeInstr::MulI64
        | BytecodeInstr::DivI64 => true,
        BytecodeInstr::Eq | BytecodeInstr::LtI64 | BytecodeInstr::GtI64 => true,
        BytecodeInstr::Jump(_) | BytecodeInstr::JumpIfFalse(_) => true,
        BytecodeInstr::Return => true,
        BytecodeInstr::AddI64LL(_, _) | BytecodeInstr::AddI64LC(_, _) => true,
        BytecodeInstr::ConstBool(_)
        | BytecodeInstr::Not
        | BytecodeInstr::And
        | BytecodeInstr::Or => true,
        BytecodeInstr::Len | BytecodeInstr::Size | BytecodeInstr::GetIndex => true,
        _ => false,
    }
}

/// Returns true when a bytecode function requires the heap-aware JIT path.
///
/// A function is heap-requiring when any of its params or locals are of
/// `BytecodeType::String` or `BytecodeType::Any` (which represents heap
/// handles at the bytecode level).
pub fn is_heap_requiring(func: &tempra_bytecode::function::BytecodeFunction) -> bool {
    use tempra_bytecode::function::BytecodeType;
    func.params
        .iter()
        .any(|p| *p == BytecodeType::String || *p == BytecodeType::Any)
        || func
            .locals
            .iter()
            .any(|l| *l == BytecodeType::String || *l == BytecodeType::Any)
}

pub fn compile_function_i64(
    engine: &mut JitEngine,
    module: &BytecodeModule,
    function_id: FunctionId,
    jit_cache: &mut HashMap<FunctionId, JitFunctionI64>,
    compiling: &mut std::collections::HashSet<FunctionId>,
) -> Result<JitFunctionI64, JitCompileError> {
    if compiling.contains(&function_id) {
        return Err(JitCompileError::CraneliftError(
            "Call target not JIT eligible (recursion)".into(),
        ));
    }
    compiling.insert(function_id);

    let func = module
        .functions
        .get(function_id.0 as usize)
        .ok_or_else(|| JitCompileError::CraneliftError("Function not found".into()))?;

    // Pre-compile dependencies
    let mut dependencies = Vec::new();
    for instr in &func.code {
        if let BytecodeInstr::Call(target_id, _) = instr {
            if !jit_cache.contains_key(target_id) && !dependencies.contains(target_id) {
                dependencies.push(*target_id);
            }
        }
    }

    for dep in dependencies {
        let compiled = compile_function_i64(engine, module, dep, jit_cache, compiling)?;
        jit_cache.insert(dep, compiled);
    }
    compiling.remove(&function_id);

    engine.ctx.clear();
    engine.ctx.func.signature = engine.module.make_signature();
    engine
        .ctx
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));
    engine
        .ctx
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));
    for _ in 0..func.params.len() {
        engine
            .ctx
            .func
            .signature
            .params
            .push(AbiParam::new(types::I64));
    }

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);

    // 1. Identify jump targets
    let mut jump_targets = HashMap::new();
    for instr in &func.code {
        match instr {
            BytecodeInstr::Jump(off) => {
                jump_targets.insert(off.0, None);
            }
            BytecodeInstr::JumpIfFalse(off) => {
                jump_targets.insert(off.0, None);
            }
            _ => {}
        }
    }

    // Create blocks for jump targets
    for (_, block) in jump_targets.iter_mut() {
        *block = Some(builder.create_block());
    }

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // 2. Map locals to variables
    let mut variables = Vec::new();
    for i in 0..func.locals.len() {
        let var = Variable::new(i);
        builder.declare_var(var, types::I64);
        variables.push(var);

        // If it's a param, initialize it
        if i < func.params.len() {
            let val = builder.block_params(entry_block)[i];
            builder.def_var(var, val);
        }
    }

    let mut stack: Vec<Value> = Vec::new();
    let mut current_block_terminated = false;

    for (ip, instr) in func.code.iter().enumerate() {
        // If this IP is a jump target, switch to the block
        if let Some(Some(target_block)) = jump_targets.get(&ip) {
            // Before switching, if the previous block wasn't terminated, jump to this one
            if !current_block_terminated {
                builder.ins().jump(*target_block, &[]);
            }
            builder.switch_to_block(*target_block);
            current_block_terminated = false;
            // We don't seal it here because we might have backward jumps
        }

        if current_block_terminated {
            continue;
        }

        match instr {
            BytecodeInstr::ConstI64(id) => {
                let val = match module.constants.get(id.0 as usize) {
                    Some(BytecodeConstant::Int64(i)) => *i,
                    _ => return Err(JitCompileError::CraneliftError("Invalid constant".into())),
                };
                stack.push(builder.ins().iconst(types::I64, val));
            }
            BytecodeInstr::LoadLocal(id) => {
                let var = variables[id.0 as usize];
                stack.push(builder.use_var(var));
            }
            BytecodeInstr::StoreLocal(id) => {
                let val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let var = variables[id.0 as usize];
                builder.def_var(var, val);
            }
            BytecodeInstr::AddI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::SubI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Sub,
                ));
            }
            BytecodeInstr::MulI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Mul,
                ));
            }
            BytecodeInstr::DivI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_div(&mut builder, a, b));
            }
            BytecodeInstr::Eq => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::Equal, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::LtI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::GtI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::ConstBool(b) => {
                let val = if *b { 1 } else { 0 };
                stack.push(builder.ins().iconst(types::I64, val));
            }
            BytecodeInstr::Not => {
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                // Boolean in Tempra is 0 or 1. !a is 1 - a.
                // Or use icmp_imm(Equal, a, 0)
                let cmp = builder.ins().icmp_imm(IntCC::Equal, a, 0);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::And => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().band(a, b));
            }
            BytecodeInstr::Or => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().bor(a, b));
            }
            BytecodeInstr::Jump(off) => {
                let target = jump_targets.get(&off.0).unwrap().unwrap();
                builder.ins().jump(target, &[]);
                current_block_terminated = true;
            }
            BytecodeInstr::JumpIfFalse(off) => {
                let cond_val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                // In Tempra VM, bool is i64(1) for true, i64(0) for false
                let cond = builder.ins().icmp_imm(IntCC::NotEqual, cond_val, 0);
                let target = jump_targets.get(&off.0).unwrap().unwrap();

                let next_block = builder.create_block();
                builder.ins().brif(cond, next_block, &[], target, &[]);
                builder.switch_to_block(next_block);
                builder.seal_block(next_block);
                // Still not terminated because we switched to next_block
            }
            BytecodeInstr::Return => {
                let val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let status_ok = builder
                    .ins()
                    .iconst(types::I64, crate::abi::JIT_STATUS_SUCCESS);
                builder.ins().return_(&[val, status_ok]);
                current_block_terminated = true;
            }
            BytecodeInstr::AddI64LL(a_id, b_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let b = builder.use_var(variables[b_id.0 as usize]);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::AddI64LC(a_id, c_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let c_val = match module.constants.get(c_id.0 as usize) {
                    Some(BytecodeConstant::Int64(i)) => *i,
                    _ => return Err(JitCompileError::CraneliftError("Invalid constant".into())),
                };
                let b = builder.ins().iconst(types::I64, c_val);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::Call(target_id, argc) => {
                let mut args = Vec::new();
                for _ in 0..*argc {
                    args.push(stack.pop().ok_or_else(|| {
                        JitCompileError::CraneliftError("Stack underflow".into())
                    })?);
                }
                args.reverse();

                let mut sig = engine.module.make_signature();
                sig.returns.push(AbiParam::new(types::I64));
                sig.returns.push(AbiParam::new(types::I64));
                for _ in 0..*argc {
                    sig.params.push(AbiParam::new(types::I64));
                }

                let target_name = format!("jit_fn_{}", target_id.0);
                let callee_id = engine
                    .module
                    .declare_function(&target_name, Linkage::Export, &sig)
                    .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

                let local_callee = engine
                    .module
                    .declare_func_in_func(callee_id, &mut builder.func);
                let call_inst = builder.ins().call(local_callee, &args);
                let results = builder.inst_results(call_inst).to_vec();
                let value = results[0];
                let status = results[1];
                let failed = builder.ins().icmp_imm(IntCC::NotEqual, status, 0);
                return_status_value_if(&mut builder, failed, status);
                stack.push(value);
            }
            _ => return Err(JitCompileError::UnsupportedInstr(*instr)),
        }
    }

    // Seal all jump target blocks
    for (_, block) in jump_targets {
        builder.seal_block(block.unwrap());
    }

    builder.finalize();

    // 3. Compile
    let func_name = format!("jit_fn_{}", function_id.0);
    let id = engine
        .module
        .declare_function(&func_name, Linkage::Export, &engine.ctx.func.signature)
        .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

    let _result = engine
        .module
        .define_function(id, &mut engine.ctx)
        .map_err(|e| JitCompileError::CraneliftError(format!("Compilation error: {}", e)))?;

    engine.module.clear_context(&mut engine.ctx);
    engine
        .module
        .finalize_definitions()
        .map_err(|e| JitCompileError::CraneliftError(format!("Finalization error: {}", e)))?;

    let code_ptr = engine.module.get_finalized_function(id);
    Ok(JitFunctionI64::new(code_ptr, func.params.len()))
}

pub fn compile_function_heap_aware(
    engine: &mut JitEngine,
    module: &BytecodeModule,
    function_id: FunctionId,
    jit_cache: &mut HashMap<FunctionId, crate::engine::JitFunctionI64HeapAware>,
    compiling: &mut std::collections::HashSet<FunctionId>,
) -> Result<crate::engine::JitFunctionI64HeapAware, JitCompileError> {
    if compiling.contains(&function_id) {
        return Err(JitCompileError::CraneliftError(
            "Call target not JIT eligible (recursion)".into(),
        ));
    }
    compiling.insert(function_id);

    let func = module
        .functions
        .get(function_id.0 as usize)
        .ok_or_else(|| JitCompileError::CraneliftError("Function not found".into()))?;

    // Pre-compile dependencies
    let mut dependencies = Vec::new();
    for instr in &func.code {
        if let BytecodeInstr::Call(target_id, _) = instr {
            if !jit_cache.contains_key(target_id) && !dependencies.contains(target_id) {
                dependencies.push(*target_id);
            }
        }
    }

    for dep in dependencies {
        let compiled = compile_function_heap_aware(engine, module, dep, jit_cache, compiling)?;
        jit_cache.insert(dep, compiled);
    }
    compiling.remove(&function_id);

    engine.ctx.clear();
    engine.ctx.func.signature = engine.module.make_signature();

    // Returns: (value: i64, status: i64)
    engine
        .ctx
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));
    engine
        .ctx
        .func
        .signature
        .returns
        .push(AbiParam::new(types::I64));

    // Params: ctx_ptr: i64, args...
    engine
        .ctx
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64));
    for _ in 0..func.params.len() {
        engine
            .ctx
            .func
            .signature
            .params
            .push(AbiParam::new(types::I64));
    }

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);

    // 1. Identify jump targets
    let mut jump_targets = HashMap::new();
    for instr in &func.code {
        match instr {
            BytecodeInstr::Jump(off) => {
                jump_targets.insert(off.0, None);
            }
            BytecodeInstr::JumpIfFalse(off) => {
                jump_targets.insert(off.0, None);
            }
            _ => {}
        }
    }

    // Create blocks for jump targets
    for (_, block) in jump_targets.iter_mut() {
        *block = Some(builder.create_block());
    }

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // Context is param 0
    let ctx_val = builder.block_params(entry_block)[0];

    // 2. Map locals to variables
    let mut variables = Vec::new();
    for i in 0..func.locals.len() {
        let var = Variable::new(i);
        builder.declare_var(var, types::I64);
        variables.push(var);

        // If it's a param, initialize it (offset by 1 for ctx)
        if i < func.params.len() {
            let val = builder.block_params(entry_block)[i + 1];
            builder.def_var(var, val);
        }
    }

    let mut stack: Vec<Value> = Vec::new();
    let mut current_block_terminated = false;

    let emit_helper_call = |builder: &mut FunctionBuilder,
                            helper_ptr: *const (),
                            args: &[Value],
                            stack: &mut Vec<Value>|
     -> Result<bool, JitCompileError> {
        let mut sig = cranelift::codegen::ir::Signature::new(builder.func.signature.call_conv);
        for _ in 0..args.len() {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64)); // value
        sig.returns.push(AbiParam::new(types::I64)); // status
        let helper_sig_ref = builder.import_signature(sig);

        let helper_addr = builder.ins().iconst(types::I64, helper_ptr as i64);
        let call = builder
            .ins()
            .call_indirect(helper_sig_ref, helper_addr, args);
        let results = builder.inst_results(call).to_vec();

        let val = results[0];
        let status = results[1];

        let success_block = builder.create_block();
        let error_block = builder.create_block();

        let is_ok = builder.ins().icmp_imm(IntCC::Equal, status, 0); // JIT_STATUS_SUCCESS
        builder
            .ins()
            .brif(is_ok, success_block, &[], error_block, &[]);

        // Error path: return (0, status) immediately
        builder.switch_to_block(error_block);
        builder.seal_block(error_block);
        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().return_(&[zero, status]);

        // Success path: continue with value
        builder.switch_to_block(success_block);
        builder.seal_block(success_block);
        stack.push(val);
        Ok(false) // current block not terminated
    };

    for (ip, instr) in func.code.iter().enumerate() {
        if let Some(Some(target_block)) = jump_targets.get(&ip) {
            if !current_block_terminated {
                builder.ins().jump(*target_block, &[]);
            }
            builder.switch_to_block(*target_block);
            current_block_terminated = false;
        }

        if current_block_terminated {
            continue;
        }

        match instr {
            BytecodeInstr::ConstI64(id) => {
                let val = match module.constants.get(id.0 as usize) {
                    Some(BytecodeConstant::Int64(i)) => *i,
                    _ => return Err(JitCompileError::CraneliftError("Invalid constant".into())),
                };
                stack.push(builder.ins().iconst(types::I64, val));
            }
            BytecodeInstr::LoadLocal(id) => {
                let var = variables[id.0 as usize];
                stack.push(builder.use_var(var));
            }
            BytecodeInstr::StoreLocal(id) => {
                let val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let var = variables[id.0 as usize];
                builder.def_var(var, val);
            }
            BytecodeInstr::AddI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::SubI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Sub,
                ));
            }
            BytecodeInstr::MulI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Mul,
                ));
            }
            BytecodeInstr::DivI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(emit_checked_i64_div(&mut builder, a, b));
            }
            BytecodeInstr::Eq => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::Equal, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::LtI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::GtI64 => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::ConstBool(b) => {
                let val = if *b { 1 } else { 0 };
                stack.push(builder.ins().iconst(types::I64, val));
            }
            BytecodeInstr::Not => {
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp_imm(IntCC::Equal, a, 0);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::And => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().band(a, b));
            }
            BytecodeInstr::Or => {
                let b = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().bor(a, b));
            }
            BytecodeInstr::Jump(off) => {
                let target = jump_targets.get(&off.0).unwrap().unwrap();
                builder.ins().jump(target, &[]);
                current_block_terminated = true;
            }
            BytecodeInstr::JumpIfFalse(off) => {
                let cond_val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cond = builder.ins().icmp_imm(IntCC::NotEqual, cond_val, 0);
                let target = jump_targets.get(&off.0).unwrap().unwrap();

                let next_block = builder.create_block();
                builder.ins().brif(cond, next_block, &[], target, &[]);
                builder.switch_to_block(next_block);
                builder.seal_block(next_block);
            }
            BytecodeInstr::Return => {
                let val = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let status_ok = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[val, status_ok]);
                current_block_terminated = true;
            }
            BytecodeInstr::Len => {
                let handle = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                current_block_terminated = emit_helper_call(
                    &mut builder,
                    crate::helpers::tmpr_jit_generic_len as *const (),
                    &[ctx_val, handle],
                    &mut stack,
                )?;
            }
            BytecodeInstr::Size => {
                let handle = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                current_block_terminated = emit_helper_call(
                    &mut builder,
                    crate::helpers::tmpr_jit_generic_size as *const (),
                    &[ctx_val, handle],
                    &mut stack,
                )?;
            }
            BytecodeInstr::GetIndex => {
                let index = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let handle = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                current_block_terminated = emit_helper_call(
                    &mut builder,
                    crate::helpers::tmpr_jit_generic_get_i64 as *const (),
                    &[ctx_val, handle, index],
                    &mut stack,
                )?;
            }
            BytecodeInstr::AddI64LL(a_id, b_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let b = builder.use_var(variables[b_id.0 as usize]);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::AddI64LC(a_id, c_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let c_val = match module.constants.get(c_id.0 as usize) {
                    Some(BytecodeConstant::Int64(i)) => *i,
                    _ => return Err(JitCompileError::CraneliftError("Invalid constant".into())),
                };
                let b = builder.ins().iconst(types::I64, c_val);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    a,
                    b,
                    CheckedI64Op::Add,
                ));
            }
            BytecodeInstr::Call(target_id, argc) => {
                let mut args = Vec::new();
                args.push(ctx_val);

                let mut call_args = Vec::new();
                for _ in 0..*argc {
                    call_args.push(stack.pop().ok_or_else(|| {
                        JitCompileError::CraneliftError("Stack underflow".into())
                    })?);
                }
                call_args.reverse();
                args.extend(call_args);

                let mut sig = engine.module.make_signature();
                sig.returns.push(AbiParam::new(types::I64));
                sig.returns.push(AbiParam::new(types::I64)); // status
                for _ in 0..(*argc + 1) {
                    // +1 for ctx
                    sig.params.push(AbiParam::new(types::I64));
                }

                let target_name = format!("jit_fn_{}", target_id.0);
                let callee_id = engine
                    .module
                    .declare_function(&target_name, Linkage::Export, &sig)
                    .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

                let local_callee = engine
                    .module
                    .declare_func_in_func(callee_id, &mut builder.func);
                let call_inst = builder.ins().call(local_callee, &args);
                let results = builder.inst_results(call_inst).to_vec();

                let val = results[0];
                let status = results[1];

                let success_block = builder.create_block();
                let error_block = builder.create_block();

                let is_ok = builder.ins().icmp_imm(IntCC::Equal, status, 0);
                builder
                    .ins()
                    .brif(is_ok, success_block, &[], error_block, &[]);

                builder.switch_to_block(error_block);
                builder.seal_block(error_block);
                let zero = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[zero, status]);

                builder.switch_to_block(success_block);
                builder.seal_block(success_block);
                stack.push(val);
            }
            _ => return Err(JitCompileError::UnsupportedInstr(*instr)),
        }
    }

    for (_, block) in jump_targets {
        builder.seal_block(block.unwrap());
    }

    builder.finalize();

    let func_name = format!("jit_fn_{}", function_id.0);
    let id = engine
        .module
        .declare_function(&func_name, Linkage::Export, &engine.ctx.func.signature)
        .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

    let _result = engine
        .module
        .define_function(id, &mut engine.ctx)
        .map_err(|e| JitCompileError::CraneliftError(format!("Compilation error: {}", e)))?;

    engine.module.clear_context(&mut engine.ctx);
    engine
        .module
        .finalize_definitions()
        .map_err(|e| JitCompileError::CraneliftError(format!("Finalization error: {}", e)))?;

    let code_ptr = engine.module.get_finalized_function(id);
    Ok(crate::engine::JitFunctionI64HeapAware::new(
        code_ptr,
        func.params.len(),
    ))
}
