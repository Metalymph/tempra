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

/// Writes a `(value, status)` pair through the caller-provided out pointer.
///
/// The 16-byte [`crate::abi::JitHelperResult`] is always returned via an
/// explicit out pointer rather than as an aggregate return value: target ABIs
/// disagree on how to return such aggregates (registers on SysV, a hidden
/// pointer that displaces the first parameter on Windows x64), while an out
/// pointer is unambiguous on every target. See [`crate::abi::JitHelperResult`].
fn store_result(builder: &mut FunctionBuilder<'_>, out_ptr: Value, value: Value, status: Value) {
    let memflags = MemFlags::new();
    builder.ins().store(
        memflags,
        value,
        out_ptr,
        crate::abi::JIT_RESULT_VALUE_OFFSET,
    );
    builder.ins().store(
        memflags,
        status,
        out_ptr,
        crate::abi::JIT_RESULT_STATUS_OFFSET,
    );
}

/// Allocates stack storage for a [`crate::abi::JitHelperResult`] and returns
/// the address of that storage, to be passed as a callee out pointer.
fn create_result_slot(builder: &mut FunctionBuilder<'_>) -> Value {
    let slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        crate::abi::JIT_RESULT_SIZE,
    ));
    builder.ins().stack_addr(types::I64, slot, 0)
}

/// Reads the `(value, status)` pair previously written to `out_ptr`.
fn load_result(builder: &mut FunctionBuilder<'_>, out_ptr: Value) -> (Value, Value) {
    let memflags = MemFlags::new();
    let value = builder.ins().load(
        types::I64,
        memflags,
        out_ptr,
        crate::abi::JIT_RESULT_VALUE_OFFSET,
    );
    let status = builder.ins().load(
        types::I64,
        memflags,
        out_ptr,
        crate::abi::JIT_RESULT_STATUS_OFFSET,
    );
    (value, status)
}

fn return_status_if(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    condition: Value,
    status: crate::abi::JitStatus,
) {
    let status = builder.ins().iconst(types::I64, status);
    return_status_value_if(builder, out_ptr, condition, status);
}

fn return_status_value_if(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    condition: Value,
    status: Value,
) {
    let error_block = builder.create_block();
    let success_block = builder.create_block();
    builder
        .ins()
        .brif(condition, error_block, &[], success_block, &[]);

    builder.switch_to_block(error_block);
    builder.seal_block(error_block);
    let zero = builder.ins().iconst(types::I64, 0);
    store_result(builder, out_ptr, zero, status);
    builder.ins().return_(&[]);

    builder.switch_to_block(success_block);
    builder.seal_block(success_block);
}

fn emit_checked_i64_binary(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
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
        out_ptr,
        overflowed,
        crate::abi::JIT_STATUS_ERROR_INT64_OVERFLOW,
    );
    value
}

fn emit_checked_i64_div(
    builder: &mut FunctionBuilder<'_>,
    out_ptr: Value,
    left: Value,
    right: Value,
) -> Value {
    let divisor_is_zero = builder.ins().icmp_imm(IntCC::Equal, right, 0);
    return_status_if(
        builder,
        out_ptr,
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
        out_ptr,
        division_overflow,
        crate::abi::JIT_STATUS_ERROR_INT64_OVERFLOW,
    );

    builder.ins().sdiv(left, right)
}

