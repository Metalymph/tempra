use crate::error::TirBuildError;
use crate::ir::*;
use std::collections::HashMap;

pub struct ModuleBuilder {
    pub module: TirModule,
    next_func_id: usize,
    pub function_table: HashMap<String, TirFunctionId>,
}

impl ModuleBuilder {
    pub fn new() -> Self {
        Self {
            module: TirModule::default(),
            next_func_id: 0,
            function_table: HashMap::new(),
        }
    }

    pub fn declare_function(&mut self, name: &str) -> TirFunctionId {
        if let Some(&id) = self.function_table.get(name) {
            id
        } else {
            let id = TirFunctionId(self.next_func_id);
            self.next_func_id += 1;
            self.function_table.insert(name.to_string(), id);
            id
        }
    }

    pub fn new_function(&mut self, name: &str, return_type: TirType) -> FunctionBuilder {
        let id = self.declare_function(name);
        FunctionBuilder::new(id, name, return_type)
    }

    pub fn add_function(&mut self, func: TirFunction) {
        // If it's already there, replace it (e.g. replacing a declaration with a definition)
        if let Some(existing) = self.module.functions.iter_mut().find(|f| f.id == func.id) {
            *existing = func;
        } else {
            self.module.functions.push(func);
        }
    }

    pub fn add_declaration(&mut self, name: &str, params: Vec<TirType>, return_type: TirType) {
        let id = self.declare_function(name);
        // Only add if not already defined/declared
        if !self.module.functions.iter().any(|f| f.id == id) {
            self.module.functions.push(TirFunction {
                id,
                name: name.to_string(),
                params: params.into_iter().map(|t| (TirValueId(0), t)).collect(), // Dummy value IDs for params in decl
                return_type,
                is_extern: false,
                blocks: Vec::new(),
                next_value_id: 0,
                next_block_id: 0,
                next_slot_id: 0,
                debug_slots: Default::default(),
            });
        }
    }
}

pub struct FunctionBuilder {
    pub func: TirFunction,
    current_block: Option<TirBlockId>,
}

impl FunctionBuilder {
    pub fn new(id: TirFunctionId, name: &str, return_type: TirType) -> Self {
        Self {
            func: TirFunction {
                id,
                name: name.to_string(),
                params: Vec::new(),
                return_type,
                is_extern: false,
                blocks: Vec::new(),
                next_value_id: 0,
                next_block_id: 0,
                next_slot_id: 0,
                debug_slots: Default::default(),
            },
            current_block: None,
        }
    }

    pub fn add_param(&mut self, ty: TirType) -> TirValueId {
        let id = self.next_value();
        self.func.params.push((id, ty));
        id
    }

    pub fn new_block(&mut self) -> TirBlockId {
        let id = TirBlockId(self.func.next_block_id);
        self.func.next_block_id += 1;
        self.func.blocks.push(TirBlock {
            id,
            instrs: Vec::new(),
            terminator: None,
        });
        id
    }

    pub fn set_current_block(&mut self, id: TirBlockId) {
        self.current_block = Some(id);
    }

    pub fn get_current_block(&self) -> Option<TirBlockId> {
        self.current_block
    }

    pub fn next_value(&mut self) -> TirValueId {
        let id = TirValueId(self.func.next_value_id);
        self.func.next_value_id += 1;
        id
    }

    pub fn next_slot(&mut self, name: Option<&str>) -> TirSlotId {
        let id = TirSlotId(self.func.next_slot_id);
        self.func.next_slot_id += 1;
        if let Some(n) = name {
            self.func.debug_slots.insert(id, n.to_string());
        }
        id
    }

    pub fn push_instr(&mut self, instr: TirInstr) -> Result<(), TirBuildError> {
        let block_id = self.current_block.ok_or(TirBuildError::NoCurrentBlock)?;
        let block = self
            .func
            .blocks
            .iter_mut()
            .find(|b| b.id == block_id)
            .unwrap();
        if block.terminator.is_some() {
            return Err(TirBuildError::BlockAlreadyTerminated(block_id));
        }
        block.instrs.push(instr);
        Ok(())
    }

    pub fn set_terminator(&mut self, term: TirTerminator) -> Result<(), TirBuildError> {
        let block_id = self.current_block.ok_or(TirBuildError::NoCurrentBlock)?;
        let block = self
            .func
            .blocks
            .iter_mut()
            .find(|b| b.id == block_id)
            .unwrap();
        if block.terminator.is_some() {
            return Err(TirBuildError::BlockAlreadyTerminated(block_id));
        }
        block.terminator = Some(term);
        Ok(())
    }

    pub fn is_terminated(&self) -> bool {
        if let Some(block_id) = self.current_block {
            if let Some(block) = self.func.blocks.iter().find(|b| b.id == block_id) {
                return block.terminator.is_some();
            }
        }
        false
    }

    pub fn current_block_terminated(&self) -> bool {
        self.is_terminated()
    }

