use std::collections::HashMap;
use tempra_tir::ir::*;
use tempra_bytecode::*;
use tempra_bytecode::opcode::stack_effect;
use crate::error::BytecodeLowerError;

pub fn lower_tir_module(module: &TirModule) -> Result<BytecodeModule, BytecodeLowerError> {
    let mut bc_module = BytecodeModule::new();

    // First pass: declare all functions to populate the function table
    for tir_func in &module.functions {
        let params = tir_func.params.iter().map(|(_, ty)| map_type(ty)).collect::<Result<Vec<_>, _>>()?;
        let ret = map_type(&tir_func.return_type)?;
        bc_module.add_function(BytecodeFunction::new(tir_func.name.clone(), params, ret));
    }

    // Second pass: lower function bodies
    for i in 0..module.functions.len() {
        let tir_func = &module.functions[i];
        let bc_func = lower_function(tir_func, &mut bc_module)?;
        bc_module.functions[i] = bc_func;
    }

    Ok(bc_module)
}

fn lower_function(tir_func: &TirFunction, bc_module: &mut BytecodeModule) -> Result<BytecodeFunction, BytecodeLowerError> {
    let params = tir_func.params.iter().map(|(_, ty)| map_type(ty)).collect::<Result<Vec<_>, _>>()?;
    let ret = map_type(&tir_func.return_type)?;
    
    let mut lowerer = FunctionLowerer {
        tir_func,
        bc_module,
        bc_func: BytecodeFunction::new(tir_func.name.clone(), params, ret),
        value_to_local: HashMap::new(),
        slot_to_local: HashMap::new(),
        block_to_offset: HashMap::new(),
        jump_fixups: Vec::new(),
        current_stack: 0,
        max_stack: 0,
    };

    lowerer.lower()?;
    Ok(lowerer.bc_func)
}

struct FunctionLowerer<'a> {
    tir_func: &'a TirFunction,
    bc_module: &'a mut BytecodeModule,
    bc_func: BytecodeFunction,
    value_to_local: HashMap<TirValueId, LocalId>,
    slot_to_local: HashMap<TirSlotId, LocalId>,
    block_to_offset: HashMap<TirBlockId, usize>,
    jump_fixups: Vec<(usize, TirBlockId)>, 
    current_stack: u32,
    max_stack: u32,
}

impl<'a> FunctionLowerer<'a> {
    fn lower(&mut self) -> Result<(), BytecodeLowerError> {
        // 1. Map parameters to locals (0..N)
        for (i, (val_id, _ty)) in self.tir_func.params.iter().enumerate() {
            self.value_to_local.insert(*val_id, LocalId(i as u32));
        }
        let mut next_local = self.tir_func.params.len() as u32;

        // 2. Map TIR slots to locals
        let mut slot_ids: Vec<_> = self.tir_func.debug_slots.keys().cloned().collect();
        slot_ids.sort_by_key(|s| s.0);
        for slot_id in slot_ids {
            self.slot_to_local.insert(slot_id, LocalId(next_local));
            next_local += 1;
        }

        // 3. Map all other TirValueId to temporary locals
        for block in &self.tir_func.blocks {
            for instr in &block.instrs {
                if let Some(id) = get_instr_res_id(instr) {
                    if !self.value_to_local.contains_key(&id) {
                        self.value_to_local.insert(id, LocalId(next_local));
                        next_local += 1;
                    }
                }
            }
        }

        self.bc_func.locals = vec![BytecodeType::Int64; next_local as usize];

        // 4. Lower blocks
        for block in &self.tir_func.blocks {
            self.block_to_offset.insert(block.id, self.bc_func.code.len());
            
            for instr in &block.instrs {
                self.lower_instr(instr)?;
            }

            if let Some(term) = &block.terminator {
                self.lower_terminator(term)?;
            }
        }

        // 5. Patch jumps
        for (instr_idx, target_block) in &self.jump_fixups {
            let target_offset = self.block_to_offset.get(target_block)
                .ok_or_else(|| BytecodeLowerError::InvalidBlock(format!("^block{}", target_block.0)))?;
            
            match &mut self.bc_func.code[*instr_idx] {
                BytecodeInstr::Jump(off) => off.0 = *target_offset,
                BytecodeInstr::JumpIfFalse(off) => off.0 = *target_offset,
                _ => unreachable!(),
            }
        }

        self.bc_func.max_stack = self.max_stack;
        Ok(())
    }

