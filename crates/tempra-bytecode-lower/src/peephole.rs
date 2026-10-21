use tempra_bytecode::bytecode::BytecodeModule;
use tempra_bytecode::function::BytecodeFunction;
use tempra_bytecode::opcode::{stack_effect, BytecodeInstr};

pub fn optimize_function(
    func: &mut BytecodeFunction,
    module: &BytecodeModule,
    first_temp_local: u32,
) {
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
            BytecodeInstr::AddI64LL(a, b) => {
                if a.0 < use_counts.len() as u32 {
                    use_counts[a.0 as usize] += 1;
                }
                if b.0 < use_counts.len() as u32 {
                    use_counts[b.0 as usize] += 1;
                }
            }
            BytecodeInstr::AddI64LC(a, _) if a.0 < use_counts.len() as u32 => {
                use_counts[a.0 as usize] += 1;
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
                if let (BytecodeInstr::StoreLocal(s_id), BytecodeInstr::LoadLocal(l_id)) =
                    (func.code[i], func.code[i + 1])
                {
                    if s_id == l_id
                        && s_id.0 >= first_temp_local
                        && use_counts[s_id.0 as usize] == 2
                    {
                        let prev_is_producer =
                            if let Ok(effect) = stack_effect(&func.code[i - 1], module, func) {
                                effect.pushes == 1
                            } else {
                                false
                            };

                        if prev_is_producer
                            && !jump_targets.contains(&i)
                            && !jump_targets.contains(&(i + 1))
                        {
                            remove_at(func, i, 2);
                            changed = true;
                            break;
                        }
                    }
                }
            }

            // Pattern: LoadLocal(a), LoadLocal(b), AddI64 -> AddI64LL(a, b)
            if i + 2 < func.code.len() {
                if let (
                    BytecodeInstr::LoadLocal(a),
                    BytecodeInstr::LoadLocal(b),
                    BytecodeInstr::AddI64,
                ) = (func.code[i], func.code[i + 1], func.code[i + 2])
                {
                    if !jump_targets.contains(&(i + 1)) && !jump_targets.contains(&(i + 2)) {
                        func.code[i] = BytecodeInstr::AddI64LL(a, b);
                        remove_at(func, i + 1, 2);
                        changed = true;
                        break;
                    }
                }

                // Pattern: LoadLocal(a), ConstI64(id), AddI64 -> AddI64LC(a, id)
                if let (
                    BytecodeInstr::LoadLocal(a),
                    BytecodeInstr::ConstI64(id),
                    BytecodeInstr::AddI64,
                ) = (func.code[i], func.code[i + 1], func.code[i + 2])
                {
                    if !jump_targets.contains(&(i + 1)) && !jump_targets.contains(&(i + 2)) {
                        func.code[i] = BytecodeInstr::AddI64LC(a, id);
                        remove_at(func, i + 1, 2);
                        changed = true;
                        break;
                    }
                }

                // Pattern: ConstI64(id), LoadLocal(a), AddI64 -> AddI64LC(a, id) (commutative)
                if let (
                    BytecodeInstr::ConstI64(id),
                    BytecodeInstr::LoadLocal(a),
                    BytecodeInstr::AddI64,
                ) = (func.code[i], func.code[i + 1], func.code[i + 2])
                {
                    if !jump_targets.contains(&(i + 1)) && !jump_targets.contains(&(i + 2)) {
                        func.code[i] = BytecodeInstr::AddI64LC(a, id);
                        remove_at(func, i + 1, 2);
                        changed = true;
                        break;
                    }
                }
            }

            // Complex Pattern: ConstI64(id), StoreLocal(tmp), LoadLocal(a), LoadLocal(tmp), AddI64
            // Becomes: AddI64LC(a, id)
            if i + 4 < func.code.len() {
                if let (
                    BytecodeInstr::ConstI64(id),
                    BytecodeInstr::StoreLocal(s_id),
                    BytecodeInstr::LoadLocal(a),
                    BytecodeInstr::LoadLocal(l_id),
                    BytecodeInstr::AddI64,
                ) = (
                    func.code[i],
                    func.code[i + 1],
                    func.code[i + 2],
                    func.code[i + 3],
                    func.code[i + 4],
                ) {
                    if s_id == l_id
                        && s_id.0 >= first_temp_local
                        && use_counts[s_id.0 as usize] == 2
                        && !jump_targets.contains(&(i + 1))
                        && !jump_targets.contains(&(i + 2))
                        && !jump_targets.contains(&(i + 3))
                        && !jump_targets.contains(&(i + 4))
                    {
                        func.code[i] = BytecodeInstr::AddI64LC(a, id);
                        remove_at(func, i + 1, 4);
                        changed = true;
                        break;
                    }
                }
            }

            // Complex Pattern: LoadLocal(a), ConstI64(id), StoreLocal(tmp), LoadLocal(tmp), AddI64
            // Becomes: AddI64LC(a, id)
            if i + 4 < func.code.len() {
                if let (
                    BytecodeInstr::LoadLocal(a),
                    BytecodeInstr::ConstI64(id),
                    BytecodeInstr::StoreLocal(s_id),
                    BytecodeInstr::LoadLocal(l_id),
                    BytecodeInstr::AddI64,
                ) = (
                    func.code[i],
                    func.code[i + 1],
                    func.code[i + 2],
                    func.code[i + 3],
                    func.code[i + 4],
                ) {
                    if s_id == l_id
                        && s_id.0 >= first_temp_local
                        && use_counts[s_id.0 as usize] == 2
                        && !jump_targets.contains(&(i + 1))
                        && !jump_targets.contains(&(i + 2))
                        && !jump_targets.contains(&(i + 3))
                        && !jump_targets.contains(&(i + 4))
                    {
                        func.code[i] = BytecodeInstr::AddI64LC(a, id);
                        remove_at(func, i + 1, 4);
                        changed = true;
                        break;
                    }
                }
            }
            i += 1;
        }
    }

    // 4. Re-calculate max_stack
    recalculate_max_stack(func, module);

    // 5. Verify final result
    if tempra_bytecode::verify::verify_function_direct(module, func).is_err() {
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
            BytecodeInstr::Jump(offset) | BytecodeInstr::JumpIfFalse(offset)
                if offset.0 > index =>
            {
                // If the jump was pointing after the removed range, shift it back
                offset.0 -= count;
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
