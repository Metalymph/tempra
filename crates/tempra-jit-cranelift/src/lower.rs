use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::FunctionId;
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::constant::BytecodeConstant;
use crate::engine::{JitEngine, JitFunctionI64};
use crate::error::JitCompileError;
use std::collections::HashMap;

pub fn is_instr_supported(instr: &BytecodeInstr) -> bool {
    match instr {
        BytecodeInstr::ConstI64(_) => true,
        BytecodeInstr::LoadLocal(_) => true,
        BytecodeInstr::StoreLocal(_) => true,
        BytecodeInstr::AddI64 | BytecodeInstr::SubI64 | BytecodeInstr::MulI64 | BytecodeInstr::DivI64 => true,
        BytecodeInstr::Eq | BytecodeInstr::LtI64 | BytecodeInstr::GtI64 => true,
        BytecodeInstr::Jump(_) | BytecodeInstr::JumpIfFalse(_) => true,
        BytecodeInstr::Return => true,
        BytecodeInstr::AddI64LL(_, _) | BytecodeInstr::AddI64LC(_, _) => true,
        _ => false,
    }
}

pub fn compile_function_i64(
    engine: &mut JitEngine,
    module: &BytecodeModule,
    function_id: FunctionId,
    jit_cache: &mut HashMap<FunctionId, JitFunctionI64>,
    compiling: &mut std::collections::HashSet<FunctionId>,
) -> Result<JitFunctionI64, JitCompileError> {
    if compiling.contains(&function_id) {
        return Err(JitCompileError::CraneliftError("Call target not JIT eligible (recursion)".into()));
    }
    compiling.insert(function_id);

    let func = module.functions.get(function_id.0 as usize)
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
    engine.ctx.func.signature.returns.push(AbiParam::new(types::I64));
    for _ in 0..func.params.len() {
        engine.ctx.func.signature.params.push(AbiParam::new(types::I64));
    }

    let mut builder = FunctionBuilder::new(&mut engine.ctx.func, &mut engine.builder_context);
    

    // 1. Identify jump targets
    let mut jump_targets = HashMap::new();
    for instr in &func.code {
        match instr {
            BytecodeInstr::Jump(off) => { jump_targets.insert(off.0, None); }
            BytecodeInstr::JumpIfFalse(off) => { jump_targets.insert(off.0, None); }
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
                let val = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let var = variables[id.0 as usize];
                builder.def_var(var, val);
            }
            BytecodeInstr::AddI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().iadd(a, b));
            }
            BytecodeInstr::SubI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().isub(a, b));
            }
            BytecodeInstr::MulI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                stack.push(builder.ins().imul(a, b));
            }
            BytecodeInstr::DivI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                // TODO: Check for zero? VM handles it.
                stack.push(builder.ins().sdiv(a, b));
            }
            BytecodeInstr::Eq => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::Equal, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::LtI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::GtI64 => {
                let b = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let a = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                let cmp = builder.ins().icmp(IntCC::SignedGreaterThan, a, b);
                stack.push(builder.ins().uextend(types::I64, cmp));
            }
            BytecodeInstr::Jump(off) => {
                let target = jump_targets.get(&off.0).unwrap().unwrap();
                builder.ins().jump(target, &[]);
                current_block_terminated = true;
            }
            BytecodeInstr::JumpIfFalse(off) => {
                let cond_val = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
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
                let val = stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?;
                builder.ins().return_(&[val]);
                current_block_terminated = true;
            }
            BytecodeInstr::AddI64LL(a_id, b_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let b = builder.use_var(variables[b_id.0 as usize]);
                stack.push(builder.ins().iadd(a, b));
            }
            BytecodeInstr::AddI64LC(a_id, c_id) => {
                let a = builder.use_var(variables[a_id.0 as usize]);
                let c_val = match module.constants.get(c_id.0 as usize) {
                    Some(BytecodeConstant::Int64(i)) => *i,
                    _ => return Err(JitCompileError::CraneliftError("Invalid constant".into())),
                };
                let b = builder.ins().iconst(types::I64, c_val);
                stack.push(builder.ins().iadd(a, b));
            }
            BytecodeInstr::Call(target_id, argc) => {
                let mut args = Vec::new();
                for _ in 0..*argc {
                    args.push(stack.pop().ok_or_else(|| JitCompileError::CraneliftError("Stack underflow".into()))?);
                }
                args.reverse();

                let mut sig = engine.module.make_signature();
                sig.returns.push(AbiParam::new(types::I64));
                for _ in 0..*argc {
                    sig.params.push(AbiParam::new(types::I64));
                }

                let target_name = format!("jit_fn_{}", target_id.0);
                let callee_id = engine.module.declare_function(&target_name, Linkage::Export, &sig)
                    .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;
                
                let local_callee = engine.module.declare_func_in_func(callee_id, &mut builder.func);
                let call_inst = builder.ins().call(local_callee, &args);
                let res = builder.inst_results(call_inst)[0];
                stack.push(res);
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
    let id = engine.module.declare_function(&func_name, Linkage::Export, &engine.ctx.func.signature)
        .map_err(|e| JitCompileError::CraneliftError(e.to_string()))?;

    let _result = engine.module.define_function(id, &mut engine.ctx)
        .map_err(|e| JitCompileError::CraneliftError(format!("Compilation error: {}", e)))?;

    engine.module.clear_context(&mut engine.ctx);
    engine.module.finalize_definitions().map_err(|e| JitCompileError::CraneliftError(format!("Finalization error: {}", e)))?;

    let code_ptr = engine.module.get_finalized_function(id);
    Ok(JitFunctionI64::new(code_ptr, func.params.len()))
}