    fn lower_instr(&mut self, instr: &TirInstr) -> Result<(), BytecodeLowerError> {
        match instr {
            TirInstr::Const(res, val) => {
                match val {
                    TirConst::Unit => self.emit(BytecodeInstr::ConstUnit),
                    TirConst::Bool(b) => self.emit(BytecodeInstr::ConstBool(*b)),
                    TirConst::Int64(i) => {
                        let id = self.bc_module.add_constant(BytecodeConstant::Int64(*i));
                        self.emit(BytecodeInstr::ConstI64(id));
                    }
                    TirConst::UInt64(u) => {
                        let id = self.bc_module.add_constant(BytecodeConstant::UInt64(*u));
                        self.emit(BytecodeInstr::ConstU64(id));
                    }
                    TirConst::Float64(f) => {
                        let id = self.bc_module.add_constant(BytecodeConstant::Float64(*f));
                        self.emit(BytecodeInstr::ConstF64(id));
                    }
                    TirConst::String(s) => {
                        let id = self.bc_module.add_constant(BytecodeConstant::String(s.clone()));
                        self.emit(BytecodeInstr::ConstString(id));
                    }
                }
                self.store_res(*res)?;
            }
            TirInstr::Load(res, slot) => {
                let local = self.slot_to_local.get(slot)
                    .ok_or_else(|| BytecodeLowerError::UnknownSlot(format!("${}", slot.0)))?;
                self.emit(BytecodeInstr::LoadLocal(*local));
                self.store_res(*res)?;
            }
            TirInstr::Store(slot, val) => {
                self.load_val(*val)?;
                let local = self.slot_to_local.get(slot)
                    .ok_or_else(|| BytecodeLowerError::UnknownSlot(format!("${}", slot.0)))?;
                self.emit(BytecodeInstr::StoreLocal(*local));
            }
            TirInstr::Add(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::AddI64);
                self.store_res(*res)?;
            }
            TirInstr::Sub(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::SubI64);
                self.store_res(*res)?;
            }
            TirInstr::Mul(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::MulI64);
                self.store_res(*res)?;
            }
            TirInstr::Div(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::DivI64);
                self.store_res(*res)?;
            }
            TirInstr::Eq(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::Eq);
                self.store_res(*res)?;
            }
            TirInstr::Lt(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::LtI64);
                self.store_res(*res)?;
            }
            TirInstr::Gt(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::GtI64);
                self.store_res(*res)?;
            }
            TirInstr::And(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::And);
                self.store_res(*res)?;
            }
            TirInstr::Or(res, a, b) => {
                self.load_val(*a)?;
                self.load_val(*b)?;
                self.emit(BytecodeInstr::Or);
                self.store_res(*res)?;
            }
            TirInstr::Not(res, a) => {
                self.load_val(*a)?;
                self.emit(BytecodeInstr::Not);
                self.store_res(*res)?;
            }
            TirInstr::Call(res, func_id, args) => {
                for arg in args {
                    self.load_val(*arg)?;
                }
                self.emit(BytecodeInstr::Call(FunctionId(func_id.0 as u32), args.len() as u16));
                self.store_res(*res)?;
            }
            TirInstr::Slot(_, _) => {}
            TirInstr::Make(_, _, _) | TirInstr::Get(_, _, _) | TirInstr::Set(_, _, _) |
            TirInstr::Tag(_, _) | TirInstr::Unwrap(_, _) => {
                return Err(BytecodeLowerError::UnsupportedInstr(format!("{:?}", instr)));
            }
        }
        Ok(())
    }

    fn lower_terminator(&mut self, term: &TirTerminator) -> Result<(), BytecodeLowerError> {
        match term {
            TirTerminator::Ret(val) => {
                if let Some(id) = val {
                    self.load_val(*id)?;
                }
                self.emit(BytecodeInstr::Return);
            }
            TirTerminator::Br(target) => {
                let idx = self.bc_func.code.len();
                self.emit(BytecodeInstr::Jump(InstrOffset(0)));
                self.jump_fixups.push((idx, *target));
            }
            TirTerminator::CondBr(cond, t_target, f_target) => {
                self.load_val(*cond)?;
                let jf_idx = self.bc_func.code.len();
                self.emit(BytecodeInstr::JumpIfFalse(InstrOffset(0)));
                self.jump_fixups.push((jf_idx, *f_target));
                
                let j_idx = self.bc_func.code.len();
                self.emit(BytecodeInstr::Jump(InstrOffset(0)));
                self.jump_fixups.push((j_idx, *t_target));
            }
            TirTerminator::Unreachable => {
                self.emit(BytecodeInstr::Unreachable);
            }
        }
        Ok(())
    }

    fn load_val(&mut self, id: TirValueId) -> Result<(), BytecodeLowerError> {
        let local = self.value_to_local.get(&id)
            .ok_or_else(|| BytecodeLowerError::UnknownValue(format!("%{}", id.0)))?;
        self.emit(BytecodeInstr::LoadLocal(*local));
        Ok(())
    }

    fn store_res(&mut self, id: TirValueId) -> Result<(), BytecodeLowerError> {
        let local = self.value_to_local.get(&id)
            .ok_or_else(|| BytecodeLowerError::UnknownValue(format!("%{}", id.0)))?;
        self.emit(BytecodeInstr::StoreLocal(*local));
        Ok(())
    }

    fn emit(&mut self, instr: BytecodeInstr) {
        if let Ok(effect) = stack_effect(&instr, self.bc_module, &self.bc_func) {
             self.current_stack = (self.current_stack as i32 - effect.pops as i32 + effect.pushes as i32) as u32;
        }
        if self.current_stack > self.max_stack {
            self.max_stack = self.current_stack;
        }
        self.bc_func.code.push(instr);
    }
}

