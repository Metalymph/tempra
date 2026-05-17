use std::collections::HashMap;
use tempra_tir::ir::*;
use tempra_tir::verifier::verify_module;

use crate::error::TirInterpError;
use crate::value::TirRuntimeValue;

/// Interpret a function by name within a TIR module.
///
/// Verifies the module first, then executes the named function
/// with the given arguments. Returns the function's return value.
pub fn interpret_function(
    module: &TirModule,
    function_name: &str,
    args: Vec<TirRuntimeValue>,
) -> Result<TirRuntimeValue, TirInterpError> {
    // Verify module before executing
    if let Err(errs) = verify_module(module) {
        let msgs: Vec<String> = errs.iter().map(|e| format!("{:?}", e)).collect();
        return Err(TirInterpError::VerifierFailed(msgs.join("; ")));
    }

    let func = module
        .functions
        .iter()
        .find(|f| f.name == function_name)
        .ok_or_else(|| TirInterpError::UnknownFunction(function_name.to_string()))?;

    if func.params.len() != args.len() {
        return Err(TirInterpError::WrongArity {
            name: function_name.to_string(),
            expected: func.params.len(),
            found: args.len(),
        });
    }

    let mut frame = Frame::new(func, module);

    // Bind parameters
    for ((param_id, _ty), arg_val) in func.params.iter().zip(args.into_iter()) {
        frame.values.insert(*param_id, arg_val);
    }

    frame.execute()
}

struct Frame<'a> {
    func: &'a TirFunction,
    module: &'a TirModule,
    values: HashMap<TirValueId, TirRuntimeValue>,
    slots: HashMap<TirSlotId, TirRuntimeValue>,
    current_block: usize,
}

impl<'a> Frame<'a> {
    fn new(func: &'a TirFunction, module: &'a TirModule) -> Self {
        Self {
            func,
            module,
            values: HashMap::new(),
            slots: HashMap::new(),
            current_block: 0,
        }
    }

    fn execute(&mut self) -> Result<TirRuntimeValue, TirInterpError> {
        if self.func.blocks.is_empty() {
            return Err(TirInterpError::NoEntryBlock);
        }

        loop {
            let block = &self.func.blocks[self.current_block];

            // Execute instructions
            for instr in &block.instrs {
                self.exec_instr(instr)?;
            }

            // Execute terminator
            let term = block
                .terminator
                .as_ref()
                .ok_or(TirInterpError::MissingTerminator(block.id))?;

            match term {
                TirTerminator::Ret(Some(val_id)) => {
                    let val = self.get_value(*val_id)?;
                    return Ok(val);
                }
                TirTerminator::Ret(None) => {
                    return Ok(TirRuntimeValue::Unit);
                }
                TirTerminator::Br(target) => {
                    self.current_block = self.find_block_index(*target)?;
                }
                TirTerminator::CondBr(cond_id, t_target, f_target) => {
                    let cond = self.get_value(*cond_id)?;
                    match cond {
                        TirRuntimeValue::Bool(true) => {
                            self.current_block = self.find_block_index(*t_target)?;
                        }
                        TirRuntimeValue::Bool(false) => {
                            self.current_block = self.find_block_index(*f_target)?;
                        }
                        _ => {
                            return Err(TirInterpError::TypeMismatch(format!(
                                "condbr condition must be Bool, got {:?}",
                                cond
                            )));
                        }
                    }
                }
                TirTerminator::Unreachable => {
                    return Err(TirInterpError::UnreachableExecuted);
                }
            }
        }
    }

