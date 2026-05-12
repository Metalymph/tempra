use std::collections::HashMap;
use tempra_tir::ir::*;
use tempra_bytecode::*;
use tempra_bytecode::opcode::stack_effect;
use tempra_bytecode::constant::BytecodeConstant;
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
        slot_types: HashMap::new(),
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
    slot_types: HashMap<TirSlotId, BytecodeType>,
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

        // 2. Map TIR slots to locals and collect their types
        for block in &self.tir_func.blocks {
            for instr in &block.instrs {
                if let TirInstr::Slot(id, ty) = instr {
                    self.slot_to_local.insert(*id, LocalId(next_local));
                    self.slot_types.insert(*id, map_type(ty)?);
                    next_local += 1;
                }
            }
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

        let mut local_types = vec![BytecodeType::Unit; next_local as usize];

        // Map params types
        for (val_id, ty) in &self.tir_func.params {
            if let Some(local) = self.value_to_local.get(val_id) {
                local_types[local.0 as usize] = map_type(ty)?;
            }
        }

        // Map slot types
        for (slot_id, local) in &self.slot_to_local {
            local_types[local.0 as usize] = *self.slot_types.get(slot_id).unwrap();
        }

        // Map SSA values types
        for block in &self.tir_func.blocks {
            for instr in &block.instrs {
                if let Some(id) = get_instr_res_id(instr) {
                    if let Some(local) = self.value_to_local.get(&id) {
                        local_types[local.0 as usize] = self.infer_instr_res_type(instr)?;
                    }
                }
            }
        }

        self.bc_func.locals = local_types;

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
            TirInstr::Make(res, ty, variant, args) => {
                for arg in args {
                    self.load_val(*arg)?;
                }
                match ty {
                    TirType::Array(_) => {
                        self.emit(BytecodeInstr::MakeArray(args.len() as u16));
                    }
                    _ => {
                        let enum_name = match ty {
                            tempra_tir::ir::TirType::Option(_) => "Option",
                            tempra_tir::ir::TirType::Result(_, _) => "Result",
                            _ => "Enum",
                        };
                        let enum_id = self.bc_module.add_constant(BytecodeConstant::String(enum_name.to_string()));
                        let variant_id = self.bc_module.add_constant(BytecodeConstant::String(variant.clone().unwrap_or_else(|| "Unknown".to_string())));
                        self.emit(BytecodeInstr::MakeEnum(enum_id, variant_id, args.len() as u16));
                    }
                }
                self.store_res(*res)?;
            }
            TirInstr::MakeStruct(res, name, fields) => {
                let name_id = self.bc_module.add_constant(BytecodeConstant::String(name.clone()));
                let mut field_names = Vec::new();
                for (fname, val) in fields {
                    self.load_val(*val)?;
                    field_names.push(fname.clone());
                }
                let fields_id = self.bc_module.add_constant(BytecodeConstant::FieldList(field_names));
                self.emit(BytecodeInstr::MakeStruct(name_id, fields_id, fields.len() as u16));
                self.store_res(*res)?;
            }
            TirInstr::Get(res, target, key) => {
                self.load_val(*target)?;
                if let tempra_tir::ir::TirKey::Field(f) = key {
                    let field_id = self.bc_module.add_constant(BytecodeConstant::String(f.clone()));
                    self.emit(BytecodeInstr::GetField(field_id));
                } else {
                    return Err(BytecodeLowerError::UnsupportedInstr("Get with non-field key".into()));
                }
                self.store_res(*res)?;
            }
            TirInstr::Set(res, target, key, val) => {
                self.load_val(*target)?;
                self.load_val(*val)?;
                if let tempra_tir::ir::TirKey::Field(f) = key {
                    let field_id = self.bc_module.add_constant(BytecodeConstant::String(f.clone()));
                    self.emit(BytecodeInstr::SetField(field_id));
                } else {
                    return Err(BytecodeLowerError::UnsupportedInstr("Set with non-field key".into()));
                }
                self.store_res(*res)?;
            }
            TirInstr::Tag(res, target) => {
                self.load_val(*target)?;
                self.emit(BytecodeInstr::Tag);
                self.store_res(*res)?;
            }
            TirInstr::Unwrap(res, target, variant, idx) => {
                self.load_val(*target)?;
                let variant_id = self.bc_module.add_constant(BytecodeConstant::String(variant.clone()));
                self.emit(BytecodeInstr::Unwrap(variant_id, *idx));
                self.store_res(*res)?;
            }
            TirInstr::GetIndex(res, obj, idx) => {
                self.load_val(*obj)?;
                self.load_val(*idx)?;
                self.emit(BytecodeInstr::GetIndex);
                self.store_res(*res)?;
            }
            TirInstr::Slice(res, obj, start, end) => {
                self.load_val(*obj)?;
                self.load_val(*start)?;
                self.load_val(*end)?;
                self.emit(BytecodeInstr::Slice);
                self.store_res(*res)?;
            }
            TirInstr::Len(res, obj) => {
                self.load_val(*obj)?;
                self.emit(BytecodeInstr::Len);
                self.store_res(*res)?;
            }
            TirInstr::Size(res, obj) => {
                self.load_val(*obj)?;
                self.emit(BytecodeInstr::Size);
                self.store_res(*res)?;
            }
            TirInstr::Bytes(res, obj) => {
                self.load_val(*obj)?;
                self.emit(BytecodeInstr::Bytes);
                self.store_res(*res)?;
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

    fn infer_instr_res_type(&self, instr: &TirInstr) -> Result<BytecodeType, BytecodeLowerError> {
        match instr {
            TirInstr::Const(_, val) => match val {
                TirConst::Unit => Ok(BytecodeType::Unit),
                TirConst::Bool(_) => Ok(BytecodeType::Bool),
                TirConst::Int64(_) => Ok(BytecodeType::Int64),
                TirConst::UInt64(_) => Ok(BytecodeType::UInt64),
                TirConst::Float64(_) => Ok(BytecodeType::Float64),
                TirConst::String(_) => Ok(BytecodeType::String),
            },
            TirInstr::Load(_, slot_id) => {
                self.slot_types.get(slot_id).cloned()
                    .ok_or_else(|| BytecodeLowerError::UnknownSlot(format!("${}", slot_id.0)))
            }
            TirInstr::Add(_, _, _) | TirInstr::Sub(_, _, _) |
            TirInstr::Mul(_, _, _) | TirInstr::Div(_, _, _) => Ok(BytecodeType::Int64),
            TirInstr::Eq(_, _, _) | TirInstr::Lt(_, _, _) | TirInstr::Gt(_, _, _) |
            TirInstr::And(_, _, _) | TirInstr::Or(_, _, _) | TirInstr::Not(_, _) => Ok(BytecodeType::Bool),
            TirInstr::Call(_, func_id, _) => {
                let target = self.bc_module.functions.get(func_id.0 as usize)
                    .ok_or_else(|| BytecodeLowerError::UnknownFunction(format!("@{}", func_id.0)))?;
                Ok(target.ret)
            }
            TirInstr::Make(_, _, _, _) => Ok(BytecodeType::Any),
            TirInstr::MakeStruct(_, _, _) => Ok(BytecodeType::Any),
            TirInstr::Get(_, _, _) => Ok(BytecodeType::Any),
            TirInstr::Set(_, _, _, _) => Ok(BytecodeType::Any),
            TirInstr::Tag(_, _) => Ok(BytecodeType::String),
            TirInstr::Unwrap(_, _, _, _) => Ok(BytecodeType::Any),
            TirInstr::GetIndex(_, _, _) => Ok(BytecodeType::Any),
            TirInstr::Slice(_, _, _, _) => Ok(BytecodeType::Any),
            TirInstr::Len(_, _) => Ok(BytecodeType::Int64),
            TirInstr::Size(_, _) => Ok(BytecodeType::Int64),
            TirInstr::Bytes(_, _) => Ok(BytecodeType::Any),
            _ => Err(BytecodeLowerError::UnsupportedInstr(format!("Cannot infer type for {:?}", instr))),
        }
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
        TirInstr::Make(id, _, _, _) | TirInstr::MakeStruct(id, _, _) |
        TirInstr::Get(id, _, _) | TirInstr::Set(id, _, _, _) |
        TirInstr::Tag(id, _) | TirInstr::Unwrap(id, _, _, _) |
        TirInstr::GetIndex(id, _, _) | TirInstr::Slice(id, _, _, _) |
        TirInstr::Len(id, _) | TirInstr::Size(id, _) | TirInstr::Bytes(id, _) => Some(*id),
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
        TirType::Result(_, _) => Ok(BytecodeType::Any),
        TirType::Option(_) => Ok(BytecodeType::Any),
        TirType::Array(_) => Ok(BytecodeType::Any),
        TirType::Slice(_) => Ok(BytecodeType::Any),
        TirType::UInt8 => Ok(BytecodeType::UInt8),
        _ => Err(BytecodeLowerError::UnsupportedType(format!("{:?}", ty))),
    }
}