    pub fn has_predecessors(&self, block_id: TirBlockId) -> bool {
        for block in &self.func.blocks {
            if let Some(term) = &block.terminator {
                match term {
                    TirTerminator::Br(target) => {
                        if *target == block_id {
                            return true;
                        }
                    }
                    TirTerminator::CondBr(_, t, f) => {
                        if *t == block_id || *f == block_id {
                            return true;
                        }
                    }
                    TirTerminator::Ret(_) | TirTerminator::Unreachable => {}
                }
            }
        }
        false
    }

    pub fn build(self) -> TirFunction {
        self.func
    }

    // Common Helpers
    pub fn build_const(&mut self, val: TirConst) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Const(id, val))?;
        Ok(id)
    }

    pub fn build_add(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Add(id, a, b))?;
        Ok(id)
    }

    pub fn build_sub(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Sub(id, a, b))?;
        Ok(id)
    }

    pub fn build_mul(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Mul(id, a, b))?;
        Ok(id)
    }

    pub fn build_div(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Div(id, a, b))?;
        Ok(id)
    }

    pub fn build_eq(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Eq(id, a, b))?;
        Ok(id)
    }

    pub fn build_lt(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Lt(id, a, b))?;
        Ok(id)
    }

    pub fn build_gt(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Gt(id, a, b))?;
        Ok(id)
    }

    pub fn build_and(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::And(id, a, b))?;
        Ok(id)
    }

    pub fn build_or(&mut self, a: TirValueId, b: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Or(id, a, b))?;
        Ok(id)
    }

    pub fn build_not(&mut self, a: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Not(id, a))?;
        Ok(id)
    }

    pub fn build_make(
        &mut self,
        ty: TirType,
        variant: Option<String>,
        args: Vec<TirValueId>,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Make(id, ty, variant, args))?;
        Ok(id)
    }

    pub fn build_make_struct(
        &mut self,
        name: String,
        fields: Vec<(String, TirValueId)>,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::MakeStruct(id, name, fields))?;
        Ok(id)
    }

    pub fn build_get(
        &mut self,
        target: TirValueId,
        key: TirKey,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Get(id, target, key))?;
        Ok(id)
    }

    pub fn build_set(
        &mut self,
        target: TirValueId,
        key: TirKey,
        val: TirValueId,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Set(id, target, key, val))?;
        Ok(id)
    }

    pub fn build_tag(&mut self, target: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Tag(id, target))?;
        Ok(id)
    }

    pub fn build_unwrap(
        &mut self,
        target: TirValueId,
        variant: String,
        index: u16,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Unwrap(id, target, variant, index))?;
        Ok(id)
    }

    pub fn build_call(
        &mut self,
        target: TirFunctionId,
        args: Vec<TirValueId>,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Call(id, target, args))?;
        Ok(id)
    }

    pub fn build_slot(
        &mut self,
        ty: TirType,
        name: Option<&str>,
    ) -> Result<TirSlotId, TirBuildError> {
        let slot = self.next_slot(name);
        self.push_instr(TirInstr::Slot(slot, ty))?;
        Ok(slot)
    }

    pub fn build_store(&mut self, slot: TirSlotId, val: TirValueId) -> Result<(), TirBuildError> {
        self.push_instr(TirInstr::Store(slot, val))?;
        Ok(())
    }

    pub fn build_load(&mut self, slot: TirSlotId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Load(id, slot))?;
        Ok(id)
    }

    pub fn build_condbr(
        &mut self,
        cond: TirValueId,
        t: TirBlockId,
        f: TirBlockId,
    ) -> Result<(), TirBuildError> {
        self.set_terminator(TirTerminator::CondBr(cond, t, f))?;
        Ok(())
    }

    pub fn build_br(&mut self, target: TirBlockId) -> Result<(), TirBuildError> {
        self.set_terminator(TirTerminator::Br(target))?;
        Ok(())
    }

    pub fn build_ret(&mut self, val: Option<TirValueId>) -> Result<(), TirBuildError> {
        self.set_terminator(TirTerminator::Ret(val))?;
        Ok(())
    }

    pub fn build_unreachable(&mut self) -> Result<(), TirBuildError> {
        self.set_terminator(TirTerminator::Unreachable)?;
        Ok(())
    }

    pub fn build_get_index(
        &mut self,
        object: TirValueId,
        index: TirValueId,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::GetIndex(id, object, index))?;
        Ok(id)
    }

    pub fn build_slice(
        &mut self,
        object: TirValueId,
        start: TirValueId,
        end: TirValueId,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Slice(id, object, start, end))?;
        Ok(id)
    }

    pub fn build_len(&mut self, object: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Len(id, object))?;
        Ok(id)
    }

    pub fn build_size(&mut self, object: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Size(id, object))?;
        Ok(id)
    }

    pub fn build_bytes(&mut self, object: TirValueId) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::Bytes(id, object))?;
        Ok(id)
    }

    pub fn build_array_push(
        &mut self,
        array: TirValueId,
        value: TirValueId,
    ) -> Result<TirValueId, TirBuildError> {
        let id = self.next_value();
        self.push_instr(TirInstr::ArrayPush(id, array, value))?;
        Ok(id)
    }
}