    fn exec_instr(&mut self, instr: &TirInstr) -> Result<(), TirInterpError> {
        match instr {
            TirInstr::Const(res, val) => {
                let rv = match val {
                    TirConst::Unit => TirRuntimeValue::Unit,
                    TirConst::Bool(b) => TirRuntimeValue::Bool(*b),
                    TirConst::Int64(i) => TirRuntimeValue::Int64(*i),
                    TirConst::UInt64(u) => TirRuntimeValue::UInt64(*u),
                    TirConst::Float64(f) => TirRuntimeValue::Float64(*f),
                    TirConst::String(s) => TirRuntimeValue::String(s.clone()),
                };
                self.values.insert(*res, rv);
            }
            TirInstr::Slot(slot_id, _ty) => {
                // Initialize slot with Unit as placeholder
                self.slots.insert(*slot_id, TirRuntimeValue::Unit);
            }
            TirInstr::Load(res, slot_id) => {
                let val = self
                    .slots
                    .get(slot_id)
                    .ok_or(TirInterpError::MissingSlot(*slot_id))?
                    .clone();
                self.values.insert(*res, val);
            }
            TirInstr::Store(slot_id, val_id) => {
                let val = self.get_value(*val_id)?;
                self.slots.insert(*slot_id, val);
            }
            TirInstr::Add(res, a, b) => {
                let result =
                    self.arith_op(*a, *b, "add", |l, r| l + r, |l, r| l + r, |l, r| l + r)?;
                self.values.insert(*res, result);
            }
            TirInstr::Sub(res, a, b) => {
                let result =
                    self.arith_op(*a, *b, "sub", |l, r| l - r, |l, r| l - r, |l, r| l - r)?;
                self.values.insert(*res, result);
            }
            TirInstr::Mul(res, a, b) => {
                let result =
                    self.arith_op(*a, *b, "mul", |l, r| l * r, |l, r| l * r, |l, r| l * r)?;
                self.values.insert(*res, result);
            }
            TirInstr::Div(res, a, b) => {
                // Check for division by zero
                let b_val = self.get_value(*b)?;
                match &b_val {
                    TirRuntimeValue::Int64(0) | TirRuntimeValue::UInt64(0) => {
                        return Err(TirInterpError::DivisionByZero);
                    }
                    TirRuntimeValue::Float64(f) if *f == 0.0 => {
                        return Err(TirInterpError::DivisionByZero);
                    }
                    _ => {}
                }
                let result =
                    self.arith_op(*a, *b, "div", |l, r| l / r, |l, r| l / r, |l, r| l / r)?;
                self.values.insert(*res, result);
            }
            TirInstr::Eq(res, a, b) => {
                let lhs = self.get_value(*a)?;
                let rhs = self.get_value(*b)?;
                let result = lhs == rhs;
                self.values.insert(*res, TirRuntimeValue::Bool(result));
            }
            TirInstr::Lt(res, a, b) => {
                let result = self.cmp_op(
                    *a,
                    *b,
                    "lt",
                    |l, r| l < r,
                    |l, r| l < r,
                    |l: f64, r: f64| l < r,
                )?;
                self.values.insert(*res, TirRuntimeValue::Bool(result));
            }
            TirInstr::Gt(res, a, b) => {
                let result = self.cmp_op(
                    *a,
                    *b,
                    "gt",
                    |l, r| l > r,
                    |l, r| l > r,
                    |l: f64, r: f64| l > r,
                )?;
                self.values.insert(*res, TirRuntimeValue::Bool(result));
            }
            TirInstr::And(res, a, b) => {
                let lhs = self.get_bool(*a)?;
                let rhs = self.get_bool(*b)?;
                self.values.insert(*res, TirRuntimeValue::Bool(lhs && rhs));
            }
            TirInstr::Or(res, a, b) => {
                let lhs = self.get_bool(*a)?;
                let rhs = self.get_bool(*b)?;
                self.values.insert(*res, TirRuntimeValue::Bool(lhs || rhs));
            }
            TirInstr::Not(res, a) => {
                let val = self.get_bool(*a)?;
                self.values.insert(*res, TirRuntimeValue::Bool(!val));
            }
            TirInstr::Call(res, func_id, arg_ids) => {
                let callee = self
                    .module
                    .functions
                    .iter()
                    .find(|f| f.id == *func_id)
                    .ok_or_else(|| TirInterpError::UnknownFunction(format!("@{}", func_id.0)))?;

                let mut call_args = Vec::new();
                for arg_id in arg_ids {
                    call_args.push(self.get_value(*arg_id)?);
                }

                if callee.params.len() != call_args.len() {
                    return Err(TirInterpError::WrongArity {
                        name: callee.name.clone(),
                        expected: callee.params.len(),
                        found: call_args.len(),
                    });
                }

                // Recursive call via new frame — skip verification (already done at top level)
                let mut child_frame = Frame::new(callee, self.module);
                for ((param_id, _ty), arg_val) in callee.params.iter().zip(call_args.into_iter()) {
                    child_frame.values.insert(*param_id, arg_val);
                }
                let result = child_frame.execute()?;
                self.values.insert(*res, result);
            }
            _ => {
                return Err(TirInterpError::UnsupportedInstr(format!("{:?}", instr)));
            }
        }
        Ok(())
    }