fn get_instr_res_id(instr: &TirInstr) -> Option<TirValueId> {
    match instr {
        TirInstr::Const(id, _) => Some(*id),
        TirInstr::Load(id, _) => Some(*id),
        TirInstr::Add(id, _, _) | TirInstr::Sub(id, _, _) |
        TirInstr::Mul(id, _, _) | TirInstr::Div(id, _, _) |
        TirInstr::Eq(id, _, _) | TirInstr::Lt(id, _, _) | TirInstr::Gt(id, _, _) |
        TirInstr::And(id, _, _) | TirInstr::Or(id, _, _) | TirInstr::Not(id, _) |
        TirInstr::Call(id, _, _) => Some(*id),
        TirInstr::Make(id, _, _) | TirInstr::Get(id, _, _) | TirInstr::Set(id, _, _) |
        TirInstr::Tag(id, _) | TirInstr::Unwrap(id, _) => Some(*id),
        _ => None,
    }
}

fn map_type(ty: &TirType) -> Result<BytecodeType, BytecodeLowerError> {
    match ty {
        TirType::Unit => Ok(BytecodeType::Unit),
        TirType::Bool => Ok(BytecodeType::Bool),
        TirType::Int64 => Ok(BytecodeType::Int64),
        TirType::UInt64 => Ok(BytecodeType::UInt64),
        TirType::Float64 => Ok(BytecodeType::Float64),
        TirType::String => Ok(BytecodeType::String),
        _ => Err(BytecodeLowerError::UnsupportedType(format!("{:?}", ty))),
    }
}