pub fn is_instr_supported(instr: &BytecodeInstr) -> bool {
    matches!(
        instr,
        BytecodeInstr::ConstI64(_)
            | BytecodeInstr::LoadLocal(_)
            | BytecodeInstr::StoreLocal(_)
            | BytecodeInstr::AddI64
            | BytecodeInstr::SubI64
            | BytecodeInstr::MulI64
            | BytecodeInstr::DivI64
            | BytecodeInstr::Eq
            | BytecodeInstr::LtI64
            | BytecodeInstr::GtI64
            | BytecodeInstr::Jump(_)
            | BytecodeInstr::JumpIfFalse(_)
            | BytecodeInstr::Return
            | BytecodeInstr::AddI64LL(_, _)
            | BytecodeInstr::AddI64LC(_, _)
            | BytecodeInstr::ConstBool(_)
            | BytecodeInstr::Not
            | BytecodeInstr::And
            | BytecodeInstr::Or
            | BytecodeInstr::Len
            | BytecodeInstr::Size
            | BytecodeInstr::GetIndex
    )
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
    // Boundary convention: the first parameter is the caller-provided out
    // pointer for the (value, status) result and the function returns unit.
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
    for block in jump_targets.values_mut() {
        *block = Some(builder.create_block());
    }

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // Out pointer is param 0; user parameters follow it.
    let out_ptr = builder.block_params(entry_block)[0];

    // 2. Map locals to variables
    let mut variables = Vec::new();
    for i in 0..func.locals.len() {
        let var = Variable::new(i);
        builder.declare_var(var, types::I64);
        variables.push(var);

        // If it's a param, initialize it (offset by 1 for the out pointer)
        if i < func.params.len() {
            let val = builder.block_params(entry_block)[i + 1];
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
                    out_ptr,
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
                    out_ptr,
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
                    out_ptr,
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
                stack.push(emit_checked_i64_div(&mut builder, out_ptr, a, b));
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
                store_result(&mut builder, out_ptr, val, status_ok);
                builder.ins().return_(&[]);
                current_block_terminated = true;
            }
            BytecodeInstr::AddI64LL(a_id, b_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let b = builder.use_var(variables[b_id.0 as usize]);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    out_ptr,
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
                    out_ptr,
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

                let out_addr = create_result_slot(&mut builder);

                let mut sig = engine.module.make_signature();
                sig.params.push(AbiParam::new(types::I64)); // out
                for _ in 0..*argc {
                    sig.params.push(AbiParam::new(types::I64));
                }

                let target_name = format!("jit_fn_{}", target_id.0);
                let callee_id = engine
                    .module
                    .declare_function(&target_name, Linkage::Export, &sig)
                    .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

                let local_callee = engine.module.declare_func_in_func(callee_id, builder.func);
                let mut call_args = Vec::with_capacity(args.len() + 1);
                call_args.push(out_addr);
                call_args.extend(args);
                builder.ins().call(local_callee, &call_args);
                let (value, status) = load_result(&mut builder, out_addr);
                let failed = builder.ins().icmp_imm(IntCC::NotEqual, status, 0);
                return_status_value_if(&mut builder, out_ptr, failed, status);
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

    engine
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

    // Boundary convention: out pointer, then context pointer, then args.
    // The function returns unit; the (value, status) result is written through
    // the out pointer.
    engine
        .ctx
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64)); // out: *mut JitHelperResult
    engine
        .ctx
        .func
        .signature
        .params
        .push(AbiParam::new(types::I64)); // ctx: *mut JitRuntimeContext
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
    for block in jump_targets.values_mut() {
        *block = Some(builder.create_block());
    }

    let entry_block = builder.create_block();
    builder.append_block_params_for_function_params(entry_block);
    builder.switch_to_block(entry_block);
    builder.seal_block(entry_block);

    // Out pointer is param 0, context is param 1.
    let out_ptr = builder.block_params(entry_block)[0];
    let ctx_val = builder.block_params(entry_block)[1];

    // 2. Map locals to variables
    let mut variables = Vec::new();
    for i in 0..func.locals.len() {
        let var = Variable::new(i);
        builder.declare_var(var, types::I64);
        variables.push(var);

        // If it's a param, initialize it (offset by 2 for out pointer + ctx)
        if i < func.params.len() {
            let val = builder.block_params(entry_block)[i + 2];
            builder.def_var(var, val);
        }
    }

    let mut stack: Vec<Value> = Vec::new();
    let mut current_block_terminated = false;

    let emit_helper_call = |builder: &mut FunctionBuilder,
                            out_ptr: Value,
                            helper_ptr: *const (),
                            args: &[Value],
                            stack: &mut Vec<Value>|
     -> Result<bool, JitCompileError> {
        // The helper writes its (value, status) result through a stack slot we
        // own; nothing is returned in registers.
        let out_addr = create_result_slot(builder);

        let mut sig = cranelift::codegen::ir::Signature::new(builder.func.signature.call_conv);
        sig.params.push(AbiParam::new(types::I64)); // out
        for _ in 0..args.len() {
            sig.params.push(AbiParam::new(types::I64));
        }
        let helper_sig_ref = builder.import_signature(sig);

        let helper_addr = builder.ins().iconst(types::I64, helper_ptr as i64);
        let mut call_args = Vec::with_capacity(args.len() + 1);
        call_args.push(out_addr);
        call_args.extend_from_slice(args);
        builder
            .ins()
            .call_indirect(helper_sig_ref, helper_addr, &call_args);

        let (val, status) = load_result(builder, out_addr);

        let success_block = builder.create_block();
        let error_block = builder.create_block();

        let is_ok = builder.ins().icmp_imm(IntCC::Equal, status, 0); // JIT_STATUS_SUCCESS
        builder
            .ins()
            .brif(is_ok, success_block, &[], error_block, &[]);

        // Error path: write (0, status) and return immediately
        builder.switch_to_block(error_block);
        builder.seal_block(error_block);
        let zero = builder.ins().iconst(types::I64, 0);
        store_result(builder, out_ptr, zero, status);
        builder.ins().return_(&[]);

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
                    out_ptr,
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
                    out_ptr,
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
                    out_ptr,
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
                stack.push(emit_checked_i64_div(&mut builder, out_ptr, a, b));
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
                store_result(&mut builder, out_ptr, val, status_ok);
                builder.ins().return_(&[]);
                current_block_terminated = true;
            }
            BytecodeInstr::Len => {
                let handle = stack
                    .pop()
                    .ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                current_block_terminated = emit_helper_call(
                    &mut builder,
                    out_ptr,
                    crate::helpers::tmpr_jit_generic_len_out as *const (),
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
                    out_ptr,
                    crate::helpers::tmpr_jit_generic_size_out as *const (),
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
                    out_ptr,
                    crate::helpers::tmpr_jit_generic_get_i64_out as *const (),
                    &[ctx_val, handle, index],
                    &mut stack,
                )?;
            }
            BytecodeInstr::AddI64LL(a_id, b_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let b = builder.use_var(variables[b_id.0 as usize]);
                stack.push(emit_checked_i64_binary(
                    &mut builder,
                    out_ptr,
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
                    out_ptr,
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

                let out_addr = create_result_slot(&mut builder);

                let mut sig = engine.module.make_signature();
                sig.params.push(AbiParam::new(types::I64)); // out
                for _ in 0..(*argc + 1) {
                    // +1 for ctx
                    sig.params.push(AbiParam::new(types::I64));
                }

                let target_name = format!("jit_fn_{}", target_id.0);
                let callee_id = engine
                    .module
                    .declare_function(&target_name, Linkage::Export, &sig)
                    .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

                let local_callee = engine.module.declare_func_in_func(callee_id, builder.func);
                let mut full_args = Vec::with_capacity(args.len() + 1);
                full_args.push(out_addr);
                full_args.extend(args);
                builder.ins().call(local_callee, &full_args);

                let (val, status) = load_result(&mut builder, out_addr);

                let success_block = builder.create_block();
                let error_block = builder.create_block();

                let is_ok = builder.ins().icmp_imm(IntCC::Equal, status, 0);
                builder
                    .ins()
                    .brif(is_ok, success_block, &[], error_block, &[]);

                builder.switch_to_block(error_block);
                builder.seal_block(error_block);
                let zero = builder.ins().iconst(types::I64, 0);
                store_result(&mut builder, out_ptr, zero, status);
                builder.ins().return_(&[]);

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

    engine
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
