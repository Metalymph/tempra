use crate::arithmetic::{
    add_i64, div_i64, div_u8, mul_i64, neg_i64, sub_i64, IntegerArithmeticError,
};
use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::{OptionValue, RegexValue, StructInstance, Value};
use std::collections::HashMap;
use tempra_hir::*;

fn integer_value<T>(
    result: Result<T, IntegerArithmeticError>,
    into_value: impl FnOnce(T) -> Value,
) -> Result<ControlFlow, String> {
    result
        .map(into_value)
        .map(ControlFlow::Value)
        .map_err(|error| error.to_string())
}

fn run_command(c: &crate::value::CommandValue) -> Result<Value, String> {
    let mut cmd = std::process::Command::new(&c.program);
    cmd.args(&c.args);
    if let Some(cwd) = &c.cwd {
        cmd.current_dir(cwd);
    }
    let output = cmd.output().map_err(|e| format!("Command error: {}", e))?;
    let res = crate::value::CommandResultValue {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    };

    if c.check && !res.success {
        Ok(Value::Result(Box::new(crate::value::ValueResult::Err(
            Value::Error(format!(
                "Command '{}' failed with exit code {}",
                c.program, res.exit_code
            )),
        ))))
    } else {
        Ok(Value::Result(Box::new(crate::value::ValueResult::Ok(
            Value::CommandResult(res),
        ))))
    }
}

fn pipe_commands(
    left: &crate::value::CommandValue,
    right: &crate::value::CommandValue,
) -> Result<Value, String> {
    let left_output = std::process::Command::new(&left.program)
        .args(&left.args)
        .output()
        .map_err(|e| format!("Left command error: {}", e))?;

    use std::io::Write;
    let mut right_child = std::process::Command::new(&right.program)
        .args(&right.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Right command error: {}", e))?;

    if let Some(mut stdin) = right_child.stdin.take() {
        let _ = stdin.write_all(&left_output.stdout);
    }

    let output = right_child
        .wait_with_output()
        .map_err(|e| format!("Right wait error: {}", e))?;
    Ok(Value::CommandResult(crate::value::CommandResultValue {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    }))
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Int64(i) => i.to_string(),
        Value::UInt8(i) => i.to_string(),
        Value::Float64(f) => f.to_string(),
        // For other types, we use their Display implementation if they have one,
        // but the prompt is specific about which types to support in the helper.
        _ => value.to_string(),
    }
}

fn format_hir_type(ty: &HirType) -> String {
    match ty {
        HirType::Named(_, n) => n.clone(),
        HirType::Array(_, inner) => format!("[{}]", format_hir_type(inner)),
        HirType::Tuple(_, elems) => {
            let inner = elems
                .iter()
                .map(|e| format_hir_type(e))
                .collect::<Vec<_>>()
                .join(", ");
            format!("({})", inner)
        }
        HirType::Apply(_, callee, args) => {
            let base = format_hir_type(callee);
            let inner = args
                .iter()
                .map(|a| format_hir_type(a))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({})", base, inner)
        }
        HirType::Optional(_, inner) => format!("{}?", format_hir_type(inner)),
        HirType::Result(_, ok, err) => {
            format!("Result({}, {})", format_hir_type(ok), format_hir_type(err))
        }
        HirType::Function(_, params, ret) => {
            let p = params
                .iter()
                .map(|p| format_hir_type(p))
                .collect::<Vec<_>>()
                .join(", ");
            format!("fn({}) -> {}", p, format_hir_type(ret))
        }
        HirType::SelfType(_) => "Self".to_string(),
        HirType::Projection(_, base, member) => format!("{}.{}", format_hir_type(base), member),
    }
}

impl Interpreter {
    fn get_type_name(ty: &HirType) -> String {
        match ty {
            HirType::Named(_, n) => n.clone(),
            HirType::Apply(_, callee, _) => Self::get_type_name(callee),
            HirType::Projection(_, base, member) => {
                format!("{}.{}", Self::get_type_name(base), member)
            }
            _ => "Unknown".to_string(),
        }
    }

    fn value_is_type(value: &Value, ty: &HirType) -> bool {
        match ty {
            HirType::Named(_, name) => match name.as_str() {
                "Unit" => matches!(value, Value::Unit),
                "Bool" => matches!(value, Value::Bool(_)),
                "String" => matches!(value, Value::String(_)),
                "Int" | "Int64" => matches!(value, Value::Int64(_)),
                "UInt8" => matches!(value, Value::UInt8(_)),
                "Float64" => matches!(value, Value::Float64(_)),
                "Error" => matches!(value, Value::Error(_)),
                "Never" => false,
                _ => {
                    if let Value::Struct(s) = value {
                        s.name == *name
                    } else if let Value::Enum(e) = value {
                        e.enum_name == *name
                    } else {
                        false
                    }
                }
            },
            HirType::Array(_, _) => matches!(value, Value::Array(_)),
            HirType::Tuple(_, elems) => {
                if let Value::Tuple(values) = value {
                    values.len() == elems.len()
                        && values
                            .iter()
                            .zip(elems.iter())
                            .all(|(v, t)| Self::value_is_type(v, t))
                } else {
                    false
                }
            }
            HirType::Apply(_, callee, args) => {
                let name = Self::get_type_name(callee);
                if name == "Option" && args.len() == 1 {
                    Self::value_is_type(
                        value,
                        &HirType::Optional(callee.span(), Box::new(args[0].clone())),
                    )
                } else if name == "Result" && args.len() == 2 {
                    Self::value_is_type(
                        value,
                        &HirType::Result(
                            callee.span(),
                            Box::new(args[0].clone()),
                            Box::new(args[1].clone()),
                        ),
                    )
                } else {
                    match value {
                        Value::Struct(s) => s.name == name,
                        Value::Enum(e) => e.enum_name == name,
                        _ => false,
                    }
                }
            }
            HirType::Optional(_, inner) => match value {
                Value::Option(OptionValue::None) => true,
                Value::Option(OptionValue::Some(v)) => Self::value_is_type(v, inner),
                _ => false,
            },
            HirType::Result(_, ok, err) => match value {
                Value::Result(res) => match res.as_ref() {
                    crate::value::ValueResult::Ok(v) => Self::value_is_type(v, ok),
                    crate::value::ValueResult::Err(e) => Self::value_is_type(e, err),
                },
                _ => false,
            },
            HirType::Function(_, _, _) => matches!(value, Value::Closure(_)),
            HirType::SelfType(_) => false, // Should not happen at runtime
            HirType::Projection(_, _, _) => false, // Should not happen at runtime
        }
    }

