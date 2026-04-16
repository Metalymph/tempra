use crate::control_flow::ControlFlow;
use crate::env::RuntimeEnv;
use crate::interpreter::Interpreter;
use crate::value::{StructInstance, Value};
use fero_hir::*;
use std::collections::HashMap;

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
        Ok(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Command '{}' failed with exit code {}", c.program, res.exit_code))))))
    } else {
        Ok(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::CommandResult(res)))))
    }
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
                        let res_v = run_command(cmd_left)?;
                        let success = match res_v {
                            Value::Result(ref r) => match r.as_ref() {
                                crate::value::ValueResult::Ok(Value::CommandResult(cr)) => cr.success,
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
                        let res_v = run_command(cmd_left)?;
                        let success = match res_v {
                            Value::Result(ref r) => match r.as_ref() {
                                crate::value::ValueResult::Ok(Value::CommandResult(cr)) => cr.success,
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

                    match &*binary.left {
                        HirExpression::Name(_, name) => {
                            env.insert(name.clone(), right.clone());
                            return Ok(ControlFlow::Value(right));
                        }
                        HirExpression::Member(m) => {
                            // Target: m.object . m.field
                            // We only support first-level struct member assignment for now
                            if let HirExpression::Name(_, obj_name) = &*m.object {
                                let mut obj_val = env.get(obj_name).ok_or_else(|| format!("Variable {} not found", obj_name))?;
                                if let Value::Struct(ref mut s) = obj_val {
                                    s.fields.insert(m.field.clone(), right.clone());
                                    // Re-insert since Value is not trivially mutable through the environment reference
                                    env.insert(obj_name.clone(), obj_val);
                                    return Ok(ControlFlow::Value(right));
                                } else {
                                    return Err("Member assignment target is not a struct".into());
                                }
                            } else {
                                return Err("Complex member assignment not yet supported".into());
                            }
                        }
                        _ => {
                            return Err("Invalid assignment target".into());
                        }
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
                                            ControlFlow::Value(v) => { named_args.insert(n.name.clone(), v); }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_fs_builtin(&member.field, &args, &named_args);
                        }
                        if obj_name == "env" {
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
                                            ControlFlow::Value(v) => { named_args.insert(n.name.clone(), v); }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_env_builtin(&member.field, &args, &named_args);
                        }
                        if obj_name == "os" {
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
                                            ControlFlow::Value(v) => { named_args.insert(n.name.clone(), v); }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_os_builtin(&member.field, &args, &named_args);
                        }
                    }
                        let obj = match self.eval_expression(&member.object, env)? {
                            ControlFlow::Value(v) => v,
                            cf => return Ok(cf),
                        };
                        if let Value::String(s) = obj {
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
                                            ControlFlow::Value(v) => { named_args.insert(n.name.clone(), v); }
                                            cf => return Ok(cf),
                                        }
                                    }
                                }
                            }
                            return self.eval_string_method(&s, &member.field, &args, &named_args).map(ControlFlow::Value);
                        }
                        if let Value::Array(arr) = obj {
                            if member.field == "len" {
                                if !call.args.is_empty() {
                                    return Err("Method 'len' expects 0 arguments".into());
                                }
                                return Ok(ControlFlow::Value(Value::Int64(arr.len() as i64)));
                            } else {
                                return Err(format!("Unknown method '{}' on Array", member.field));
                            }
                        }
                        if let Value::Command(cmd) = obj {
                            if member.field == "run" {
                                if !call.args.is_empty() {
                                    return Err("Method 'run' expects 0 arguments".into());
                                }
                                return run_command(&cmd).map(ControlFlow::Value);
                            } else {
                                return Err(format!("Unknown method '{}' on Command", member.field));
                            }
                        } else {
                            return Err(format!("Method call on unsupported type: {:?}", obj));
                        }
                    }

                let mut positional_values = Vec::new();
                let mut named_values = HashMap::new();
                for arg in &call.args {
                    match arg {
                        HirCallArgument::Positional(e) => {
                            match self.eval_expression(e, env)? {
                                ControlFlow::Value(v) => positional_values.push(v),
                                cf => return Ok(cf),
                            }
                        }
                        HirCallArgument::Named(n) => {
                            match self.eval_expression(&n.value, env)? {
                                ControlFlow::Value(v) => { named_values.insert(n.name.clone(), v); }
                                cf => return Ok(cf),
                            }
                        }
                    }
                }

                if let HirExpression::Name(_, name) = &*call.callee {
                    // Builtins
                    match name.as_str() {
                        "exit" => {
                            let val = positional_values.get(0).or_else(|| named_values.get("code")).ok_or("Missing argument 'code' for exit")?;
                            if let Value::Int64(code) = val {
                                return Ok(ControlFlow::Exit(*code as i32));
                            } else {
                                return Err("exit requires an Int64 argument".into());
                            }
                        }
                        "print" => {
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
                            let val = positional_values.get(0).or_else(|| named_values.get("message")).ok_or("Missing argument 'message' for crash")?;
                            return Ok(ControlFlow::Crash(value_to_string(val)));
                        }
                        "fmt" => {
                            let mut separator = "";
                            if let Some(Value::String(sep)) = named_values.get("separator") {
                                separator = sep;
                            }
                            let s = positional_values.iter()
                                .map(|v| value_to_string(v))
                                .collect::<Vec<_>>()
                                .join(separator);
                            return Ok(ControlFlow::Value(Value::String(s)));
                        }
                        "read_line" => {
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
                                    Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(input))))))
                                }
                                Err(e) => {
                                    Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to read stdin: {}", e)))))))
                                }
                            }
                        }
                        "cmd" => {
                            let program = match positional_values.get(0).or_else(|| named_values.get("program")) {
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
                            
                            return Ok(ControlFlow::Value(Value::Command(crate::value::CommandValue { 
                                program, 
                                args: cmd_args,
                                cwd,
                                check,
                            })));
                        }
                        "Ok" => {
                            let val = positional_values.get(0).or_else(|| named_values.get("value")).ok_or("Missing argument for Ok")?;
                            return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(val.clone())))));
                        }
                        "Err" => {
                            let val = positional_values.get(0).or_else(|| named_values.get("error")).ok_or("Missing argument for Err")?;
                            return Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(val.clone())))));
                        }
                        "expect" => {
                            let cond_val = positional_values.get(0).or_else(|| named_values.get("condition")).ok_or("Missing argument 'condition' for expect")?;
                            let msg_val = positional_values.get(1).or_else(|| named_values.get("message"));

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
                            let val = positional_values.get(0).or_else(|| named_values.get("value")).ok_or("Missing argument for dbg")?;
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

                    // User function
                    let func = self
                        .functions
                        .get(name)
                        .cloned()
                        .ok_or_else(|| format!("Function {} not found", name))?;
                    
                    let mut final_args = vec![None; func.parameters.len()];
                    for (i, v) in positional_values.into_iter().enumerate() {
                        if i < final_args.len() {
                            final_args[i] = Some(v);
                        } else {
                            return Err(format!("Too many arguments for {}", name));
                        }
                    }
                    for (n, v) in named_values {
                        if let Some(idx) = func.parameters.iter().position(|p| p.name == n) {
                            if final_args[idx].is_some() {
                                return Err(format!("Duplicate argument for parameter {}", n));
                            }
                            final_args[idx] = Some(v);
                        } else {
                            return Err(format!("Unknown named argument {} for function {}", n, name));
                        }
                    }
                    
                    let mut args = Vec::new();
                    for (i, opt_v) in final_args.into_iter().enumerate() {
                        match opt_v {
                            Some(v) => args.push(v),
                            None => return Err(format!("Missing argument for parameter {}", func.parameters[i].name)),
                        }
                    }
                    
                    return self.call_function(&func, args);
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
                    Value::Command(c) => match member.field.as_str() {
                        "program" => Ok(ControlFlow::Value(Value::String(c.program.clone()))),
                        "args" => Ok(ControlFlow::Value(Value::Array(c.args.iter().map(|s| Value::String(s.clone())).collect()))),
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

                if let (Value::Array(arr), Value::Int64(start), Value::Int64(end)) = (obj, start_val, end_val) {
                    let len = arr.len() as i64;
                    if start < 0 || end < 0 || start > end || end > len {
                        return Ok(ControlFlow::Crash(format!("Slice bounds out of range: [{}..{}] for length {}", start, end, len)));
                    }
                    let sub = arr[start as usize..end as usize].to_vec();
                    Ok(ControlFlow::Value(Value::Array(sub)))
                } else {
                    Err("Slice requires an array and two integer bounds".into())
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

                // Fill defaults
                if let Some(def) = self.structs.get(&s.name).cloned() {
                    for field in &def.fields {
                        if !fields.contains_key(&field.name) {
                            if let Some(default_expr) = &field.default_value {
                                // Evaluate default expression in the CURRENT environment
                                match self.eval_expression(default_expr, env)? {
                                    ControlFlow::Value(v) => { fields.insert(field.name.clone(), v); },
                                    cf => return Ok(cf),
                                }
                            }
                        }
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
                            Ok(ControlFlow::Crash(format!("Unwrap failed: {}", v)))
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

    fn eval_fs_builtin(&mut self, name: &str, positional_args: &[Value], named_args: &HashMap<String, Value>) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            positional_args.get(idx).or_else(|| named_args.get(arg_name))
        };

        match name {
            "read" => {
                let path_val = get_arg(0, "path").ok_or_else(|| format!("'fs.read' missing argument 'path'"))?;
                if let Value::String(path) = path_val {
                    return match std::fs::read_to_string(path) {
                        Ok(text) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(text)))))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to read '{}': {}", path, e))))))),
                    }
                } else {
                    return Err("'fs.read' requires a String for 'path'".into());
                }
            }
            "write" | "append" => {
                let path_val = get_arg(0, "path").ok_or_else(|| format!("'fs.{}' missing argument 'path'", name))?;
                let text_val = get_arg(1, "text").ok_or_else(|| format!("'fs.{}' missing argument 'text'", name))?;

                if let (Value::String(path), Value::String(text)) = (path_val, text_val) {
                    if name == "write" {
                        return match std::fs::write(path, text) {
                            Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                            Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to write to '{}': {}", path, e))))))),
                        }
                    } else {
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
                    }
                } else {
                    return Err(format!("'fs.{}' requires two String arguments", name));
                }
            }
            "exists" => {
                let path_val = get_arg(0, "path").ok_or_else(|| format!("'fs.exists' missing argument 'path'"))?;
                if let Value::String(path) = path_val {
                    let exists = std::path::Path::new(path).exists();
                    Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Bool(exists))))))
                } else {
                    return Err("'fs.exists' requires a String for 'path'".into());
                }
            }
            "remove" | "mkdir" | "list_dir" => {
                let path_val = get_arg(0, "path").ok_or_else(|| format!("'fs.{}' missing argument 'path'", name))?;
                if let Value::String(path) = path_val {
                    match name {
                        "remove" => {
                            match std::fs::remove_file(path) {
                                Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                                Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to remove '{}': {}", path, e))))))),
                            }
                        }
                        "mkdir" => {
                            match std::fs::create_dir(path) {
                                Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                                Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to create directory '{}': {}", path, e))))))),
                            }
                        }
                        "list_dir" => {
                            match std::fs::read_dir(path) {
                                Ok(entries) => {
                                    let mut names = Vec::new();
                                    for entry in entries {
                                        if let Ok(entry) = entry {
                                            names.push(Value::String(entry.file_name().to_string_lossy().to_string()));
                                        }
                                    }
                                    Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Array(names))))))
                                }
                                Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to read directory '{}': {}", path, e))))))),
                            }
                        }
                        _ => unreachable!(),
                    }
                } else {
                    return Err(format!("'fs.{}' requires a String for 'path'", name));
                }
            }
            "rename" | "copy" => {
                let from_val = get_arg(0, "from").ok_or_else(|| format!("'fs.{}' missing argument 'from'", name))?;
                let to_val = get_arg(1, "to").ok_or_else(|| format!("'fs.{}' missing argument 'to'", name))?;
                if let (Value::String(from), Value::String(to)) = (from_val, to_val) {
                    let res = if name == "rename" {
                        std::fs::rename(from, to).map(|_| ())
                    } else {
                        std::fs::copy(from, to).map(|_| ())
                    };
                    match res {
                        Ok(_) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit))))),
                        Err(e) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to {} '{}' to '{}': {}", name, from, to, e))))))),
                    }
                } else {
                    return Err(format!("'fs.{}' requires two String arguments", name));
                }
            }
            _ => Err(format!("Unknown filesystem operation 'fs.{}'", name)),
        }
    }

    fn eval_env_builtin(&mut self, name: &str, positional_args: &[Value], named_args: &HashMap<String, Value>) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            positional_args.get(idx).or_else(|| named_args.get(arg_name))
        };

        match name {
            "get" => {
                let name_val = get_arg(0, "name").ok_or_else(|| format!("'env.get' missing argument 'name'"))?;
                if let Value::String(key) = name_val {
                    match std::env::var(key) {
                        Ok(val) => Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(val)))))),
                        Err(std::env::VarError::NotPresent) => {
                            Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Environment variable '{}' not found", key)))))))
                        }
                        Err(e) => {
                            Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to get environment variable '{}': {}", key, e)))))))
                        }
                    }
                } else {
                    return Err("'env.get' requires a String for 'name'".into());
                }
            }
            _ => Err(format!("Unknown environment operation 'env.{}'", name)),
        }
    }

    fn eval_os_builtin(&self, name: &str, args: &[Value], named_args: &HashMap<String, Value>) -> Result<ControlFlow, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            args.get(idx).or_else(|| named_args.get(arg_name))
        };

        match name {
            "args" => {
                let val_args = self.args.iter().map(|s| Value::String(s.clone())).collect();
                Ok(ControlFlow::Value(Value::Array(val_args)))
            }
            "cwd" => {
                match std::env::current_dir() {
                    Ok(path) => {
                        let path_str = path.to_string_lossy().to_string();
                        Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::String(path_str))))))
                    }
                    Err(e) => {
                        Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to get cwd: {}", e)))))))
                    }
                }
            }
            "chdir" => {
                let path_val = get_arg(0, "path").ok_or("Missing argument 'path' for os.chdir")?;
                if let Value::String(path_str) = path_val {
                    match std::env::set_current_dir(path_str) {
                        Ok(_) => {
                            Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Ok(Value::Unit)))))
                        }
                        Err(e) => {
                            Ok(ControlFlow::Value(Value::Result(Box::new(crate::value::ValueResult::Err(Value::Error(format!("Failed to chdir: {}", e)))))))
                        }
                    }
                } else {
                    Err("os.chdir requires a String argument".into())
                }
            }
            _ => Err(format!("Unknown OS operation 'os.{}'", name)),
        }
    }


    fn eval_string_method(&self, s: &str, name: &str, args: &[Value], named_args: &HashMap<String, Value>) -> Result<Value, String> {
        let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> {
            args.get(idx).or_else(|| named_args.get(arg_name))
        };

        match name {
            "len" => Ok(Value::Int64(s.len() as i64)),
            "trim" => Ok(Value::String(s.trim().to_string())),
            "trim_start" => Ok(Value::String(s.trim_start().to_string())),
            "trim_end" => Ok(Value::String(s.trim_end().to_string())),
            "to_lower" => Ok(Value::String(s.to_lowercase())),
            "to_upper" => Ok(Value::String(s.to_uppercase())),
            "contains" => {
                let pattern = get_arg(0, "pattern").or_else(|| get_arg(0, "substr")).ok_or("Missing argument for contains")?;
                if let Value::String(p) = pattern {
                    Ok(Value::Bool(s.contains(p)))
                } else {
                    Err("contains expects String".into())
                }
            }
            "starts_with" => {
                let prefix = get_arg(0, "prefix").or_else(|| get_arg(0, "pattern")).ok_or("Missing argument for starts_with")?;
                if let Value::String(p) = prefix {
                    Ok(Value::Bool(s.starts_with(p)))
                } else {
                    Err("starts_with expects String".into())
                }
            }
            "ends_with" => {
                let suffix = get_arg(0, "suffix").or_else(|| get_arg(0, "pattern")).ok_or("Missing argument for ends_with")?;
                if let Value::String(p) = suffix {
                    Ok(Value::Bool(s.ends_with(p)))
                } else {
                    Err("ends_with expects String".into())
                }
            }
            "split" => {
                let sep = get_arg(0, "separator").ok_or("Missing argument 'separator' for split")?;
                if let Value::String(p) = sep {
                    let parts = s.split(p).map(|part| Value::String(part.to_string())).collect();
                    Ok(Value::Array(parts))
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
}