    fn get_value(&self, id: TirValueId) -> Result<TirRuntimeValue, TirInterpError> {
        self.values
            .get(&id)
            .cloned()
            .ok_or(TirInterpError::MissingValue(id))
    }

    fn get_bool(&self, id: TirValueId) -> Result<bool, TirInterpError> {
        match self.get_value(id)? {
            TirRuntimeValue::Bool(b) => Ok(b),
            other => Err(TirInterpError::TypeMismatch(format!(
                "expected Bool, got {:?}",
                other
            ))),
        }
    }

    fn find_block_index(&self, target: TirBlockId) -> Result<usize, TirInterpError> {
        self.func
            .blocks
            .iter()
            .position(|b| b.id == target)
            .ok_or(TirInterpError::MissingBlock(target))
    }

    fn arith_op(
        &self,
        a: TirValueId,
        b: TirValueId,
        name: &str,
        i64_op: impl Fn(i64, i64) -> i64,
        u64_op: impl Fn(u64, u64) -> u64,
        f64_op: impl Fn(f64, f64) -> f64,
    ) -> Result<TirRuntimeValue, TirInterpError> {
        let lhs = self.get_value(a)?;
        let rhs = self.get_value(b)?;
        match (&lhs, &rhs) {
            (TirRuntimeValue::Int64(l), TirRuntimeValue::Int64(r)) => {
                Ok(TirRuntimeValue::Int64(i64_op(*l, *r)))
            }
            (TirRuntimeValue::UInt64(l), TirRuntimeValue::UInt64(r)) => {
                Ok(TirRuntimeValue::UInt64(u64_op(*l, *r)))
            }
            (TirRuntimeValue::Float64(l), TirRuntimeValue::Float64(r)) => {
                Ok(TirRuntimeValue::Float64(f64_op(*l, *r)))
            }
            _ => Err(TirInterpError::TypeMismatch(format!(
                "{}: incompatible types {:?} and {:?}",
                name, lhs, rhs
            ))),
        }
    }

    fn cmp_op(
        &self,
        a: TirValueId,
        b: TirValueId,
        name: &str,
        i64_op: impl Fn(i64, i64) -> bool,
        u64_op: impl Fn(u64, u64) -> bool,
        f64_op: impl Fn(f64, f64) -> bool,
    ) -> Result<bool, TirInterpError> {
        let lhs = self.get_value(a)?;
        let rhs = self.get_value(b)?;
        match (&lhs, &rhs) {
            (TirRuntimeValue::Int64(l), TirRuntimeValue::Int64(r)) => Ok(i64_op(*l, *r)),
            (TirRuntimeValue::UInt64(l), TirRuntimeValue::UInt64(r)) => Ok(u64_op(*l, *r)),
            (TirRuntimeValue::Float64(l), TirRuntimeValue::Float64(r)) => Ok(f64_op(*l, *r)),
            _ => Err(TirInterpError::TypeMismatch(format!(
                "{}: incompatible types {:?} and {:?}",
                name, lhs, rhs
            ))),
        }
    }
}