    pub fn eval_expression(
        &mut self,
        expr: &HirExpression,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        if self.mode == crate::interpreter::EvalMode::Comptime {
            if let Some(fuel) = &mut self.fuel {
                if *fuel == 0 {
                    return Err(crate::comptime::FUEL_EXHAUSTED_MESSAGE.to_string());
                }
                *fuel -= 1;
            }
        }
        match expr {
            HirExpression::Integer(_, s) => {
                let v = s.parse::<i64>().map_err(|_| "Invalid Int64".to_string())?;
                Ok(ControlFlow::Value(Value::Int64(v)))
            }
            HirExpression::Float(_, s) => {
                let v = s
                    .parse::<f64>()
                    .map_err(|_| "Invalid Float64".to_string())?;
                Ok(ControlFlow::Value(Value::Float64(v)))
            }
            HirExpression::String(_, s) => {
                let unquoted = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    s[1..s.len() - 1].to_string()
                } else {
                    s.clone()
                };
                Ok(ControlFlow::Value(Value::String(unquoted)))
            }
            HirExpression::Bool(_, b) => Ok(ControlFlow::Value(Value::Bool(*b))),
            HirExpression::EnumConstructor(cons) => {
                let mut payloads = Vec::new();
                for arg in &cons.payloads {
                    match self.eval_expression(arg, env)? {
                        ControlFlow::Value(v) => payloads.push(v),
                        cf => return Ok(cf),
                    }
                }
                Ok(ControlFlow::Value(Value::Enum(Box::new(
                    crate::value::EnumValue {
                        enum_name: cons.enum_name.clone(),
                        variant_name: cons.variant_name.clone(),
                        payloads,
                    },
                ))))
            }
            HirExpression::Name(_, name) => {
                if name == "None" {
                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                } else if let Some(val) = env.get(name) {
                    Ok(ControlFlow::Value(val))
                } else if matches!(name.as_str(), "fs" | "os" | "env" | "Regex") {
                    Ok(ControlFlow::Value(Value::Namespace(name.clone())))
                } else if self.structs.contains_key(name) {
                    Ok(ControlFlow::Value(Value::Type(Type::Struct(
                        name.clone(),
                        Vec::new(),
                    ))))
                } else if self.enums.contains_key(name) {
                    Ok(ControlFlow::Value(Value::Type(Type::Enum(
                        name.clone(),
                        Vec::new(),
                    ))))
                } else if self.interfaces.contains_key(name) {
                    Ok(ControlFlow::Value(Value::Type(Type::Struct(
                        name.clone(),
                        Vec::new(),
                    )))) // Interfaces as Struct for now or Type::Interface?
                } else {
                    let ty = tempra_hir::types::resolve_named_type(name);
                    if ty != tempra_hir::types::Type::Unknown {
                        Ok(ControlFlow::Value(Value::Type(ty)))
                    } else {
                        Err(format!("Variable {} not found", name))
                    }
                }
            }
            HirExpression::Unary(unary) => {
                let val = match self.eval_expression(&unary.expr, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                match unary.op {
                    HirUnaryOp::Not => {
                        if let Value::Bool(b) = val {
                            Ok(ControlFlow::Value(Value::Bool(!b)))
                        } else {
                            Err("! requires a Bool".into())
                        }
                    }
                    HirUnaryOp::Negate => match val {
                        Value::Int64(i) => integer_value(neg_i64(i), Value::Int64),
                        Value::Float64(f) => Ok(ControlFlow::Value(Value::Float64(-f))),
                        _ => Err("- requires a numeric type".into()),
                    },
                }
            }
            HirExpression::Binary(binary) => {
                // Phase 7: Special handling for Assignment to avoid evaluating LHS as an expression
                // (which would incorrectly trigger 'get' hooks instead of 'set' hooks)
                if binary.op == HirBinaryOp::Assign {
                    let right = match self.eval_expression(&binary.right, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };

                    match &*binary.left {
                        HirExpression::Name(_, name) => {
                            if !env.update(name, right.clone()) {
                                env.insert(name.clone(), right.clone());
                            }
                            return Ok(ControlFlow::Value(right));
                        }
                        HirExpression::Member(m) => {
                            // Target: m.object . m.field
                            if let HirExpression::Name(_, obj_name) = &*m.object {
                                let mut obj_val = env
                                    .get(obj_name)
                                    .ok_or_else(|| format!("Variable {} not found", obj_name))?;
                                if let Value::Struct(ref mut s) = obj_val {
                                    s.fields.insert(m.field.clone(), right.clone());
                                    if !env.update(obj_name, obj_val.clone()) {
                                        env.insert(obj_name.clone(), obj_val);
                                    }
                                    return Ok(ControlFlow::Value(right));
                                } else {
                                    return Err("Member assignment target is not a struct".into());
                                }
                            } else {
                                return Err("Complex member assignment not yet supported".into());
                            }
                        }
                        // Phase 7: Index assignment xs[i] = v
                        HirExpression::Index(idx) => {
                            let receiver_name = Self::extract_receiver_name(&idx.object)
                                .ok_or("Invalid index assignment target")?;
                            let obj = env
                                .get(&receiver_name)
                                .ok_or_else(|| format!("Variable '{}' not found", receiver_name))?;
                            let idx_val = match self.eval_expression(&idx.index, env)? {
                                ControlFlow::Value(v) => v,
                                cf => return Ok(cf),
                            };

                            match obj {
                                Value::Array(arr) => {
                                    if let Value::Int64(i) = idx_val {
                                        let idx = i as usize;
                                        let mut arr_mut = arr.borrow_mut();
                                        if idx >= arr_mut.len() {
                                            return Ok(ControlFlow::Crash(format!(
                                                "Index out of bounds: {} for length {}",
                                                i,
                                                arr_mut.len()
                                            )));
                                        }
                                        arr_mut[idx] = right.clone();
                                        return Ok(ControlFlow::Value(right));
                                    }
                                    return Err("Index must be Int64".into());
                                }
                                Value::Slice(slice) => {
                                    if let Value::Int64(i) = idx_val {
                                        let idx = i as usize;
                                        if idx >= slice.len() {
                                            return Ok(ControlFlow::Crash(format!(
                                                "Index out of bounds: {} for slice length {}",
                                                i,
                                                slice.len()
                                            )));
                                        }
                                        match &slice.parent {
                                            crate::value::SliceParent::Values(v) => {
                                                v.borrow_mut()[slice.start + idx] = right.clone();
                                            }
                                            crate::value::SliceParent::Bytes(_) => {
                                                return Err(
                                                    "Cannot assign to byte slice element".into()
                                                );
                                            }
                                        }
                                        return Ok(ControlFlow::Value(right));
                                    }
                                    return Err("Index must be Int64".into());
                                }
                                Value::Struct(_) | Value::Enum(_) => {
                                    // User-defined set hook
                                    return self.eval_method_call_inout(
                                        &receiver_name,
                                        "set",
                                        vec![idx_val, right],
                                        env,
                                    );
                                }
                                _ => {
                                    return Err(format!(
                                        "Cannot assign to index of type {}",
                                        obj.type_name()
                                    ));
                                }
                            }
                        }
                        // Phase 7: Slice assignment xs[a..b] = values
                        HirExpression::Slice(slc) => {
                            let receiver_name = Self::extract_receiver_name(&slc.object)
                                .ok_or("Invalid slice assignment target")?;
                            let obj = env
                                .get(&receiver_name)
                                .ok_or_else(|| format!("Variable '{}' not found", receiver_name))?;
                            let start_val = match self.eval_expression(&slc.start, env)? {
                                ControlFlow::Value(v) => v,
                                cf => return Ok(cf),
                            };
                            let end_val = match self.eval_expression(&slc.end, env)? {
                                ControlFlow::Value(v) => v,
                                cf => return Ok(cf),
                            };

                            match obj {
                                Value::Array(arr) => {
                                    if let (Value::Int64(start_i), Value::Int64(end_i)) =
                                        (start_val, end_val)
                                    {
                                        let start = start_i as usize;
                                        let end = end_i as usize;
                                        if start > end {
                                            return Err(format!(
                                                "Slice start > end: {} > {}",
                                                start, end
                                            ));
                                        }
                                        let slice_len = end - start;

                                        // RHS can be Array or Slice
                                        let rhs_values = match right {
                                            Value::Array(ref a) => a.borrow().clone(),
                                            Value::Slice(ref s) => s.to_vec(),
                                            _ => {
                                                return Err(
                                                    "Slice assignment RHS must be array or slice"
                                                        .into(),
                                                );
                                            }
                                        };

                                        // Runtime length check
                                        if rhs_values.len() != slice_len {
                                            return Ok(ControlFlow::Crash(format!(
                                                "slice assignment length mismatch: expected {}, got {}",
                                                slice_len,
                                                rhs_values.len()
                                            )));
                                        }

                                        let mut arr_mut = arr.borrow_mut();
                                        if end > arr_mut.len() {
                                            return Ok(ControlFlow::Crash(format!(
                                                "Slice bounds out of range: [{}..{}] for length {}",
                                                start,
                                                end,
                                                arr_mut.len()
                                            )));
                                        }
                                        for (j, val) in rhs_values.into_iter().enumerate() {
                                            arr_mut[start + j] = val;
                                        }
                                        return Ok(ControlFlow::Value(right));
                                    }
                                    return Err("Slice bounds must be Int64".into());
                                }
                                Value::Slice(slice) => {
                                    if let (Value::Int64(start_i), Value::Int64(end_i)) =
                                        (start_val, end_val)
                                    {
                                        let start = start_i as usize;
                                        let end = end_i as usize;
                                        if start > end {
                                            return Err(format!(
                                                "Slice start > end: {} > {}",
                                                start, end
                                            ));
                                        }
                                        let sub_slice_len = end - start;
                                        if end > slice.len() {
                                            return Ok(ControlFlow::Crash(format!(
                                                "Slice bounds out of range: [{}..{}] for slice length {}",
                                                start,
                                                end,
                                                slice.len()
                                            )));
                                        }

                                        // RHS can be Array or Slice
                                        let rhs_values = match right {
                                            Value::Array(ref a) => a.borrow().clone(),
                                            Value::Slice(ref s) => s.to_vec(),
                                            _ => {
                                                return Err(
                                                    "Slice assignment RHS must be array or slice"
                                                        .into(),
                                                );
                                            }
                                        };

                                        if rhs_values.len() != sub_slice_len {
                                            return Ok(ControlFlow::Crash(format!(
                                                "slice assignment length mismatch: expected {}, got {}",
                                                sub_slice_len,
                                                rhs_values.len()
                                            )));
                                        }

                                        match &slice.parent {
                                            crate::value::SliceParent::Values(v) => {
                                                let mut v_mut = v.borrow_mut();
                                                for (j, val) in rhs_values.into_iter().enumerate() {
                                                    v_mut[slice.start + start + j] = val;
                                                }
                                            }
                                            crate::value::SliceParent::Bytes(_) => {
                                                return Err("Cannot assign to byte slice".into());
                                            }
                                        }
                                        return Ok(ControlFlow::Value(right));
                                    }
                                    return Err("Slice bounds must be Int64".into());
                                }
                                Value::Struct(_) | Value::Enum(_) => {
                                    // User-defined set_slice hook
                                    return self.eval_method_call_inout(
                                        &receiver_name,
                                        "set_slice",
                                        vec![start_val, end_val, right],
                                        env,
                                    );
                                }
                                _ => {
                                    return Err(format!(
                                        "Cannot assign to slice of type {}",
                                        obj.type_name()
                                    ));
                                }
                            }
                        }
                        _ => {
                            return Err("Invalid assignment target".into());
                        }
                    }
                }

                let mut v_left = None;

                // Special handling for short-circuiting logical operators and pipe
                if binary.op == HirBinaryOp::And {
                    let left = match self.eval_expression(&binary.left, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if let Value::Bool(false) = left {
                        return Ok(ControlFlow::Value(Value::Bool(false)));
                    } else if let Value::Command(ref cmd_left) = left {
                        let res_v = run_command(cmd_left)?;
                        let success = match res_v {
                            Value::Result(ref r) => match r.as_ref() {
                                crate::value::ValueResult::Ok(Value::CommandResult(cr)) => {
                                    cr.success
                                }
                                crate::value::ValueResult::Err(_) => false,
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        };
                        if !success {
                            return Ok(ControlFlow::Value(res_v));
                        }
                        let right = match self.eval_expression(&binary.right, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right).map(ControlFlow::Value);
                        } else {
                            return Err(
                                "&& right operand must be Command if left is Command".into()
                            );
                        }
                    }
                    v_left = Some(left);
                } else if binary.op == HirBinaryOp::Or {
                    let left = match self.eval_expression(&binary.left, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if let Value::Bool(true) = left {
                        return Ok(ControlFlow::Value(Value::Bool(true)));
                    } else if let Value::Command(ref cmd_left) = left {
                        let res_v = run_command(cmd_left)?;
                        let success = match res_v {
                            Value::Result(ref r) => match r.as_ref() {
                                crate::value::ValueResult::Ok(Value::CommandResult(cr)) => {
                                    cr.success
                                }
                                crate::value::ValueResult::Err(_) => false,
                                _ => unreachable!(),
                            },
                            _ => unreachable!(),
                        };
                        if success {
                            return Ok(ControlFlow::Value(res_v));
                        }
                        let right = match self.eval_expression(&binary.right, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right).map(ControlFlow::Value);
                        } else {
                            return Err(
                                "|| right operand must be Command if left is Command".into()
                            );
                        }
                    }
                    v_left = Some(left);
                } else if binary.op == HirBinaryOp::Pipe {
                    let left = match self.eval_expression(&binary.left, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    let right = match self.eval_expression(&binary.right, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if let (Value::Command(cmd_left), Value::Command(cmd_right)) = (left, right) {
                        return pipe_commands(&cmd_left, &cmd_right).map(ControlFlow::Value);
                    } else {
                        return Err("| operator expects Commands".into());
                    }
                }

                let left = if let Some(v) = v_left {
                    v
                } else {
                    match self.eval_expression(&binary.left, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    }
                };

                let right = match self.eval_expression(&binary.right, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                match binary.op {
                    HirBinaryOp::Add => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            integer_value(add_i64(a, b), Value::Int64)
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::UInt8(a.wrapping_add(b))))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a + b)))
                        }
                        (Value::String(a), Value::String(b)) => {
                            Ok(ControlFlow::Value(Value::String(a + &b)))
                        }
                        _ => Err("+ operator mismatch".into()),
                    },
                    HirBinaryOp::Sub => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            integer_value(sub_i64(a, b), Value::Int64)
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::UInt8(a.wrapping_sub(b))))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a - b)))
                        }
                        _ => Err("- operator mismatch".into()),
                    },
                    HirBinaryOp::Mul => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            integer_value(mul_i64(a, b), Value::Int64)
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::UInt8(a.wrapping_mul(b))))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a * b)))
                        }
                        _ => Err("* operator mismatch".into()),
                    },
                    HirBinaryOp::Div => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            integer_value(div_i64(a, b), Value::Int64)
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            integer_value(div_u8(a, b), Value::UInt8)
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a / b)))
                        }
                        _ => Err("/ operator mismatch".into()),
                    },
                    HirBinaryOp::Eq => Ok(ControlFlow::Value(Value::Bool(left == right))),
                    HirBinaryOp::NotEq => Ok(ControlFlow::Value(Value::Bool(left != right))),
                    HirBinaryOp::Less => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a < b)))
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a < b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a < b)))
                        }
                        _ => Err("< operator mismatch".into()),
                    },
                    HirBinaryOp::LessEq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a <= b)))
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a <= b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a <= b)))
                        }
                        _ => Err("<= operator mismatch".into()),
                    },
                    HirBinaryOp::Greater => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a > b)))
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a > b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a > b)))
                        }
                        _ => Err("> operator mismatch".into()),
                    },
                    HirBinaryOp::GreaterEq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a >= b)))
                        }
                        (Value::UInt8(a), Value::UInt8(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a >= b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a >= b)))
                        }
                        _ => Err(">= operator mismatch".into()),
                    },
                    HirBinaryOp::And => match (left, right) {
                        (Value::Bool(a), Value::Bool(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a && b)))
                        }
                        _ => Err("&& requires Bool".into()),
                    },
                    HirBinaryOp::Or => match (left, right) {
                        (Value::Bool(a), Value::Bool(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a || b)))
                        }
                        _ => Err("|| requires Bool".into()),
                    },
                    _ => Err("Unsupported binary operator".into()),
                }
            }
            HirExpression::IsType(is_expr) => {
                let value = match self.eval_expression(&is_expr.value, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                Ok(ControlFlow::Value(Value::Bool(Self::value_is_type(
                    &value,
                    &is_expr.ty,
                ))))
            }
            HirExpression::IsPattern(is_expr) => {
                let value = match self.eval_expression(&is_expr.value, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let mut bindings = Vec::new();
                let is_match = Self::value_matches_pattern(&value, &is_expr.pattern, &mut bindings);
                Ok(ControlFlow::Value(Value::Bool(is_match)))
            }
            HirExpression::AsOptional(expr) => {
                let value = match self.eval_expression(&expr.expr, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                if Self::value_is_type(&value, &expr.ty) {
                    Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                        Box::new(value),
                    ))))
                } else {
                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                }
            }
            HirExpression::AsForced(expr) => {
                let value = match self.eval_expression(&expr.expr, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                if Self::value_is_type(&value, &expr.ty) {
                    Ok(ControlFlow::Value(value))
                } else {
                    Ok(ControlFlow::Crash(format!(
                        "invalid cast: expected {}, got {}",
                        format_hir_type(&expr.ty),
                        value.type_name()
                    )))
                }
            }
            HirExpression::Call(call) => {
                if let HirExpression::Member(member) = &*call.callee {
                    if let Some((identity, function)) =
                        self.resolve_module_member_function(member, env)
                    {
                        let mut positional_values = Vec::new();
                        let mut named_values = HashMap::new();
                        for argument in &call.args {
                            match argument {
                                HirCallArgument::Positional(expression) => {
                                    match self.eval_expression(expression, env)? {
                                        ControlFlow::Value(value) => positional_values.push(value),
                                        flow => return Ok(flow),
                                    }
                                }
                                HirCallArgument::Named(named) => {
                                    match self.eval_expression(&named.value, env)? {
                                        ControlFlow::Value(value) => {
                                            named_values.insert(named.name.clone(), value);
                                        }
                                        flow => return Ok(flow),
                                    }
                                }
                            }
                        }
                        return self.bind_and_call_user_function(
                            call,
                            &identity,
                            &function,
                            positional_values,
                            named_values,
                        );
                    }
                    if let HirExpression::Name(_, ref obj_name) = *member.object {
                        if obj_name == "fs" {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err(format!(
                                    "operation 'fs.{}' is not allowed during comptime evaluation",
                                    member.field
                                ));
                            }
                            let mut args = Vec::new();
                            let mut named_args = HashMap::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(n) => {
                                        match self.eval_expression(&n.value, env)? {
                                            ControlFlow::Value(v) => {
                                                named_args.insert(n.name.clone(), v);
                                            }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_fs_builtin(&member.field, &args, &named_args);
                        }
                        if obj_name == "env" {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err(format!(
                                    "operation 'env.{}' is not allowed during comptime evaluation",
                                    member.field
                                ));
                            }
                            let mut args = Vec::new();
                            let mut named_args = HashMap::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(n) => {
                                        match self.eval_expression(&n.value, env)? {
                                            ControlFlow::Value(v) => {
                                                named_args.insert(n.name.clone(), v);
                                            }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_env_builtin(&member.field, &args, &named_args);
                        }
                        if obj_name == "os" {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err(format!(
                                    "operation 'os.{}' is not allowed during comptime evaluation",
                                    member.field
                                ));
                            }
                            let mut args = Vec::new();
                            let mut named_args = HashMap::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(n) => {
                                        match self.eval_expression(&n.value, env)? {
                                            ControlFlow::Value(v) => {
                                                named_args.insert(n.name.clone(), v);
                                            }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_os_builtin(&member.field, &args, &named_args);
                        }
                        if obj_name == "Regex" {
                            let mut args = Vec::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(_) => {
                                        return Err(
                                            "Regex constructors only support positional arguments"
                                                .into(),
                                        );
                                    }
                                }
                            }
                            let re = Self::eval_regex_constructor(&member.field, &args)?;
                            return Ok(ControlFlow::Value(Value::Regex(re)));
                        }
                    }
                    let obj = match self.eval_expression(&member.object, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    return match obj {
                        Value::String(s) => {
                            let mut args = Vec::new();
                            let mut named_args = HashMap::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(n) => {
                                        match self.eval_expression(&n.value, env)? {
                                            ControlFlow::Value(v) => {
                                                named_args.insert(n.name.clone(), v);
                                            }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self
                                .eval_string_method(&s, &member.field, &args, &named_args)
                                .map(ControlFlow::Value);
                        }
                        Value::Array(arr) => {
                            if member.field == "len" {
                                if !call.args.is_empty() {
                                    return Err("Method 'len' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::Int64(
                                    arr.borrow().len() as i64
                                )));
                            }
                            if member.field == "is_empty" {
                                if !call.args.is_empty() {
                                    return Err("Method 'is_empty' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::Bool(
                                    arr.borrow().is_empty(),
                                )));
                            }
                            if member.field == "iter" {
                                if !call.args.is_empty() {
                                    return Err("Method 'iter' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::ArrayIter(
                                    crate::value::ArrayIterValue {
                                        array: arr.clone(),
                                        index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                                    },
                                )));
                            }
                            if member.field == "first" {
                                if !call.args.is_empty() {
                                    return Err("Method 'first' expects 0 arguments".into());
                                }
                                let borrowed = arr.borrow();
                                return if borrowed.is_empty() {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                                } else {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                                        Box::new(borrowed[0].clone()),
                                    ))))
                                };
                            }
                            if member.field == "last" {
                                if !call.args.is_empty() {
                                    return Err("Method 'last' expects 0 arguments".into());
                                }
                                let borrowed = arr.borrow();
                                return if borrowed.is_empty() {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                                } else {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                                        Box::new(borrowed[borrowed.len() - 1].clone()),
                                    ))))
                                };
                            }
                            return Err(format!("Unknown method '{}' on Array", member.field));
                        }
                        Value::Slice(slice) => {
                            if member.field == "len" {
                                if !call.args.is_empty() {
                                    return Err("Method 'len' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::Int64(slice.len() as i64)));
                            }
                            if member.field == "is_empty" {
                                if !call.args.is_empty() {
                                    return Err("Method 'is_empty' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::Bool(slice.len() == 0)));
                            }
                            if member.field == "iter" {
                                if !call.args.is_empty() {
                                    return Err("Method 'iter' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::SliceIter(
                                    crate::value::SliceIterValue {
                                        slice: slice.clone(),
                                        index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                                    },
                                )));
                            }
                            if member.field == "first" {
                                if !call.args.is_empty() {
                                    return Err("Method 'first' expects 0 arguments".into());
                                }
                                return if slice.len() == 0 {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                                } else {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                                        Box::new(slice.get(0).unwrap()),
                                    ))))
                                };
                            }
                            if member.field == "last" {
                                if !call.args.is_empty() {
                                    return Err("Method 'last' expects 0 arguments".into());
                                }
                                return if slice.len() == 0 {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::None)))
                                } else {
                                    Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                                        Box::new(slice.get(slice.len() - 1).unwrap()),
                                    ))))
                                };
                            }
                            return Err(format!("Unknown method '{}' on Slice", member.field));
                        }
                        Value::ArrayIter(iter) => {
                            if member.field == "next" {
                                if !call.args.is_empty() {
                                    return Err("Method 'next' expects 0 arguments".into());
                                }
                                let mut idx = iter.index.borrow_mut();
                                let arr = iter.array.borrow();
                                if *idx < arr.len() {
                                    let val = arr[*idx].clone();
                                    *idx += 1;
                                    return Ok(ControlFlow::Value(Value::Option(
                                        crate::value::OptionValue::Some(Box::new(val)),
                                    )));
                                } else {
                                    return Ok(ControlFlow::Value(Value::Option(
                                        crate::value::OptionValue::None,
                                    )));
                                }
                            }
                            return Err(format!("Unknown method '{}' on ArrayIter", member.field));
                        }
                        Value::SliceIter(iter) => {
                            if member.field == "next" {
                                if !call.args.is_empty() {
                                    return Err("Method 'next' expects 0 arguments".into());
                                }
                                let mut idx = iter.index.borrow_mut();
                                if *idx < iter.slice.len() {
                                    let val = iter.slice.get(*idx).unwrap();
                                    *idx += 1;
                                    return Ok(ControlFlow::Value(Value::Option(
                                        crate::value::OptionValue::Some(Box::new(val)),
                                    )));
                                } else {
                                    return Ok(ControlFlow::Value(Value::Option(
                                        crate::value::OptionValue::None,
                                    )));
                                }
                            }
                            return Err(format!("Unknown method '{}' on SliceIter", member.field));
                        }
                        Value::Command(cmd) => {
                            if member.field != "run" {
                                return Err(format!(
                                    "Unknown method '{}' on Command",
                                    member.field
                                ));
                            }
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err(
                                    "command execution is not allowed during comptime evaluation"
                                        .into(),
                                );
                            }
                            return run_command(&cmd).map(ControlFlow::Value);
                        }
                        Value::Regex(re) => {
                            let mut args = Vec::new();
                            for arg in &call.args {
                                match arg {
                                    HirCallArgument::Positional(arg_expr) => {
                                        match self.eval_expression(arg_expr, env)? {
                                            ControlFlow::Value(v) => args.push(v),
                                            cf => return Ok(cf),
                                        }
                                    }
                                    HirCallArgument::Named(_) => {
                                        return Err(
                                            "Regex methods only support positional arguments"
                                                .into(),
                                        );
                                    }
                                }
                            }
                            return Self::eval_regex_method(&re, &member.field, &args)
                                .map(ControlFlow::Value);
                        }
                        Value::Struct(s) => {
                            if let Some(Value::Closure(c)) = s.fields.get(&member.field) {
                                let mut args = Vec::new();
                                for arg in &call.args {
                                    match arg {
                                        HirCallArgument::Positional(arg_expr) => {
                                            match self.eval_expression(arg_expr, env)? {
                                                ControlFlow::Value(v) => args.push(v),
                                                cf => return Ok(cf),
                                            }
                                        }
                                        HirCallArgument::Named(_) => {
                                            return Err(
                                                "Closure fields only support positional arguments"
                                                    .into(),
                                            );
                                        }
                                    }
                                }
                                return self.call_closure(c, args);
                            }

                            // Phase 6: Check for methods (e.g. TypeName.methodName)
                            let method_name = format!("{}.{}", s.name, member.field);
                            if let Some((identity, function)) =
                                self.resolve_user_function(&method_name)
                            {
                                let mut args = Vec::new();
                                args.push(Value::Struct(s.clone())); // Pass self
                                for arg in &call.args {
                                    match arg {
                                        HirCallArgument::Positional(arg_expr) => {
                                            match self.eval_expression(arg_expr, env)? {
                                                ControlFlow::Value(v) => args.push(v),
                                                cf => return Ok(cf),
                                            }
                                        }
                                        HirCallArgument::Named(n) => {
                                            return Err(format!(
                                                "Named argument '{}' not supported for methods yet",
                                                n.name
                                            ));
                                        }
                                    }
                                }
                                return self
                                    .call_resolved_function(&identity, &function, None, args);
                            }

                            return Err(format!(
                                "Field '{}' is not a closure and no method '{}' found",
                                member.field, method_name
                            ));
                        }
                        _ => Err(format!(
                            "Type {} has no method '{}'",
                            obj.type_name(),
                            member.field
                        )),
                    };
                }

                let mut positional_values = Vec::new();
                let mut named_values = HashMap::new();
                for arg in &call.args {
                    match arg {
                        HirCallArgument::Positional(e) => match self.eval_expression(e, env)? {
                            ControlFlow::Value(v) => positional_values.push(v),
                            cf => return Ok(cf),
                        },
                        HirCallArgument::Named(n) => match self.eval_expression(&n.value, env)? {
                            ControlFlow::Value(v) => {
                                named_values.insert(n.name.clone(), v);
                            }
                            cf => return Ok(cf),
                        },
                    }
                }

                if let HirExpression::Name(_, name) = &*call.callee {
                    // Builtins
                    match name.as_str() {
                        "compile_error" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("message"))
                                .ok_or("Missing argument 'message' for compile_error")?;
                            if let Value::String(message) = val {
                                return Err(message.clone());
                            }
                            return Err("compile_error requires a String argument".into());
                        }
                        "exit" => {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err("exit is not allowed during comptime evaluation".into());
                            }
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("code"))
                                .ok_or("Missing argument 'code' for exit")?;
                            if let Value::Int64(code) = val {
                                return Ok(ControlFlow::Exit(*code as i32));
                            } else {
                                return Err("exit requires an Int64 argument".into());
                            }
                        }
                        "print" => {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err(
                                    "operation 'print' is not allowed during comptime evaluation"
                                        .into(),
                                );
                            }
                            let mut s = String::new();
                            let mut newline = true;
                            for val in positional_values.iter() {
                                s.push_str(&value_to_string(val));
                            }
                            if let Some(v) = named_values.get("newline") {
                                if let Value::Bool(b) = v {
                                    newline = *b;
                                }
                            }
                            if newline {
                                println!("{}", s);
                            } else {
                                print!("{}", s);
                                use std::io::Write;
                                let _ = std::io::stdout().flush();
                            }
                            return Ok(ControlFlow::Value(Value::Unit));
                        }
                        "crash" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("message"))
                                .ok_or("Missing argument 'message' for crash")?;
                            return Ok(ControlFlow::Crash(value_to_string(val)));
                        }
                        "fmt" => {
                            let mut separator = "";
                            if let Some(Value::String(sep)) = named_values.get("separator") {
                                separator = sep;
                            }
                            let s = positional_values
                                .iter()
                                .map(|v| value_to_string(v))
                                .collect::<Vec<_>>()
                                .join(separator);
                            return Ok(ControlFlow::Value(Value::String(s)));
                        }
                        "read_line" => {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err("operation 'read_line' is not allowed during comptime evaluation".into());
                            }
                            let mut trim_newline = true;
                            if let Some(Value::Bool(b)) = named_values.get("trim_newline") {
                                trim_newline = *b;
                            }

                            let mut input = String::new();
                            return match std::io::stdin().read_line(&mut input) {
                                Ok(_) => {
                                    if trim_newline {
                                        if input.ends_with('\n') {
                                            input.pop();
                                            if input.ends_with('\r') {
                                                input.pop();
                                            }
                                        }
                                    }
                                    Ok(ControlFlow::Value(Value::Result(Box::new(
                                        crate::value::ValueResult::Ok(Value::String(input)),
                                    ))))
                                }
                                Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                    crate::value::ValueResult::Err(Value::Error(format!(
                                        "Failed to read stdin: {}",
                                        e
                                    ))),
                                )))),
                            };
                        }
                        "cmd" => {
                            if self.mode == crate::interpreter::EvalMode::Comptime {
                                return Err("command construction is not allowed during comptime evaluation".into());
                            }
                            let program = match positional_values
                                .get(0)
                                .or_else(|| named_values.get("program"))
                            {
                                Some(Value::String(s)) => s.clone(),
                                _ => return Err("cmd requires a String program name".into()),
                            };

                            let mut cmd_args = Vec::new();
                            // If second arg is an array, use it as args?
                            // No, prompt says cmd("grep", "hello", "file.txt").
                            // So positional args 1..N are command args.
                            for v in positional_values.iter().skip(1) {
                                cmd_args.push(value_to_string(v));
                            }

                            let mut cwd = None;
                            if let Some(Value::String(s)) = named_values.get("cwd") {
                                cwd = Some(s.clone());
                            }
                            let mut check = false;
                            if let Some(Value::Bool(b)) = named_values.get("check") {
                                check = *b;
                            }

                            return Ok(ControlFlow::Value(Value::Command(
                                crate::value::CommandValue {
                                    program,
                                    args: cmd_args,
                                    cwd,
                                    check,
                                },
                            )));
                        }
                        "Ok" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("value"))
                                .ok_or("Missing argument for Ok")?;
                            return Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Ok(val.clone()),
                            ))));
                        }
                        "Err" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("error"))
                                .ok_or("Missing argument for Err")?;
                            return Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(val.clone()),
                            ))));
                        }
                        "Some" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("value"))
                                .ok_or("Missing argument for Some")?;
                            return Ok(ControlFlow::Value(Value::Option(OptionValue::Some(
                                Box::new(val.clone()),
                            ))));
                        }
                        "expect" => {
                            let cond_val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("condition"))
                                .ok_or("Missing argument 'condition' for expect")?;
                            let msg_val = positional_values
                                .get(1)
                                .or_else(|| named_values.get("message"));

                            if let Value::Bool(b) = cond_val {
                                if *b {
                                    return Ok(ControlFlow::Value(Value::Unit));
                                } else {
                                    let msg = if let Some(m) = msg_val {
                                        value_to_string(m)
                                    } else {
                                        "expectation failed".to_string()
                                    };
                                    return Ok(ControlFlow::Crash(msg));
                                }
                            } else {
                                return Err("expect requires a Bool as first argument".into());
                            }
                        }
                        "dbg" => {
                            let val = positional_values
                                .get(0)
                                .or_else(|| named_values.get("value"))
                                .ok_or("Missing argument for dbg")?;
                            let mut label = "dbg";
                            if let Some(Value::String(s)) = named_values.get("label") {
                                label = s;
                            }
                            // dbg! style: show quotes for strings, readable structs
                            println!("{}: {}", label, val.to_repr());
                            return Ok(ControlFlow::Value(val.clone()));
                        }
                        _ => {}
                    }
                    // Check env for local closure
                    if let Some(Value::Closure(c)) = env.get(name) {
                        return self.call_closure(&c, positional_values);
                    }

                    // User function
                    let (identity, function) = self
                        .resolve_user_function(name)
                        .ok_or_else(|| format!("Function {} not found", name))?;
                    return self.bind_and_call_user_function(
                        call,
                        &identity,
                        &function,
                        positional_values,
                        named_values,
                    );
                } else {
                    // Try evaluating callee as a complex expression (e.g. (fn(x){x})(1))
                    let callee_val = match self.eval_expression(&call.callee, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if let Value::Closure(c) = callee_val {
                        return self.call_closure(&c, positional_values);
                    }
                    Err(format!(
                        "Value of type {} is not callable",
                        callee_val.type_name()
                    ))
                }
            }
            HirExpression::Member(member) => {
                let obj = match self.eval_expression(&member.object, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                match obj {
                    Value::Struct(s) => s
                        .fields
                        .get(&member.field)
                        .cloned()
                        .map(ControlFlow::Value)
                        .ok_or_else(|| format!("Field {} not found", member.field)),
                    Value::CommandResult(r) => match member.field.as_str() {
                        "exit_code" => Ok(ControlFlow::Value(Value::Int64(r.exit_code as i64))),
                        "stdout" => Ok(ControlFlow::Value(Value::String(r.stdout.clone()))),
                        "stderr" => Ok(ControlFlow::Value(Value::String(r.stderr.clone()))),
                        "success" => Ok(ControlFlow::Value(Value::Bool(r.success))),
                        _ => Err(format!("Field {} not found on CommandResult", member.field)),
                    },
                    Value::Command(c) => match member.field.as_str() {
                        "program" => Ok(ControlFlow::Value(Value::String(c.program.clone()))),
                        "args" => Ok(ControlFlow::Value(Value::Array(std::rc::Rc::new(
                            std::cell::RefCell::new(
                                c.args.iter().map(|s| Value::String(s.clone())).collect(),
                            ),
                        )))),
                        _ => Err(format!("Field {} not found on Command", member.field)),
                    },
                    _ => Err("Member access requires a struct, Command, or CommandResult".into()),
                }
            }
            HirExpression::Index(index) => {
                let obj = match self.eval_expression(&index.object, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let idx_val = match self.eval_expression(&index.index, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                match (&obj, &idx_val) {
                    (Value::Array(arr), Value::Int64(i)) => {
                        let arr_ref = arr.borrow();
                        arr_ref
                            .get(*i as usize)
                            .cloned()
                            .ok_or_else(|| format!("Index out of bounds: {}", i))
                            .map(ControlFlow::Value)
                    }
                    (Value::Slice(slice), Value::Int64(i)) => slice
                        .get(*i as usize)
                        .ok_or_else(|| format!("Index out of bounds: {}", i))
                        .map(ControlFlow::Value),
                    (Value::Array(_), _) | (Value::Slice(_), _) => {
                        Err("Index must be Int64".into())
                    }
                    (obj, _idx) => {
                        // Phase 7: User-defined get hook
                        self.eval_method_call(obj.clone(), "get", vec![idx_val.clone()], env)
                    }
                }
            }
            HirExpression::Slice(slice) => {
                let obj = match self.eval_expression(&slice.object, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let start_val = match self.eval_expression(&slice.start, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let end_val = match self.eval_expression(&slice.end, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                let (parent, parent_len, start_offset) = match obj {
                    Value::Array(arr) => {
                        let len = arr.borrow().len();
                        (crate::value::SliceParent::Values(arr.clone()), len, 0)
                    }
                    Value::Slice(s) => (s.parent.clone(), s.len(), s.start),
                    _ => {
                        // Phase 7: User-defined slice hook
                        return self.eval_method_call(obj, "slice", vec![start_val, end_val], env);
                    }
                };

                if let (Value::Int64(start), Value::Int64(end)) = (start_val, end_val) {
                    if start < 0 || end < 0 || start > end || end > parent_len as i64 {
                        return Ok(ControlFlow::Crash(format!(
                            "Slice bounds out of range: [{}..{}] for length {}",
                            start, end, parent_len
                        )));
                    }
                    Ok(ControlFlow::Value(Value::Slice(crate::value::SliceValue {
                        parent,
                        start: start_offset + start as usize,
                        end: start_offset + end as usize,
                    })))
                } else {
                    Err("Slice bounds must be integers".into())
                }
            }
            HirExpression::If(if_expr) => {
                let condition_res;
                let mut bindings = Vec::new();
                let mut is_pattern_match = false;

                if let HirExpression::IsPattern(is_pat) = &*if_expr.condition {
                    is_pattern_match = true;
                    let val = match self.eval_expression(&is_pat.value, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if Self::value_matches_pattern(&val, &is_pat.pattern, &mut bindings) {
                        condition_res = Ok(ControlFlow::Value(Value::Bool(true)));
                    } else {
                        condition_res = Ok(ControlFlow::Value(Value::Bool(false)));
                    }
                } else {
                    condition_res = self.eval_expression(&if_expr.condition, env);
                }

                let cond = match condition_res? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let is_true = match cond {
                    Value::Bool(b) => b,
                    _ => return Err("If condition must be a Bool".into()),
                };

                if is_true {
                    if is_pattern_match && !bindings.is_empty() {
                        env.push_scope();
                        for (name, val) in bindings {
                            env.insert(name, val);
                        }
                        let res = self.eval_block(&if_expr.then_block, env);
                        env.pop_scope();
                        res
                    } else {
                        self.eval_block(&if_expr.then_block, env)
                    }
                } else if let Some(else_branch) = &if_expr.else_branch {
                    self.eval_expression(else_branch, env)
                } else {
                    Ok(ControlFlow::Value(Value::Unit))
                }
            }
            HirExpression::Block(block) => self.eval_block(block, env),
            HirExpression::Tuple(_, exprs) => {
                let mut vals = Vec::new();
                for e in exprs {
                    match self.eval_expression(e, env)? {
                        ControlFlow::Value(v) => vals.push(v),
                        cf => return Ok(cf),
                    }
                }
                Ok(ControlFlow::Value(Value::Tuple(vals)))
            }
            HirExpression::Array(_, exprs) => {
                let mut vals = Vec::new();
                for e in exprs {
                    match self.eval_expression(e, env)? {
                        ControlFlow::Value(v) => vals.push(v),
                        cf => return Ok(cf),
                    }
                }
                Ok(ControlFlow::Value(Value::Array(std::rc::Rc::new(
                    std::cell::RefCell::new(vals),
                ))))
            }
            HirExpression::StructLiteral(s) => {
                let mut fields = HashMap::new();
                for (name, expr) in &s.fields {
                    match self.eval_expression(expr, env)? {
                        ControlFlow::Value(v) => {
                            fields.insert(name.clone(), v);
                        }
                        cf => return Ok(cf),
                    }
                }

                let struct_name = Self::get_type_name(&s.ty);
                // Fill defaults
                if let Some(def) = self.resolve_struct(&struct_name) {
                    for field in &def.fields {
                        if !fields.contains_key(&field.name) {
                            if let Some(default_expr) = &field.default_value {
                                // Evaluate default expression in the CURRENT environment
                                match self.eval_expression(default_expr, env)? {
                                    ControlFlow::Value(v) => {
                                        fields.insert(field.name.clone(), v);
                                    }
                                    cf => return Ok(cf),
                                }
                            }
                        }
                    }
                }

                Ok(ControlFlow::Value(Value::Struct(StructInstance {
                    name: struct_name,
                    fields,
                })))
            }
            HirExpression::For(for_expr) => {
                let iterable = match self.eval_expression(&for_expr.iterable, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                // Obtain an iterator value from the iterable via the Iterable protocol.
                // String is a built-in special case: iterate chars as String values.
                // All other types must support iter() → iterator that supports next().
                let mut iterator: Option<Value> = None;
                let char_values: Option<Vec<Value>> = match &iterable {
                    Value::String(s) => {
                        Some(s.chars().map(|c| Value::String(c.to_string())).collect())
                    }
                    Value::Array(arr) => {
                        iterator = Some(Value::ArrayIter(crate::value::ArrayIterValue {
                            array: arr.clone(),
                            index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                        }));
                        None
                    }
                    Value::Slice(slice) => {
                        iterator = Some(Value::SliceIter(crate::value::SliceIterValue {
                            slice: slice.clone(),
                            index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                        }));
                        None
                    }
                    Value::Struct(s) => {
                        // Custom Iterable: call iter() method
                        let method_name = format!("{}.iter", s.name);
                        if let Some((identity, function)) = self.resolve_user_function(&method_name)
                        {
                            let iter_val = match self.call_resolved_function(
                                &identity,
                                &function,
                                None,
                                vec![iterable.clone()],
                            )? {
                                ControlFlow::Value(v) => v,
                                cf => return Ok(cf),
                            };
                            iterator = Some(iter_val);
                        } else {
                            return Err(format!(
                                "type '{}' does not implement Iterable (no 'iter' method)",
                                s.name
                            ));
                        }
                        None
                    }
                    other => {
                        return Err(format!(
                            "value of type '{}' is not iterable",
                            other.type_name()
                        ));
                    }
                };

                // Helper: collect items via next() on an iterator Value
                let mut collected: Vec<Value> = Vec::new();

                if let Some(char_vals) = char_values {
                    // String fast path: already have the values
                    for item in char_vals {
                        env.push_scope();
                        env.insert(for_expr.binding.clone(), item);

                        if let Some(filter_expr) = &for_expr.filter {
                            let filter_res = match self.eval_expression(filter_expr, env)? {
                                ControlFlow::Value(v) => v,
                                cf => {
                                    env.pop_scope();
                                    return Ok(cf);
                                }
                            };
                            if let Value::Bool(b) = filter_res {
                                if !b {
                                    env.pop_scope();
                                    continue;
                                }
                            } else {
                                env.pop_scope();
                                return Err("for filter must be Bool".into());
                            }
                        }

                        match self.eval_block(&for_expr.body, env)? {
                            ControlFlow::Value(v) => {
                                collected.push(v);
                            }
                            ControlFlow::Return(v) => {
                                env.pop_scope();
                                return Ok(ControlFlow::Return(v));
                            }
                            ControlFlow::Exit(code) => {
                                env.pop_scope();
                                return Ok(ControlFlow::Exit(code));
                            }
                            ControlFlow::Crash(msg) => {
                                env.pop_scope();
                                return Ok(ControlFlow::Crash(msg));
                            }
                            ControlFlow::Break => {
                                env.pop_scope();
                                break;
                            }
                            ControlFlow::Continue => {
                                env.pop_scope();
                                continue;
                            }
                        }
                        env.pop_scope();
                    }
                } else if let Some(mut iter_val) = iterator {
                    // Protocol path: repeatedly call next() until None
                    loop {
                        // Call next() on the iterator
                        let next_result = match &iter_val {
                            Value::ArrayIter(iter) => {
                                let mut idx = iter.index.borrow_mut();
                                let arr = iter.array.borrow();
                                if *idx < arr.len() {
                                    let val = arr[*idx].clone();
                                    *idx += 1;
                                    Value::Option(crate::value::OptionValue::Some(Box::new(val)))
                                } else {
                                    Value::Option(crate::value::OptionValue::None)
                                }
                            }
                            Value::SliceIter(iter) => {
                                let mut idx = iter.index.borrow_mut();
                                if *idx < iter.slice.len() {
                                    let val = iter.slice.get(*idx).unwrap();
                                    *idx += 1;
                                    Value::Option(crate::value::OptionValue::Some(Box::new(val)))
                                } else {
                                    Value::Option(crate::value::OptionValue::None)
                                }
                            }
                            Value::Struct(s) => {
                                // Custom Iterator: call next(inout self)
                                // Use call_function_with_env to recover the mutated self from the env.
                                let method_name = format!("{}.next", s.name);
                                if let Some((identity, function)) =
                                    self.resolve_user_function(&method_name)
                                {
                                    let self_param_name = function
                                        .parameters
                                        .first()
                                        .map(|p| p.name.clone())
                                        .unwrap_or_else(|| "self".to_string());
                                    let (cf, post_env) = self.call_resolved_function_with_env(
                                        &identity,
                                        &function,
                                        None,
                                        vec![iter_val.clone()],
                                    )?;
                                    // Update iter_val to reflect mutated self
                                    if let Some(updated_self) = post_env.get(&self_param_name) {
                                        iter_val = updated_self.clone();
                                    }
                                    match cf {
                                        ControlFlow::Value(v) => v,
                                        other => return Ok(other),
                                    }
                                } else {
                                    return Err(format!(
                                        "iterator type '{}' has no 'next' method",
                                        s.name
                                    ));
                                }
                            }
                            other => {
                                return Err(format!(
                                    "value of type '{}' is not an iterator",
                                    other.type_name()
                                ));
                            }
                        };

                        match next_result {
                            Value::Option(crate::value::OptionValue::None) => break,
                            Value::Option(crate::value::OptionValue::Some(item)) => {
                                env.push_scope();
                                env.insert(for_expr.binding.clone(), *item);

                                if let Some(filter_expr) = &for_expr.filter {
                                    let filter_res = match self.eval_expression(filter_expr, env)? {
                                        ControlFlow::Value(v) => v,
                                        cf => {
                                            env.pop_scope();
                                            return Ok(cf);
                                        }
                                    };
                                    if let Value::Bool(b) = filter_res {
                                        if !b {
                                            env.pop_scope();
                                            continue;
                                        }
                                    } else {
                                        env.pop_scope();
                                        return Err("for filter must be Bool".into());
                                    }
                                }

                                match self.eval_block(&for_expr.body, env)? {
                                    ControlFlow::Value(v) => {
                                        collected.push(v);
                                    }
                                    ControlFlow::Return(v) => {
                                        env.pop_scope();
                                        return Ok(ControlFlow::Return(v));
                                    }
                                    ControlFlow::Exit(code) => {
                                        env.pop_scope();
                                        return Ok(ControlFlow::Exit(code));
                                    }
                                    ControlFlow::Crash(msg) => {
                                        env.pop_scope();
                                        return Ok(ControlFlow::Crash(msg));
                                    }
                                    ControlFlow::Break => {
                                        env.pop_scope();
                                        break;
                                    }
                                    ControlFlow::Continue => {
                                        env.pop_scope();
                                        continue;
                                    }
                                }
                                env.pop_scope();
                            }
                            other => {
                                return Err(format!(
                                    "next() returned unexpected value: {:?}",
                                    other
                                ));
                            }
                        }
                    }
                }

                Ok(ControlFlow::Value(Value::Array(std::rc::Rc::new(
                    std::cell::RefCell::new(collected),
                ))))
            }
            // Minimal implementations for others to make it compile
            HirExpression::Try(e) => {
                let val = match self.eval_expression(&e.expr, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                match val {
                    Value::Result(res) => match res.as_ref() {
                        crate::value::ValueResult::Ok(v) => Ok(ControlFlow::Value(v.clone())),
                        crate::value::ValueResult::Err(v) => Ok(ControlFlow::Return(
                            Value::Result(Box::new(crate::value::ValueResult::Err(v.clone()))),
                        )),
                    },
                    _ => Err("'?' expects a Result value".into()),
                }
            }
            HirExpression::Unwrap(e) => {
                let val = match self.eval_expression(&e.expr, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                match val {
                    Value::Result(res) => match res.as_ref() {
                        crate::value::ValueResult::Ok(v) => Ok(ControlFlow::Value(v.clone())),
                        crate::value::ValueResult::Err(v) => {
                            Ok(ControlFlow::Crash(format!("Unwrap failed: {}", v)))
                        }
                    },
                    _ => Err("'!' expects a Result value".into()),
                }
            }
            HirExpression::Match(match_expr) => {
                let value = match self.eval_expression(&match_expr.value, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                for arm in &match_expr.arms {
                    let mut bindings = Vec::new();
                    if Self::value_matches_pattern(&value, &arm.pattern, &mut bindings) {
                        env.push_scope();
                        for (name, val) in bindings {
                            env.insert(name, val);
                        }

                        let mut guard_passed = true;
                        if let Some(guard) = &arm.guard {
                            let guard_result = match self.eval_expression(guard, env)? {
                                ControlFlow::Value(Value::Bool(b)) => b,
                                ControlFlow::Value(_) => {
                                    unreachable!("typechecker ensures guard is Bool")
                                }
                                cf => {
                                    env.pop_scope();
                                    return Ok(cf);
                                }
                            };
                            guard_passed = guard_result;
                        }

                        if guard_passed {
                            let res = self.eval_expression(&arm.value, env);
                            env.pop_scope();
                            return res;
                        } else {
                            env.pop_scope(); // guard failed, continue to next arm
                        }
                    }
                }

                Ok(ControlFlow::Crash(format!(
                    "non-exhaustive match: no arm matched value {}",
                    value.to_repr()
                )))
            }
            HirExpression::Closure(c) => {
                let mut captures = HashMap::new();
                let mut self_cell = None;
                for cap in &c.captures {
                    match cap {
                        HirCapture::Named(named) => {
                            let val = match named.mode {
                                HirCaptureMode::Inout => env.get_raw(&named.name),
                                _ => env.get(&named.name),
                            }
                            .ok_or_else(|| {
                                format!(
                                    "Capture variable '{}' not found in environment",
                                    named.name
                                )
                            })?;
                            captures.insert(named.name.clone(), val);
                        }
                        HirCapture::RecursiveSelf(_) => {
                            let cell =
                                Value::Cell(std::rc::Rc::new(std::cell::RefCell::new(Value::Unit)));
                            captures.insert("self".into(), cell.clone());
                            self_cell = Some(cell);
                        }
                    }
                }
                let closure = std::rc::Rc::new(crate::value::ClosureValue {
                    params: c.parameters.clone(),
                    body: c.body.clone(),
                    captures,
                    is_recursive: self_cell.is_some(),
                });
                let val = Value::Closure(closure.clone());
                if let Some(Value::Cell(inner)) = self_cell {
                    *inner.borrow_mut() = val.clone();
                }
                Ok(ControlFlow::Value(val))
            }
            HirExpression::Comptime(e) => {
                if self.mode == crate::interpreter::EvalMode::Comptime {
                    self.eval_expression(&e.expression, env)
                } else {
                    self.evaluate_comptime(
                        &e.expression,
                        env,
                        crate::comptime::ComptimeEvalConfig::default(),
                    )
                    .map(ControlFlow::Value)
                    .map_err(|error| error.to_string())
                }
            }
        }
    }

    fn value_matches_pattern(
        value: &Value,
        pattern: &HirPattern,
        bindings: &mut Vec<(String, Value)>,
    ) -> bool {
        match pattern {
            HirPattern::Wildcard(_) => true,
            HirPattern::Identifier(_, name) => {
                bindings.push((name.clone(), value.clone()));
                true
            }
            HirPattern::EnumCase(_, name, args) => {
                // 1. User-defined Enum
                if let Value::Enum(e) = value {
                    if &e.variant_name == name {
                        if args.len() == e.payloads.len() {
                            let mut temp_bindings = Vec::new();
                            for (arg_pat, arg_val) in args.iter().zip(e.payloads.iter()) {
                                if !Self::value_matches_pattern(
                                    arg_val,
                                    arg_pat,
                                    &mut temp_bindings,
                                ) {
                                    return false;
                                }
                            }
                            bindings.extend(temp_bindings);
                            return true;
                        }
                    }
                    if args.is_empty() && &e.enum_name == name {
                        return true;
                    }
                }

                if let Value::Type(ty) = value {
                    if args.is_empty() {
                        let pat_ty = tempra_hir::types::resolve_named_type(name);
                        if pat_ty == *ty {
                            return true;
                        }
                    }
                }

                match name.as_str() {
                    "Some" => {
                        if let Value::Option(crate::value::OptionValue::Some(inner)) = value {
                            if args.len() == 1 {
                                return Self::value_matches_pattern(inner, &args[0], bindings);
                            }
                        }
                        false
                    }
                    "None" => {
                        args.is_empty()
                            && matches!(value, Value::Option(crate::value::OptionValue::None))
                    }
                    "Ok" => {
                        if let Value::Result(res) = value {
                            if let crate::value::ValueResult::Ok(inner) = res.as_ref() {
                                if args.len() == 1 {
                                    return Self::value_matches_pattern(inner, &args[0], bindings);
                                }
                            }
                        }
                        false
                    }
                    "Err" => {
                        if let Value::Result(res) = value {
                            if let crate::value::ValueResult::Err(inner) = res.as_ref() {
                                if args.len() == 1 {
                                    return Self::value_matches_pattern(inner, &args[0], bindings);
                                }
                            }
                        }
                        false
                    }
                    _ => {
                        if args.is_empty() {
                            // Fallback for "x is Int" cases parsed as patterns:
                            // Check if the constructor name matches the type name of the value.
                            match (name.as_str(), value) {
                                ("Unit", Value::Unit) => true,
                                ("Bool", Value::Bool(_)) => true,
                                ("String", Value::String(_)) => true,
                                ("Int" | "Int64", Value::Int64(_)) => true,
                                ("UInt8", Value::UInt8(_)) => true,
                                ("Float32" | "Float64", Value::Float64(_)) => true,
                                ("Error", Value::Error(_)) => true,
                                _ => false,
                            }
                        } else {
                            false
                        }
                    }
                }
            }
            HirPattern::Literal(_, lit_expr) => {
                // Evaluate the literal and compare by value equality.
                let mut _dummy_bindings = Vec::new();
                match lit_expr.as_ref() {
                    HirExpression::Integer(_, s) => {
                        if let Ok(n) = s.parse::<i64>() {
                            return matches!(value, Value::Int64(v) if *v == n);
                        }
                        false
                    }
                    HirExpression::Float(_, s) => {
                        if let Ok(f) = s.parse::<f64>() {
                            return matches!(value, Value::Float64(v) if *v == f);
                        }
                        false
                    }
                    HirExpression::String(_, s) => {
                        let s = s.trim_matches('"');
                        matches!(value, Value::String(v) if v == s)
                    }
                    HirExpression::Bool(_, b) => {
                        matches!(value, Value::Bool(v) if *v == *b)
                    }
                    other => Self::value_matches_pattern(
                        value,
                        &HirPattern::Wildcard(other.span()),
                        &mut _dummy_bindings,
                    ),
                }
            }
            HirPattern::Struct(_, name, fields) => {
                if let Value::Struct(struct_val) = value {
                    if struct_val.name == *name {
                        let mut temp_bindings = Vec::new();
                        for (field_name, field_pat) in fields {
                            if let Some(field_value) = struct_val.fields.get(field_name) {
                                if let Some(pat) = field_pat {
                                    if !Self::value_matches_pattern(
                                        field_value,
                                        pat,
                                        &mut temp_bindings,
                                    ) {
                                        return false;
                                    }
                                } else {
                                    temp_bindings.push((field_name.clone(), field_value.clone()));
                                }
                            } else {
                                return false;
                            }
                        }
                        bindings.extend(temp_bindings);
                        return true;
                    }
                }
                false
            }
            HirPattern::Tuple(_, items) => {
                if let Value::Tuple(elements) = value {
                    if elements.len() == items.len() {
                        let mut temp_bindings = Vec::new();
                        for (item_pat, item_val) in items.iter().zip(elements.iter()) {
                            if !Self::value_matches_pattern(item_val, item_pat, &mut temp_bindings)
                            {
                                return false;
                            }
                        }
                        bindings.extend(temp_bindings);
                        return true;
                    }
                }
                false
            }
            HirPattern::EnumConstructor(cons) => {
                if let Value::Enum(e) = value {
                    if e.enum_name == cons.enum_name && e.variant_name == cons.variant_name {
                        if cons.payloads.len() == e.payloads.len() {
                            let mut temp_bindings = Vec::new();
                            for (p_pat, p_val) in cons.payloads.iter().zip(e.payloads.iter()) {
                                if !Self::value_matches_pattern(p_val, p_pat, &mut temp_bindings) {
                                    return false;
                                }
                            }
                            bindings.extend(temp_bindings);
                            return true;
                        }
                    }
                }
                false
            }
            HirPattern::Or(_, alternatives) => {
                for alt in alternatives {
                    let mut temp_bindings = Vec::new();
                    if Self::value_matches_pattern(value, alt, &mut temp_bindings) {
                        bindings.extend(temp_bindings);
                        return true;
                    }
                }
                false
            }
        }
    }

    fn eval_block(
        &mut self,
        block: &HirBlock,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        env.push_scope();
        let mut last_val = Value::Unit;
        for stmt in &block.statements {
            let flow = self.eval_statement(stmt, env)?;
            match flow {
                ControlFlow::Value(v) => last_val = v,
                ControlFlow::Return(v) => {
                    env.pop_scope();
                    return Ok(ControlFlow::Return(v));
                }
                ControlFlow::Exit(code) => {
                    env.pop_scope();
                    return Ok(ControlFlow::Exit(code));
                }
                ControlFlow::Crash(msg) => {
                    env.pop_scope();
                    return Ok(ControlFlow::Crash(msg));
                }
                ControlFlow::Break => {
                    env.pop_scope();
                    return Ok(ControlFlow::Break);
                }
                ControlFlow::Continue => {
                    env.pop_scope();
                    return Ok(ControlFlow::Continue);
                }
            }
        }
        env.pop_scope();
        Ok(ControlFlow::Value(last_val))
    }

    fn eval_fs_builtin(
        &mut self,
        name: &str,
        positional_args: &[Value],
        named_args: &HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            positional_args
                .get(idx)
                .or_else(|| named_args.get(arg_name))
        };

        match name {
            "read" => {
                let path_val = get_arg(0, "path")
                    .ok_or_else(|| format!("'fs.read' missing argument 'path'"))?;
                if let Value::String(path) = path_val {
                    return match std::fs::read_to_string(path) {
                        Ok(text) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Ok(Value::String(text)),
                        )))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Err(Value::Error(format!(
                                "Failed to read '{}': {}",
                                path, e
                            ))),
                        )))),
                    };
                } else {
                    return Err("'fs.read' requires a String for 'path'".into());
                }
            }
            "write" | "append" => {
                let path_val = get_arg(0, "path")
                    .ok_or_else(|| format!("'fs.{}' missing argument 'path'", name))?;
                let text_val = get_arg(1, "text")
                    .ok_or_else(|| format!("'fs.{}' missing argument 'text'", name))?;

                if let (Value::String(path), Value::String(text)) = (path_val, text_val) {
                    if name == "write" {
                        return match std::fs::write(path, text) {
                            Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Ok(Value::Unit),
                            )))),
                            Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(Value::Error(format!(
                                    "Failed to write to '{}': {}",
                                    path, e
                                ))),
                            )))),
                        };
                    } else {
                        use std::io::Write;
                        let res = (|| -> Result<(), String> {
                            let mut file = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                                .map_err(|e| {
                                    format!("Failed to open '{}' for appending: {}", path, e)
                                })?;
                            file.write_all(text.as_bytes())
                                .map_err(|e| format!("Failed to append to '{}': {}", path, e))?;
                            Ok(())
                        })();
                        return match res {
                            Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Ok(Value::Unit),
                            )))),
                            Err(msg) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(Value::Error(msg)),
                            )))),
                        };
                    }
                } else {
                    return Err(format!("'fs.{}' requires two String arguments", name));
                }
            }
            "exists" => {
                let path_val = get_arg(0, "path")
                    .ok_or_else(|| format!("'fs.exists' missing argument 'path'"))?;
                if let Value::String(path) = path_val {
                    let exists = std::path::Path::new(path).exists();
                    Ok(ControlFlow::Value(Value::Result(Box::new(
                        crate::value::ValueResult::Ok(Value::Bool(exists)),
                    ))))
                } else {
                    return Err("'fs.exists' requires a String for 'path'".into());
                }
            }
            "remove" | "mkdir" | "list_dir" => {
                let path_val = get_arg(0, "path")
                    .ok_or_else(|| format!("'fs.{}' missing argument 'path'", name))?;
                if let Value::String(path) = path_val {
                    match name {
                        "remove" => match std::fs::remove_file(path) {
                            Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Ok(Value::Unit),
                            )))),
                            Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(Value::Error(format!(
                                    "Failed to remove '{}': {}",
                                    path, e
                                ))),
                            )))),
                        },
                        "mkdir" => match std::fs::create_dir(path) {
                            Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Ok(Value::Unit),
                            )))),
                            Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(Value::Error(format!(
                                    "Failed to create directory '{}': {}",
                                    path, e
                                ))),
                            )))),
                        },
                        "list_dir" => match std::fs::read_dir(path) {
                            Ok(entries) => {
                                let mut names = Vec::new();
                                for entry in entries {
                                    if let Ok(entry) = entry {
                                        names.push(Value::String(
                                            entry.file_name().to_string_lossy().to_string(),
                                        ));
                                    }
                                }
                                Ok(ControlFlow::Value(Value::Result(Box::new(
                                    crate::value::ValueResult::Ok(Value::Array(std::rc::Rc::new(
                                        std::cell::RefCell::new(names),
                                    ))),
                                ))))
                            }
                            Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                                crate::value::ValueResult::Err(Value::Error(format!(
                                    "Failed to read directory '{}': {}",
                                    path, e
                                ))),
                            )))),
                        },
                        _ => unreachable!(),
                    }
                } else {
                    return Err(format!("'fs.{}' requires a String for 'path'", name));
                }
            }
            "rename" | "copy" => {
                let from_val = get_arg(0, "from")
                    .ok_or_else(|| format!("'fs.{}' missing argument 'from'", name))?;
                let to_val = get_arg(1, "to")
                    .ok_or_else(|| format!("'fs.{}' missing argument 'to'", name))?;
                if let (Value::String(from), Value::String(to)) = (from_val, to_val) {
                    let res = if name == "rename" {
                        std::fs::rename(from, to).map(|_| ())
                    } else {
                        std::fs::copy(from, to).map(|_| ())
                    };
                    match res {
                        Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Ok(Value::Unit),
                        )))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Err(Value::Error(format!(
                                "Failed to {} '{}' to '{}': {}",
                                name, from, to, e
                            ))),
                        )))),
                    }
                } else {
                    return Err(format!("'fs.{}' requires two String arguments", name));
                }
            }
            _ => Err(format!("Unknown filesystem operation 'fs.{}'", name)),
        }
    }

    fn eval_env_builtin(
        &mut self,
        name: &str,
        positional_args: &[Value],
        named_args: &HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            positional_args
                .get(idx)
                .or_else(|| named_args.get(arg_name))
        };

        match name {
            "get" => {
                let name_val = get_arg(0, "name")
                    .ok_or_else(|| format!("'env.get' missing argument 'name'"))?;
                if let Value::String(key) = name_val {
                    match std::env::var(key) {
                        Ok(val) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Ok(Value::String(val)),
                        )))),
                        Err(std::env::VarError::NotPresent) => Ok(ControlFlow::Value(
                            Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(
                                format!("Environment variable '{}' not found", key),
                            )))),
                        )),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Err(Value::Error(format!(
                                "Failed to get environment variable '{}': {}",
                                key, e
                            ))),
                        )))),
                    }
                } else {
                    return Err("'env.get' requires a String for 'name'".into());
                }
            }
            _ => Err(format!("Unknown environment operation 'env.{}'", name)),
        }
    }

    fn eval_os_builtin(
        &self,
        name: &str,
        args: &[Value],
        named_args: &HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            args.get(idx).or_else(|| named_args.get(arg_name))
        };

        match name {
            "args" => {
                let val_args = self.args.iter().map(|s| Value::String(s.clone())).collect();
                Ok(ControlFlow::Value(Value::Array(std::rc::Rc::new(
                    std::cell::RefCell::new(val_args),
                ))))
            }
            "cwd" => match std::env::current_dir() {
                Ok(path) => {
                    let path_str = path.to_string_lossy().to_string();
                    Ok(ControlFlow::Value(Value::Result(Box::new(
                        crate::value::ValueResult::Ok(Value::String(path_str)),
                    ))))
                }
                Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                    crate::value::ValueResult::Err(Value::Error(format!(
                        "Failed to get cwd: {}",
                        e
                    ))),
                )))),
            },
            "chdir" => {
                let path_val = get_arg(0, "path").ok_or("Missing argument 'path' for os.chdir")?;
                if let Value::String(path_str) = path_val {
                    match std::env::set_current_dir(path_str) {
                        Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Ok(Value::Unit),
                        )))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(
                            crate::value::ValueResult::Err(Value::Error(format!(
                                "Failed to chdir: {}",
                                e
                            ))),
                        )))),
                    }
                } else {
                    Err("os.chdir requires a String argument".into())
                }
            }
            _ => Err(format!("Unknown OS operation 'os.{}'", name)),
        }
    }

    fn eval_string_method(
        &mut self,
        s: &str,
        name: &str,
        args: &[Value],
        _named_args: &HashMap<std::string::String, Value>,
    ) -> Result<Value, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            args.get(idx).or_else(|| _named_args.get(arg_name))
        };

        match name {
            "len" => Ok(Value::Int64(s.chars().count() as i64)),
            "size" => Ok(Value::Int64(s.len() as i64)),
            "bytes" => {
                let bytes = std::rc::Rc::new(s.as_bytes().to_vec());
                Ok(Value::Slice(crate::value::SliceValue {
                    parent: crate::value::SliceParent::Bytes(bytes),
                    start: 0,
                    end: s.len(),
                }))
            }
            "trim" => Ok(Value::String(s.trim().to_string())),
            "trim_start" => Ok(Value::String(s.trim_start().to_string())),
            "trim_end" => Ok(Value::String(s.trim_end().to_string())),
            "to_lower" => Ok(Value::String(s.to_lowercase())),
            "to_upper" => Ok(Value::String(s.to_uppercase())),
            "contains" => {
                let pattern = get_arg(0, "pattern")
                    .or_else(|| get_arg(0, "substr"))
                    .ok_or("Missing argument for contains")?;
                if let Value::String(p) = pattern {
                    Ok(Value::Bool(s.contains(p)))
                } else {
                    Err("contains expects String".into())
                }
            }
            "starts_with" => {
                let prefix = get_arg(0, "prefix")
                    .or_else(|| get_arg(0, "pattern"))
                    .ok_or("Missing argument for starts_with")?;
                if let Value::String(p) = prefix {
                    Ok(Value::Bool(s.starts_with(p)))
                } else {
                    Err("starts_with expects String".into())
                }
            }
            "ends_with" => {
                let suffix = get_arg(0, "suffix")
                    .or_else(|| get_arg(0, "pattern"))
                    .ok_or("Missing argument for ends_with")?;
                if let Value::String(p) = suffix {
                    Ok(Value::Bool(s.ends_with(p)))
                } else {
                    Err("ends_with expects String".into())
                }
            }
            "split" => {
                let sep =
                    get_arg(0, "separator").ok_or("Missing argument 'separator' for split")?;
                if let Value::String(p) = sep {
                    let parts = s
                        .split(p)
                        .map(|part| Value::String(part.to_string()))
                        .collect();
                    Ok(Value::Array(std::rc::Rc::new(std::cell::RefCell::new(
                        parts,
                    ))))
                } else {
                    Err("split expects String".into())
                }
            }
            "replace" => {
                let from = get_arg(0, "from").ok_or("Missing argument 'from' for replace")?;
                let to = get_arg(1, "to").ok_or("Missing argument 'to' for replace")?;
                if let (Value::String(f), Value::String(t)) = (from, to) {
                    Ok(Value::String(s.replace(f, t)))
                } else {
                    Err("replace expects Strings".into())
                }
            }
            _ => Err(format!("Unknown String method '{}'", name)),
        }
    }

    fn eval_regex_constructor(name: &str, args: &[Value]) -> Result<RegexValue, String> {
        let require_regex = |v: &Value| -> Result<RegexValue, String> {
            if let Value::Regex(re) = v {
                Ok(re.clone())
            } else {
                Err("Expected Regex argument".into())
            }
        };

        match name {
            "text" => match args {
                [Value::String(s)] => Ok(RegexValue::Text(s.clone())),
                [_] => Err("'Regex.text' expects a String argument".into()),
                _ => Err("'Regex.text' expects exactly 1 argument".into()),
            },
            "any" => {
                if args.is_empty() {
                    Ok(RegexValue::Any)
                } else {
                    Err("'Regex.any' expects 0 arguments".into())
                }
            }
            "seq" => {
                if args.len() < 2 {
                    return Err("'Regex.seq' expects at least 2 Regex arguments".into());
                }
                let mut parts = Vec::with_capacity(args.len());
                for arg in args {
                    parts.push(require_regex(arg)?);
                }
                Ok(RegexValue::Seq(parts))
            }
            "alt" => {
                if args.len() < 2 {
                    return Err("'Regex.alt' expects at least 2 Regex arguments".into());
                }
                let mut parts = Vec::with_capacity(args.len());
                for arg in args {
                    parts.push(require_regex(arg)?);
                }
                Ok(RegexValue::Alt(parts))
            }
            "optional" => match args {
                [arg] => Ok(RegexValue::Optional(Box::new(require_regex(arg)?))),
                _ => Err("'Regex.optional' expects exactly 1 Regex argument".into()),
            },
            "zero_or_more" => match args {
                [arg] => Ok(RegexValue::ZeroOrMore(Box::new(require_regex(arg)?))),
                _ => Err("'Regex.zero_or_more' expects exactly 1 Regex argument".into()),
            },
            "one_or_more" => match args {
                [arg] => Ok(RegexValue::OneOrMore(Box::new(require_regex(arg)?))),
                _ => Err("'Regex.one_or_more' expects exactly 1 Regex argument".into()),
            },
            "start" => {
                if args.is_empty() {
                    Ok(RegexValue::Start)
                } else {
                    Err("'Regex.start' expects 0 arguments".into())
                }
            }
            "end" => {
                if args.is_empty() {
                    Ok(RegexValue::End)
                } else {
                    Err("'Regex.end' expects 0 arguments".into())
                }
            }
            "digit" => {
                if args.is_empty() {
                    Ok(RegexValue::Digit)
                } else {
                    Err("'Regex.digit' expects 0 arguments".into())
                }
            }
            "space" => {
                if args.is_empty() {
                    Ok(RegexValue::Space)
                } else {
                    Err("'Regex.space' expects 0 arguments".into())
                }
            }
            "word" => {
                if args.is_empty() {
                    Ok(RegexValue::Word)
                } else {
                    Err("'Regex.word' expects 0 arguments".into())
                }
            }
            _ => Err(format!("Unknown Regex constructor 'Regex.{}'", name)),
        }
    }

    fn eval_regex_method(re: &RegexValue, name: &str, args: &[Value]) -> Result<Value, String> {
        match name {
            "is_match" => match args {
                [Value::String(text)] => Ok(Value::Bool(re.is_match(text))),
                [_] => Err("'is_match' expects a String argument".into()),
                _ => Err("'is_match' expects exactly 1 argument".into()),
            },
            "replace" => match args {
                [Value::String(text), Value::String(with)] => {
                    Ok(Value::String(re.replace_all(text, with)))
                }
                [_, _] => Err("'replace' expects (String, String)".into()),
                _ => Err("'replace' expects exactly 2 arguments".into()),
            },
            _ => Err(format!("Unknown Regex method '{}'", name)),
        }
    }

    fn resolve_module_member_function(
        &self,
        member: &HirMember,
        env: &RuntimeEnv,
    ) -> Option<(String, HirFunction)> {
        let (root, namespace) = Self::module_namespace(&member.object)?;
        if env.get(&root).is_some() {
            return None;
        }
        self.resolve_module_function(&namespace, &member.field)
    }

    fn module_namespace(expression: &HirExpression) -> Option<(String, String)> {
        match expression {
            HirExpression::Name(_, name) => Some((name.clone(), name.clone())),
            HirExpression::Member(member) => {
                let (root, prefix) = Self::module_namespace(&member.object)?;
                Some((root, format!("{prefix}.{}", member.field)))
            }
            _ => None,
        }
    }

    fn bind_and_call_user_function(
        &mut self,
        call: &HirCall,
        identity: &str,
        function: &HirFunction,
        positional_values: Vec<Value>,
        named_values: HashMap<String, Value>,
    ) -> Result<ControlFlow, String> {
        let mut final_args = vec![None; function.parameters.len()];
        let num_positional_args = positional_values.len();
        let mut type_params_skipped = 0;
        let mut param_idx = 0;

        for value in positional_values {
            while param_idx < function.parameters.len()
                && function.parameters[param_idx].kind == ParameterKind::Type
            {
                let already_satisfied = call.type_args.is_some();
                let not_enough_args = (num_positional_args - (param_idx - type_params_skipped))
                    < (function.parameters.len() - param_idx);

                if already_satisfied || not_enough_args {
                    param_idx += 1;
                    type_params_skipped += 1;
                } else {
                    break;
                }
            }

            if param_idx < final_args.len() {
                final_args[param_idx] = Some(value);
                param_idx += 1;
            } else {
                return Err(format!("Too many arguments for {identity}"));
            }
        }

        for (name, value) in named_values {
            if let Some(index) = function
                .parameters
                .iter()
                .position(|parameter| parameter.name == name)
            {
                if final_args[index].is_some() {
                    return Err(format!("Duplicate argument for parameter {name}"));
                }
                final_args[index] = Some(value);
            } else {
                return Err(format!(
                    "Unknown named argument {name} for function {identity}"
                ));
            }
        }

        let mut args = Vec::new();
        for (index, value) in final_args.into_iter().enumerate() {
            match value {
                Some(value) => args.push(value),
                None => {
                    let parameter = &function.parameters[index];
                    let satisfied_by_type_arg = parameter.kind == ParameterKind::Type
                        && call.type_args.as_ref().is_some_and(|type_args| {
                            type_args.len() == function.generic_parameters.len()
                        });

                    if !satisfied_by_type_arg {
                        return Err(format!("Missing argument for parameter {}", parameter.name));
                    }
                    // The concrete type value is installed by `call_function`.
                    args.push(Value::Unit);
                }
            }
        }

        let type_args = call.type_args.as_ref().map(|type_args| {
            type_args
                .iter()
                .map(|ty| self.resolve_hir_type(ty))
                .map(Value::Type)
                .collect()
        });

        self.call_resolved_function(identity, function, type_args, args)
    }

    /// Phase 7: Dispatch a read-only method call on a receiver value.
    /// Used for user-defined `get` and `slice` hooks.
    fn eval_method_call(
        &mut self,
        receiver: Value,
        method_name: &str,
        args: Vec<Value>,
        _env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        let type_name = match &receiver {
            Value::Struct(s) => s.name.clone(),
            Value::Enum(e) => e.enum_name.clone(),
            _ => {
                return Err(format!(
                    "type {} has no method '{}'",
                    receiver.type_name(),
                    method_name
                ))
            }
        };
        let mangled = format!("{}.{}", type_name, method_name);
        if let Some((identity, function)) = self.resolve_user_function(&mangled) {
            let mut call_args = vec![receiver];
            call_args.extend(args);
            self.call_resolved_function(&identity, &function, None, call_args)
        } else {
            Err(format!(
                "no method '{}' found on type '{}'",
                method_name, type_name
            ))
        }
    }

    /// Phase 7: Dispatch a mutating method call on a receiver identified by variable name.
    /// Used for user-defined `set` and `set_slice` hooks.
    /// After the method executes, the (potentially mutated) receiver is written back to env.
    fn eval_method_call_inout(
        &mut self,
        receiver_name: &str,
        method_name: &str,
        args: Vec<Value>,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        let receiver = env
            .get(receiver_name)
            .ok_or_else(|| format!("Variable '{}' not found", receiver_name))?;
        let type_name = match &receiver {
            Value::Struct(s) => s.name.clone(),
            Value::Enum(e) => e.enum_name.clone(),
            _ => {
                return Err(format!(
                    "type {} has no method '{}'",
                    receiver.type_name(),
                    method_name
                ))
            }
        };
        let mangled = format!("{}.{}", type_name, method_name);
        if let Some((identity, function)) = self.resolve_user_function(&mangled) {
            let mut call_args = vec![receiver];
            call_args.extend(args);
            let (result, call_env) =
                self.call_resolved_function_with_env(&identity, &function, None, call_args)?;
            // Writeback: extract the mutated 'self' and update it in the caller's environment
            if let Some(mutated_self) = call_env.get("self") {
                env.update(receiver_name, mutated_self);
            }
            Ok(result)
        } else {
            Err(format!(
                "no method '{}' found on type '{}'",
                method_name, type_name
            ))
        }
    }

    /// Phase 7: Extract the root variable name from an assignment target expression.
    fn extract_receiver_name(expr: &HirExpression) -> Option<String> {
        match expr {
            HirExpression::Name(_, name) => Some(name.clone()),
            HirExpression::Member(m) => Self::extract_receiver_name(&m.object),
            _ => None,
        }
    }
}
