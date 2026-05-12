use tempra_bytecode::function::BytecodeFunction;
use tempra_bytecode::opcode::{BytecodeInstr, stack_effect};
use tempra_bytecode::bytecode::BytecodeModule;

pub fn optimize_function(func: &mut BytecodeFunction, module: &BytecodeModule, first_temp_local: u32) {
    let original_code = func.code.clone();
    let original_max_stack = func.max_stack;

    // 1. Count uses of each local
    let mut use_counts = vec![0u32; func.locals.len()];
    for instr in &func.code {
        match instr {
            BytecodeInstr::LoadLocal(id) | BytecodeInstr::StoreLocal(id) => {
                if id.0 < use_counts.len() as u32 {
                    use_counts[id.0 as usize] += 1;
                }
            }
            _ => {}
        }
    }

    // 2. Identify Jump Targets to avoid optimizing across them
    let mut jump_targets = std::collections::HashSet::new();
    for instr in &func.code {
        match instr {
            BytecodeInstr::Jump(offset) | BytecodeInstr::JumpIfFalse(offset) => {
                jump_targets.insert(offset.0);
            }
            _ => {}
        }
    }

    // 3. Scan and optimize
    // We do it in a loop until no more changes or just one pass for v1
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < func.code.len() {
            // Pattern: Producer -> StoreLocal(tmp) -> LoadLocal(tmp)
            // We need i >= 1 (producer), i (store), i + 1 (load)
            if i >= 1 && i + 1 < func.code.len() {
                if let (BytecodeInstr::StoreLocal(s_id), BytecodeInstr::LoadLocal(l_id)) = (&func.code[i], &func.code[i+1]) {
                    if s_id == l_id && s_id.0 >= first_temp_local && use_counts[s_id.0 as usize] == 2 {
                        // Safety checks:
                        // - Is the previous instruction a producer?
                        // - Are there any jump targets pointing to the Store or Load?
                        let prev_is_producer = if let Ok(effect) = stack_effect(&func.code[i-1], module, func) {
                            effect.pushes == 1
                        } else {
                            false
                        };

                        if prev_is_producer && !jump_targets.contains(&i) && !jump_targets.contains(&(i + 1)) {
                            // Eliminate Store and Load
                            remove_at(func, i, 2);
                            // We need to rebuild jump_targets and use_counts if we were to continue in this pass
                            // but for v1 we just restart the loop or finish.
                            changed = true;
                            // Update use counts (they both become 0 now, but we don't care about others)
                            break; 
                        }
                    }
                }
            }
            i += 1;
        }
    }

    // 4. Re-calculate max_stack
    recalculate_max_stack(func, module);
    
    // 5. Verify final result
    if let Err(_) = tempra_bytecode::verify::verify_function_direct(module, func) {
        // Fallback on failure
        func.code = original_code;
        func.max_stack = original_max_stack;
    }
}

fn remove_at(func: &mut BytecodeFunction, index: usize, count: usize) {
    func.code.drain(index..index + count);
    
    // Fixup all jump offsets
    for instr in &mut func.code {
        match instr {
            BytecodeInstr::Jump(offset) | BytecodeInstr::JumpIfFalse(offset) => {
                if offset.0 > index {
                    // If the jump was pointing after the removed range, shift it back
                    offset.0 -= count;
                }
            }
            _ => {}
        }
    }
}

fn recalculate_max_stack(func: &mut BytecodeFunction, module: &BytecodeModule) {
    let mut current = 0i32;
    let mut max = 0u32;
    for instr in &func.code {
        if let Ok(effect) = stack_effect(instr, module, func) {
            current = current - effect.pops as i32 + effect.pushes as i32;
            if current > max as i32 {
                max = current as u32;
            }
        }
    }
    func.max_stack = max;
}
