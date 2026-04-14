use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::Value;
use fero_hir::*;

impl Interpreter {
    pub fn eval_statement(
        &mut self,
        stmt: &HirStatement,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
        match stmt {
            HirStatement::Let(let_stmt) => {
                match self.eval_expression(&let_stmt.value, env) {
                    Ok(val) => {
                        env.insert(let_stmt.name.clone(), val);
                        Ok(ControlFlow::Value(Value::Unit))
                    }
                    Err(e) if e.starts_with("@@FERO_EXIT@@") => {
                        let code: i32 = e["@@FERO_EXIT@@".len()..].parse().unwrap_or(1);
                        Ok(ControlFlow::Exit(code))
                    }
                    Err(e) => Err(e),
                }
            }
            HirStatement::Var(var_stmt) => {
                match self.eval_expression(&var_stmt.value, env) {
                    Ok(val) => {
                        env.insert(var_stmt.name.clone(), val);
                        Ok(ControlFlow::Value(Value::Unit))
                    }
                    Err(e) if e.starts_with("@@FERO_EXIT@@") => {
                        let code: i32 = e["@@FERO_EXIT@@".len()..].parse().unwrap_or(1);
                        Ok(ControlFlow::Exit(code))
                    }
                    Err(e) => Err(e),
                }
            }
            HirStatement::Return(ret) => {
                let val = if let Some(expr) = ret {
                    match self.eval_expression(expr, env) {
                        Ok(val) => val,
                        Err(e) if e.starts_with("@@FERO_EXIT@@") => {
                            let code: i32 = e["@@FERO_EXIT@@".len()..].parse().unwrap_or(1);
                            return Ok(ControlFlow::Exit(code));
                        }
                        Err(e) => return Err(e),
                    }
                } else {
                    Value::Unit
                };
                Ok(ControlFlow::Return(val))
            }
            HirStatement::Break(_) => Ok(ControlFlow::Break),
            HirStatement::Continue(_) => Ok(ControlFlow::Continue),
            HirStatement::Expr(expr) => {
                match self.eval_expression(expr, env) {
                    Ok(val) => Ok(ControlFlow::Value(val)),
                    Err(e) if e.starts_with("@@FERO_EXIT@@") => {
                        let code: i32 = e["@@FERO_EXIT@@".len()..].parse().unwrap_or(1);
                        Ok(ControlFlow::Exit(code))
                    }
                    Err(e) => Err(e),
                }
            }
            HirStatement::For(for_stmt) => {
                let iterable = self.eval_expression(&for_stmt.iterable, env)?;
                let iter_values = match iterable {
                    Value::Array(arr) => arr,
                    _ => return Err("For loop iterable must be an Array".into()),
                };

                let mut last_val = Value::Unit;
                for item in iter_values {
                    env.push_scope();
                    env.insert(for_stmt.binding.clone(), item);

                    // Note: If filters exist, we'd eval them here and skip if false. Not strictly required by prompt but good to have if HirFor has it.
                    if let Some(filter_expr) = &for_stmt.filter {
                        let filter_res = self.eval_expression(filter_expr, env)?;
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
                    if let ControlFlow::Value(v) = loop_flow {
                        last_val = v;
                    }
                }
                Ok(ControlFlow::Value(last_val))
            }
        }
    }
}
