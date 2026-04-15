use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::{StructInstance, Value};
use fero_hir::*;
use std::collections::HashMap;

fn run_command(c: &crate::value::CommandValue) -> Result<crate::value::CommandResultValue, String> {
    let mut cmd = std::process::Command::new(&c.program);
    cmd.args(&c.args);
    let output = cmd.output().map_err(|e| format!("Command error: {}", e))?;
    Ok(crate::value::CommandResultValue {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    })
}

fn pipe_commands(left: &crate::value::CommandValue, right: &crate::value::CommandValue) -> Result<Value, String> {
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
    
    let output = right_child.wait_with_output().map_err(|e| format!("Right wait error: {}", e))?;
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
        Value::Float64(f) => f.to_string(),
        // For other types, we use their Display implementation if they have one,
        // but the prompt is specific about which types to support in the helper.
        _ => value.to_string(),
    }
}

impl Interpreter {
    pub fn eval_expression(
        &mut self,
        expr: &HirExpression,
        env: &mut RuntimeEnv,
    ) -> Result<ControlFlow, String> {
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
            HirExpression::Name(_, name) => env
                .get(name)
                .map(ControlFlow::Value)
                .ok_or_else(|| format!("Variable {} not found", name)),
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
                        Value::Int64(i) => Ok(ControlFlow::Value(Value::Int64(-i))),
                        Value::Float64(f) => Ok(ControlFlow::Value(Value::Float64(-f))),
                        _ => Err("- requires a numeric type".into()),
                    },
                }
            }
            HirExpression::Binary(binary) => {
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
                        let res_left = run_command(cmd_left)?;
                        if !res_left.success {
                            return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(res_left))))));
                        }
                        let right = match self.eval_expression(&binary.right, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right).map(|v| ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(v))))));
                        } else {
                            return Err("&& right operand must be Command if left is Command".into());
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
                        let res_left = run_command(cmd_left)?;
                        if res_left.success {
                            return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(res_left))))));
                        }
                        let right = match self.eval_expression(&binary.right, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right).map(|v| ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(v))))));
                        } else {
                            return Err("|| right operand must be Command if left is Command".into());
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

                // Assignment
                if binary.op == HirBinaryOp::Assign {
                    let right = match self.eval_expression(&binary.right, env)? {
                        ControlFlow::Value(v) => v,
                        cf => return Ok(cf),
                    };
                    if let HirExpression::Name(_, name) = &*binary.left {
                        env.insert(name.clone(), right.clone());
                        return Ok(ControlFlow::Value(right));
                    } else {
                        return Err("Invalid assignment target".into());
                    }
                }

                let right = match self.eval_expression(&binary.right, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };

                match binary.op {
                    HirBinaryOp::Add => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Int64(a + b)))
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
                            Ok(ControlFlow::Value(Value::Int64(a - b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a - b)))
                        }
                        _ => Err("- operator mismatch".into()),
                    },
                    HirBinaryOp::Mul => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Int64(a * b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a * b)))
                        }
                        _ => Err("* operator mismatch".into()),
                    },
                    HirBinaryOp::Div => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Int64(a / b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Float64(a / b)))
                        }
                        _ => Err("/ operator mismatch".into()),
                    },
                    HirBinaryOp::Eq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a == b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a == b)))
                        }
                        (Value::Bool(a), Value::Bool(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a == b)))
                        }
                        (Value::String(a), Value::String(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a == b)))
                        }
                        _ => Ok(ControlFlow::Value(Value::Bool(false))),
                    },
                    HirBinaryOp::NotEq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a != b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a != b)))
                        }
                        (Value::Bool(a), Value::Bool(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a != b)))
                        }
                        (Value::String(a), Value::String(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a != b)))
                        }
                        _ => Ok(ControlFlow::Value(Value::Bool(true))),
                    },
                    HirBinaryOp::Less => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a < b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a < b)))
                        }
                        _ => Err("< operator mismatch".into()),
                    },
                    HirBinaryOp::Greater => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a > b)))
                        }
                        (Value::Float64(a), Value::Float64(b)) => {
                            Ok(ControlFlow::Value(Value::Bool(a > b)))
                        }
                        _ => Err("> operator mismatch".into()),
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
            HirExpression::Call(call) => {
                if let HirExpression::Member(member) = &*call.callee {
                    if let HirExpression::Name(_, ref obj_name) = *member.object {
                        if obj_name == "fs" {
                            let mut args = Vec::new();
                            for arg_expr in &call.args {
                                match self.eval_expression(arg_expr, env)? {
                                    ControlFlow::Value(v) => args.push(v),
                                    cf => return Ok(cf),
                                }
                            }
                            return self.eval_fs_builtin(&member.field, &args);
                        }
                    }
                    if member.field == "run" {
                        let obj = match self.eval_expression(&member.object, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::Command(cmd) = obj {
                            if !call.args.is_empty() {
                                return Err("Method 'run' expects 0 arguments".into());
                            }
                            return match run_command(&cmd) {
                                Ok(v) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(v)))))),
                                Err(msg) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(msg)))))),
                            };
                        } else {
                            return Err("Method 'run' requires a Command".into());
                        }
                    }
                }

                let mut args = Vec::new();
                for arg_expr in &call.args {
                    match self.eval_expression(arg_expr, env)? {
                        ControlFlow::Value(v) => args.push(v),
                        cf => return Ok(cf),
                    }
                }

                if let HirExpression::Name(_, name) = &*call.callee {
                    if name == "exit" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'exit' expects 1 argument"));
                        }
                        if let Value::Int64(code) = args[0] {
                            return Ok(ControlFlow::Exit(code as i32));
                        } else {
                            return Err("exit requires an Int64 argument".into());
                        }
                    }
                    if name == "print" {
                        let result = args.iter().map(value_to_string).collect::<String>();
                        println!("{}", result);
                        return Ok(ControlFlow::Value(Value::Unit));
                    }
                    if name == "panic" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'panic' expects 1 argument"));
                        }
                        return Ok(ControlFlow::Panic(value_to_string(&args[0])));
                    }
                    if name == "fmt" {
                        let result = args.iter().map(value_to_string).collect::<String>();
                        return Ok(ControlFlow::Value(Value::String(result)));
                    }
                    if name == "read_line" {
                        if args.len() != 0 {
                            return Err(format!("Builtin 'read_line' expects 0 arguments"));
                        }
                        let mut input = String::new();
                        return match std::io::stdin().read_line(&mut input) {
                            Ok(_) => {
                                if input.ends_with('\n') {
                                    input.pop();
                                    if input.ends_with('\r') {
                                        input.pop();
                                    }
                                }
                                Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(input))))))
                            }
                            Err(e) => {
                                Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to read stdin: {}", e)))))))
                            }
                        }
                    }
                    if name == "cmd" {
                        if args.len() != 2 {
                            return Err(format!("Builtin 'cmd' expects 2 arguments"));
                        }
                        let program = match &args[0] {
                            Value::String(s) => s.clone(),
                            _ => return Err("cmd requires a String as first argument".into()),
                        };
                        let cmd_args = match &args[1] {
                            Value::Array(arr) => {
                                let mut a = Vec::new();
                                for el in arr {
                                    if let Value::String(s) = el {
                                        a.push(s.clone());
                                    } else {
                                        return Err("cmd requires an array of Strings".into());
                                    }
                                }
                                a
                            }
                            _ => return Err("cmd requires an array of Strings as second argument".into()),
                        };
                        return Ok(ControlFlow::Value(Value::Command(crate::value::CommandValue { program, args: cmd_args })));
                    }

                    if name == "Ok" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'Ok' expects 1 argument"));
                        }
                        return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(args[0].clone())))));
                    }
                    if name == "Err" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'Err' expects 1 argument"));
                        }
                        return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(args[0].clone())))));
                    }
                    if name == "Error" {
                         if args.len() != 1 {
                            return Err(format!("Builtin 'Error' expects 1 argument"));
                        }
                        return Ok(ControlFlow::Value(Value::Error(value_to_string(&args[0]))));
                    }
                    if name == "expect" {
                        if args.is_empty() || args.len() > 2 {
                            return Err(format!("Builtin 'expect' expects 1 or 2 arguments"));
                        }
                        if let Value::Bool(b) = args[0] {
                            if b {
                                return Ok(ControlFlow::Value(Value::Unit));
                            } else {
                                let msg = if args.len() == 2 {
                                    value_to_string(&args[1])
                                } else {
                                    "expectation failed".to_string()
                                };
                                return Ok(ControlFlow::Panic(msg));
                            }
                        } else {
                            return Err("expect requires a Bool as first argument".into());
                        }
                    }
                    if name == "dbg" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'dbg' expects 1 argument"));
                        }
                        let repr = match &args[0] {
                            Value::String(s) => format!("\"{}\"", s),
                            v => v.to_string(),
                        };
                        println!("dbg: {}", repr);
                        return Ok(ControlFlow::Value(args[0].clone()));
                    }

                    let func = self
                        .functions
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("Function {} not found", name))?;
                    
                    self.call_function(&func, args)
                } else {
                    Err("Only named functions are supported".into())
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
                    _ => Err("Member access requires a struct or CommandResult".into()),
                }
            }
            HirExpression::Index(index) => {
                let obj = match self.eval_expression(&index.object, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let idx = match self.eval_expression(&index.index, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                if let Value::Array(arr) = obj {
                    if let Value::Int64(i) = idx {
                        arr.get(i as usize)
                            .cloned()
                            .map(ControlFlow::Value)
                            .ok_or_else(|| "Index out of bounds".into())
                    } else {
                        Err("Index must be Int64".into())
                    }
                } else {
                    Err("Index access requires an array".into())
                }
            }
            HirExpression::If(if_expr) => {
                let cond = match self.eval_expression(&if_expr.condition, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let is_true = match cond {
                    Value::Bool(b) => b,
                    _ => return Err("If condition must be a Bool".into()),
                };

                if is_true {
                    self.eval_block(&if_expr.then_block, env)
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
                Ok(ControlFlow::Value(Value::Array(vals)))
            }
            HirExpression::StructLiteral(s) => {
                let mut fields = HashMap::new();
                for (name, expr) in &s.fields {
                    match self.eval_expression(expr, env)? {
                        ControlFlow::Value(v) => { fields.insert(name.clone(), v); },
                        cf => return Ok(cf),
                    }
                }
                Ok(ControlFlow::Value(Value::Struct(StructInstance {
                    name: s.name.clone(),
                    fields,
                })))
            }
            HirExpression::For(for_expr) => {
                let iterable = match self.eval_expression(&for_expr.iterable, env)? {
                    ControlFlow::Value(v) => v,
                    cf => return Ok(cf),
                };
                let iter_values = match iterable {
                    Value::Array(arr) => arr,
                    _ => return Err("ForExpr iterable must be an Array".into()),
                };

                let mut last_val = Value::Unit;
                for item in iter_values {
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
                            return Err("ForExpr filter must be a Bool".into());
                        }
                    }

                    match self.eval_block(&for_expr.body, env)? {
                        ControlFlow::Value(v) => last_val = v,
                        ControlFlow::Return(v) => {
                            env.pop_scope();
                            return Ok(ControlFlow::Return(v));
                        }
                        ControlFlow::Exit(code) => {
                            env.pop_scope();
                            return Ok(ControlFlow::Exit(code));
                        }
                        ControlFlow::Panic(msg) => {
                            env.pop_scope();
                            return Ok(ControlFlow::Panic(msg));
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
                Ok(ControlFlow::Value(last_val))
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
                        crate::value::ValueResult::Err(v) => {
                            Ok(ControlFlow::Return(Value::Result(Box::new(crate::value::ValueResult::Err(v.clone())))))
                        }
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
                            Ok(ControlFlow::Panic(format!("Unwrap failed: {}", v)))
                        }
                    },
                    _ => Err("'!' expects a Result value".into()),
                }
            }
            HirExpression::Match(_) => Err("Match expressions are not fully supported yet".into()),
        }
    }

    fn eval_block(&mut self, block: &HirBlock, env: &mut RuntimeEnv) -> Result<ControlFlow, String> {
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
                ControlFlow::Panic(msg) => {
                    env.pop_scope();
                    return Ok(ControlFlow::Panic(msg));
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

    fn eval_fs_builtin(&mut self, name: &str, args: &[Value]) -> Result<ControlFlow, String> {
        match name {
            "read" => {
                if args.len() != 1 {
                    return Err(format!("'fs.read' expects 1 argument"));
                }
                if let Value::String(path) = &args[0] {
                    return match std::fs::read_to_string(path) {
                        Ok(text) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(text)))))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to read '{}': {}", path, e))))))),
                    }
                } else {
                    return Err("'fs.read' requires a String as first argument".into());
                }
            }
            "write" => {
                if args.len() != 2 {
                    return Err(format!("'fs.write' expects 2 arguments"));
                }
                if let (Value::String(path), Value::String(text)) = (&args[0], &args[1]) {
                    return match std::fs::write(path, text) {
                        Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to write to '{}': {}", path, e))))))),
                    }
                } else {
                    return Err("'fs.write' requires two String arguments".into());
                }
            }
            "append" => {
                if args.len() != 2 {
                    return Err(format!("'fs.append' expects 2 arguments"));
                }
                if let (Value::String(path), Value::String(text)) = (&args[0], &args[1]) {
                    use std::io::Write;
                    let res = (|| -> Result<(), String> {
                        let mut file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .map_err(|e| format!("Failed to open '{}' for appending: {}", path, e))?;
                        file.write_all(text.as_bytes())
                            .map_err(|e| format!("Failed to append to '{}': {}", path, e))?;
                        Ok(())
                    })();
                    return match res {
                        Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                        Err(msg) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(msg)))))),
                    }
                } else {
                    return Err("'fs.append' requires two String arguments".into());
                }
            }
            _ => Err(format!("Unknown filesystem operation 'fs.{}'", name)),
        }
    }
}
