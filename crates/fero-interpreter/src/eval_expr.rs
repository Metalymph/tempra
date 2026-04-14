use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::{StructInstance, Value};
use fero_hir::*;
use std::collections::HashMap;

fn run_command(c: &crate::value::CommandValue) -> Result<Value, String> {
    let mut cmd = std::process::Command::new(&c.program);
    cmd.args(&c.args);
    let output = cmd.output().map_err(|e| format!("Command error: {}", e))?;
    Ok(Value::CommandResult(crate::value::CommandResultValue {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        success: output.status.success(),
    }))
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

impl Interpreter {
    pub fn eval_expression(
        &mut self,
        expr: &HirExpression,
        env: &mut RuntimeEnv,
    ) -> Result<Value, String> {
        match expr {
            HirExpression::Integer(_, s) => {
                let v = s.parse::<i64>().map_err(|_| "Invalid Int64".to_string())?;
                Ok(Value::Int64(v))
            }
            HirExpression::Float(_, s) => {
                let v = s
                    .parse::<f64>()
                    .map_err(|_| "Invalid Float64".to_string())?;
                Ok(Value::Float64(v))
            }
            HirExpression::String(_, s) => {
                let unquoted = if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                    s[1..s.len() - 1].to_string()
                } else {
                    s.clone()
                };
                Ok(Value::String(unquoted))
            }
            HirExpression::Bool(_, b) => Ok(Value::Bool(*b)),
            HirExpression::Name(_, name) => env
                .get(name)
                .ok_or_else(|| format!("Variable {} not found", name)),
            HirExpression::Unary(unary) => {
                let val = self.eval_expression(&unary.expr, env)?;
                match unary.op {
                    HirUnaryOp::Not => {
                        if let Value::Bool(b) = val {
                            Ok(Value::Bool(!b))
                        } else {
                            Err("! requires a Bool".into())
                        }
                    }
                    HirUnaryOp::Negate => match val {
                        Value::Int64(i) => Ok(Value::Int64(-i)),
                        Value::Float64(f) => Ok(Value::Float64(-f)),
                        _ => Err("- requires a numeric type".into()),
                    },
                }
            }
            HirExpression::Binary(binary) => {
                let mut v_left = None;

                // Special handling for short-circuiting logical operators and pipe
                if binary.op == HirBinaryOp::And {
                    let left = self.eval_expression(&binary.left, env)?;
                    if let Value::Bool(false) = left {
                        return Ok(Value::Bool(false));
                    } else if let Value::Command(ref cmd_left) = left {
                        let res_left = run_command(cmd_left)?;
                        if let Value::CommandResult(r) = &res_left {
                            if !r.success {
                                return Ok(res_left);
                            }
                        }
                        let right = self.eval_expression(&binary.right, env)?;
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right);
                        } else {
                            return Err("&& right operand must be Command if left is Command".into());
                        }
                    }
                    v_left = Some(left);
                } else if binary.op == HirBinaryOp::Or {
                    let left = self.eval_expression(&binary.left, env)?;
                    if let Value::Bool(true) = left {
                        return Ok(Value::Bool(true));
                    } else if let Value::Command(ref cmd_left) = left {
                        let res_left = run_command(cmd_left)?;
                        if let Value::CommandResult(r) = &res_left {
                            if r.success {
                                return Ok(res_left);
                            }
                        }
                        let right = self.eval_expression(&binary.right, env)?;
                        if let Value::Command(cmd_right) = right {
                            return run_command(&cmd_right);
                        } else {
                            return Err("|| right operand must be Command if left is Command".into());
                        }
                    }
                    v_left = Some(left);
                } else if binary.op == HirBinaryOp::Pipe {
                    let left = self.eval_expression(&binary.left, env)?;
                    let right = self.eval_expression(&binary.right, env)?;
                    if let (Value::Command(cmd_left), Value::Command(cmd_right)) = (left, right) {
                        return pipe_commands(&cmd_left, &cmd_right);
                    } else {
                        return Err("| operator expects Commands".into());
                    }
                }

                let left = if let Some(v) = v_left {
                    v
                } else {
                    self.eval_expression(&binary.left, env)?
                };

                // Assignment
                if binary.op == HirBinaryOp::Assign {
                    let right = self.eval_expression(&binary.right, env)?;
                    if let HirExpression::Name(_, name) = &*binary.left {
                        env.insert(name.clone(), right.clone());
                        return Ok(right);
                    } else {
                        // Advanced assignment like obj.field = x is not strictly required by the basic spec,
                        // but normally handled. We'll return an error for now to keep it minimal as requested.
                        return Err("Invalid assignment target".into());
                    }
                }

                let right = self.eval_expression(&binary.right, env)?;

                match binary.op {
                    HirBinaryOp::Add => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Int64(a + b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(a + b)),
                        (Value::String(a), Value::String(b)) => Ok(Value::String(a + &b)),
                        _ => Err("+ operator mismatch".into()),
                    },
                    HirBinaryOp::Sub => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Int64(a - b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(a - b)),
                        _ => Err("- operator mismatch".into()),
                    },
                    HirBinaryOp::Mul => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Int64(a * b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(a * b)),
                        _ => Err("* operator mismatch".into()),
                    },
                    HirBinaryOp::Div => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Int64(a / b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Float64(a / b)),
                        _ => Err("/ operator mismatch".into()),
                    },
                    HirBinaryOp::Eq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Bool(a == b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Bool(a == b)),
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a == b)),
                        (Value::String(a), Value::String(b)) => Ok(Value::Bool(a == b)),
                        _ => Ok(Value::Bool(false)), // Or error
                    },
                    HirBinaryOp::NotEq => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Bool(a != b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Bool(a != b)),
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a != b)),
                        (Value::String(a), Value::String(b)) => Ok(Value::Bool(a != b)),
                        _ => Ok(Value::Bool(true)), // Or error
                    },
                    HirBinaryOp::Less => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Bool(a < b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Bool(a < b)),
                        _ => Err("< operator mismatch".into()),
                    },
                    HirBinaryOp::Greater => match (left, right) {
                        (Value::Int64(a), Value::Int64(b)) => Ok(Value::Bool(a > b)),
                        (Value::Float64(a), Value::Float64(b)) => Ok(Value::Bool(a > b)),
                        _ => Err("> operator mismatch".into()),
                    },
                    HirBinaryOp::And => match (left, right) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
                        _ => Err("&& requires Bool".into()),
                    },
                    HirBinaryOp::Or => match (left, right) {
                        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
                        _ => Err("|| requires Bool".into()),
                    },
                    _ => Err("Unsupported binary operator".into()),
                }
            }
            HirExpression::Call(call) => {
                if let HirExpression::Member(member) = &*call.callee {
                    if member.field == "run" {
                        let obj = self.eval_expression(&member.object, env)?;
                        if let Value::Command(cmd) = obj {
                            if !call.args.is_empty() {
                                return Err("Method 'run' expects 0 arguments".into());
                            }
                            return run_command(&cmd);
                        } else {
                            return Err("Method 'run' requires a Command".into());
                        }
                    }
                }

                let mut args = Vec::new();
                for arg_expr in &call.args {
                    args.push(self.eval_expression(arg_expr, env)?);
                }

                if let HirExpression::Name(_, name) = &*call.callee {
                    if name == "print" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'print' expects 1 argument"));
                        }
                        println!("{}", args[0]);
                        return Ok(Value::Unit);
                    }
                    if name == "read_line" {
                        if args.len() != 0 {
                            return Err(format!("Builtin 'read_line' expects 0 arguments"));
                        }
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).map_err(|e| format!("Failed to read stdin: {}", e))?;
                        if input.ends_with('\n') {
                            input.pop();
                            if input.ends_with('\r') {
                                input.pop();
                            }
                        }
                        return Ok(Value::String(input));
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
                        return Ok(Value::Command(crate::value::CommandValue { program, args: cmd_args }));
                    }
                    if name == "read" {
                        if args.len() != 1 {
                            return Err(format!("Builtin 'read' expects 1 argument"));
                        }
                        if let Value::String(path) = &args[0] {
                            let text = std::fs::read_to_string(path)
                                .map_err(|e| format!("Failed to read '{}': {}", path, e))?;
                            return Ok(Value::String(text));
                        } else {
                            return Err("read requires a String as first argument".into());
                        }
                    }
                    if name == "write" {
                        if args.len() != 2 {
                            return Err(format!("Builtin 'write' expects 2 arguments"));
                        }
                        if let (Value::String(path), Value::String(text)) = (&args[0], &args[1]) {
                            std::fs::write(path, text)
                                .map_err(|e| format!("Failed to write to '{}': {}", path, e))?;
                            return Ok(Value::Unit);
                        } else {
                            return Err("write requires two String arguments".into());
                        }
                    }
                    if name == "append" {
                        if args.len() != 2 {
                            return Err(format!("Builtin 'append' expects 2 arguments"));
                        }
                        if let (Value::String(path), Value::String(text)) = (&args[0], &args[1]) {
                            use std::io::Write;
                            let mut file = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                                .map_err(|e| format!("Failed to open '{}' for appending: {}", path, e))?;
                            file.write_all(text.as_bytes())
                                .map_err(|e| format!("Failed to append to '{}': {}", path, e))?;
                            return Ok(Value::Unit);
                        } else {
                            return Err("append requires two String arguments".into());
                        }
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
                let obj = self.eval_expression(&member.object, env)?;
                match obj {
                    Value::Struct(s) => s
                        .fields
                        .get(&member.field)
                        .cloned()
                        .ok_or_else(|| format!("Field {} not found", member.field)),
                    Value::CommandResult(r) => match member.field.as_str() {
                        "exit_code" => Ok(Value::Int64(r.exit_code as i64)),
                        "stdout" => Ok(Value::String(r.stdout.clone())),
                        "stderr" => Ok(Value::String(r.stderr.clone())),
                        "success" => Ok(Value::Bool(r.success)),
                        _ => Err(format!("Field {} not found on CommandResult", member.field)),
                    },
                    _ => Err("Member access requires a struct or CommandResult".into()),
                }
            }
            HirExpression::Index(index) => {
                let obj = self.eval_expression(&index.object, env)?;
                let idx = self.eval_expression(&index.index, env)?;
                if let Value::Array(arr) = obj {
                    if let Value::Int64(i) = idx {
                        arr.get(i as usize)
                            .cloned()
                            .ok_or_else(|| "Index out of bounds".into())
                    } else {
                        Err("Index must be Int64".into())
                    }
                } else {
                    Err("Index access requires an array".into())
                }
            }
            HirExpression::If(if_expr) => {
                let cond = self.eval_expression(&if_expr.condition, env)?;
                let is_true = match cond {
                    Value::Bool(b) => b,
                    _ => return Err("If condition must be a Bool".into()),
                };

                if is_true {
                    self.eval_block(&if_expr.then_block, env)
                } else if let Some(else_branch) = &if_expr.else_branch {
                    self.eval_expression(else_branch, env)
                } else {
                    Ok(Value::Unit)
                }
            }
            HirExpression::Block(block) => self.eval_block(block, env),
            HirExpression::Tuple(_, exprs) => {
                let mut vals = Vec::new();
                for e in exprs {
                    vals.push(self.eval_expression(e, env)?);
                }
                Ok(Value::Tuple(vals))
            }
            HirExpression::Array(_, exprs) => {
                let mut vals = Vec::new();
                for e in exprs {
                    vals.push(self.eval_expression(e, env)?);
                }
                Ok(Value::Array(vals))
            }
            HirExpression::StructLiteral(s) => {
                let mut fields = HashMap::new();
                for (name, expr) in &s.fields {
                    let val = self.eval_expression(expr, env)?;
                    fields.insert(name.clone(), val);
                }
                Ok(Value::Struct(StructInstance {
                    name: s.name.clone(),
                    fields,
                }))
            }
            HirExpression::For(for_expr) => {
                // Return value from a for loop block is unit for now
                let iterable = self.eval_expression(&for_expr.iterable, env)?;
                let iter_values = match iterable {
                    Value::Array(arr) => arr,
                    _ => return Err("ForExpr iterable must be an Array".into()),
                };

                let mut last_val = Value::Unit;
                for item in iter_values {
                    env.push_scope();
                    env.insert(for_expr.binding.clone(), item);

                    if let Some(filter_expr) = &for_expr.filter {
                        let filter_res = self.eval_expression(filter_expr, env)?;
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

                    let val = self.eval_block(&for_expr.body, env)?;
                    last_val = val;
                    env.pop_scope();
                }
                Ok(last_val)
            }
            // Minimal implementations for others to make it compile
            HirExpression::Match(_) => Err("Match expressions are not fully supported yet".into()),
        }
    }

    fn eval_block(&mut self, block: &HirBlock, env: &mut RuntimeEnv) -> Result<Value, String> {
        env.push_scope();
        let mut last_val = Value::Unit;
        for stmt in &block.statements {
            let flow = self.eval_statement(stmt, env)?;
            match flow {
                ControlFlow::Value(v) => last_val = v,
                ControlFlow::Return(_v) => {
                    env.pop_scope();
                    // In a block, a return should bubble up to the function call.
                    // But our signature here returns Result<Value>.
                    // Wait, eval_block is called from eval_expression! So if it hits a Return statement,
                    // we actually have a slight problem. Expression evaluation doesn't bubble ControlFlow.
                    // Technically, Return inside an expression block should return from the function.
                    // To do this right, eval_expression would need to return ControlFlow.
                    // Let's just return to keep it simple, though properly it bubbles up.
                    // If we just return Ok(v), the return is local to the block, which is somewhat incorrect.
                    // For the scope of 'minimal interpreter', assuming statements inside blocks might need to break early.
                    return Err("Return inside expression block bubbling is unsupported".into());
                }
                ControlFlow::Break | ControlFlow::Continue => {
                    return Err(
                        "Break/Continue inside expression block bubbling is unsupported".into(),
                    );
                }
            }
        }
        env.pop_scope();
        Ok(last_val)
    }
}
