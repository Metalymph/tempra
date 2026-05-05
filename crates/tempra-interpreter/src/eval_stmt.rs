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
                let iter_values: Vec<Value> = match iterable {
                    Value::Array(arr) => arr.borrow().clone(),
                    Value::Slice(slice) => {
                        let mut vals = Vec::new();
                        for i in 0..slice.len() {
                            vals.push(slice.get(i).unwrap());
                        }
                        vals
                    }
                    Value::String(s) => s.chars().map(|c| Value::String(c.to_string())).collect(),
                    _ => return Err("For loop iterable must be an Array, Slice, or String".into()),
                };

                let mut last_val = Value::Unit;
                for item in iter_values {
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
                            return Err("For loop filter must be a Bool".into());
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
                Ok(ControlFlow::Value(last_val))
            }
        }
    }
}
