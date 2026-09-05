use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::Value;
use tempra_hir::*;

impl Interpreter {
    pub fn eval_statement(
        &mut self,
        stmt: &HirStatement,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        match stmt {
            HirStatement::Let(let_stmt) => match self.eval_expression(&let_stmt.value, env)? {
                ControlFlow::Value(val) => {
                    env.insert(let_stmt.name.clone(), val);
                    Ok(ControlFlow::Value(Value::Unit))
                }
                cf => Ok(cf),
            },
            HirStatement::Var(var_stmt) => match self.eval_expression(&var_stmt.value, env)? {
                ControlFlow::Value(val) => {
                    env.insert_var(var_stmt.name.clone(), val);
                    Ok(ControlFlow::Value(Value::Unit))
                }
                cf => Ok(cf),
            },
            HirStatement::Return(ret) => {
                let val = if let Some(expr) = ret {
                    match self.eval_expression(expr, env)? {
                        ControlFlow::Value(val) => val,
                        cf => return Ok(cf),
                    }
                } else {
                    Value::Unit
                };
                Ok(ControlFlow::Return(val))
            }
            HirStatement::Break(_) => Ok(ControlFlow::Break),
            HirStatement::Continue(_) => Ok(ControlFlow::Continue),
            HirStatement::Expr(expr) => self.eval_expression(expr, env),
            HirStatement::For(for_stmt) => {
                let iterable = match self.eval_expression(&for_stmt.iterable, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                let mut iterator = None;
                let mut char_values = None;

                match &iterable {
                    Value::String(s) => {
                        char_values = Some(
                            s.chars()
                                .map(|c| Value::String(c.to_string()))
                                .collect::<Vec<_>>(),
                        );
                    }
                    Value::Array(arr) => {
                        iterator = Some(Value::ArrayIter(crate::value::ArrayIterValue {
                            array: arr.clone(),
                            index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                        }));
                    }
                    Value::Slice(slice) => {
                        iterator = Some(Value::SliceIter(crate::value::SliceIterValue {
                            slice: slice.clone(),
                            index: std::rc::Rc::new(std::cell::RefCell::new(0)),
                        }));
                    }
                    Value::Struct(s) => {
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
                    }
                    other => {
                        return Err(format!(
                            "value of type '{}' is not iterable",
                            other.type_name()
                        ));
                    }
                }

                let mut last_val = Value::Unit;
                if let Some(chars) = char_values {
                    for item in chars {
                        env.push_scope();
                        env.insert(for_stmt.binding.clone(), item);

                        if let Some(filter_expr) = &for_stmt.filter {
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

                        let mut loop_flow = ControlFlow::Value(Value::Unit);
                        for body_stmt in &for_stmt.body.statements {
                            let flow = self.eval_statement(body_stmt, env)?;
                            match flow {
                                ControlFlow::Value(v) => loop_flow = ControlFlow::Value(v),
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
                                    loop_flow = ControlFlow::Break;
                                    break;
                                }
                                ControlFlow::Continue => {
                                    loop_flow = ControlFlow::Continue;
                                    break;
                                }
                            }
                        }
                        env.pop_scope();

                        if let ControlFlow::Break = loop_flow {
                            break;
                        }
                        if let ControlFlow::Exit(code) = loop_flow {
                            return Ok(ControlFlow::Exit(code));
                        }
                        if let ControlFlow::Crash(msg) = loop_flow {
                            return Ok(ControlFlow::Crash(msg));
                        }
                        if let ControlFlow::Value(v) = loop_flow {
                            last_val = v;
                        }
                    }
                } else if let Some(mut iter_val) = iterator {
                    loop {
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
                                env.insert(for_stmt.binding.clone(), *item);

                                if let Some(filter_expr) = &for_stmt.filter {
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

                                let mut loop_flow = ControlFlow::Value(Value::Unit);
                                for body_stmt in &for_stmt.body.statements {
                                    let flow = self.eval_statement(body_stmt, env)?;
                                    match flow {
                                        ControlFlow::Value(v) => loop_flow = ControlFlow::Value(v),
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
                                            loop_flow = ControlFlow::Break;
                                            break;
                                        }
                                        ControlFlow::Continue => {
                                            loop_flow = ControlFlow::Continue;
                                            break;
                                        }
                                    }
                                }
                                env.pop_scope();

                                if let ControlFlow::Break = loop_flow {
                                    break;
                                }
                                if let ControlFlow::Exit(code) = loop_flow {
                                    return Ok(ControlFlow::Exit(code));
                                }
                                if let ControlFlow::Crash(msg) = loop_flow {
                                    return Ok(ControlFlow::Crash(msg));
                                }
                                if let ControlFlow::Value(v) = loop_flow {
                                    last_val = v;
                                }
                            }
                            _ => return Err("Iterator next() must return an Option".into()),
                        }
                    }
                }
                Ok(ControlFlow::Value(last_val))
            }
        }
    }
}
