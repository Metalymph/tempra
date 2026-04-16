# Fero Stdlib Contribution Tutorial

This guide explains how to add a new builtin module and API to the Fero interpreter.

## Overview

Adding a new stdlib API involves changes in four main areas:
1. **HIR Representation**: (Optional) if adding new expression types.
2. **Type Checker**: Define the type rules for the new API.
3. **Interpreter**: Implement the runtime behavior.
4. **Tests & Examples**: Verify the implementation.

---

## 1. Type Checker (`crates/fero-typeck/src/check_expr.rs`)

Fero uses explicit dispatch for builtin namespaces like `fs.*`, `os.*`, and `env.*`.

### Step 1.1: Create a checking function
Create a function to validate the arguments and return type of your module's operations.

```rust
fn check_my_module_builtin(ctx: &mut TypeContext, name: &str, args: &[HirCallArgument], span: Span) -> Type {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&HirExpression> {
        // Helper to find arg by position or name
    };

    match name {
        "my_func" => {
            if args.len() != 1 {
                ctx.error("my_module.my_func expects 1 argument", span);
            }
            // Check arg0 is String
            if let Some(e) = get_arg(0, "arg") {
                let ty = check_expression(ctx, e);
                if ty != Type::String && ty != Type::Error {
                    ctx.error("Expected String", e.span());
                }
            }
            Type::Result(Box::new(Type::Int), Box::new(Type::BuiltinError))
        }
        _ => {
            ctx.error(format!("Unknown operation '{}'", name), span);
            Type::Error
        }
    }
}
```

### Step 1.2: Register in `check_call`
Add your namespace check to the `check_call` logic.

```rust
if let HirExpression::Member(m) = &*c.callee {
    if let HirExpression::Name(_, ref obj_name) = *m.object {
        if obj_name == "my_module" {
            return self::check_my_module_builtin(ctx, &m.field, &c.args, c.span);
        }
    }
}
```

---

## 2. Interpreter (`crates/fero-interpreter/src/eval_expr.rs`)

### Step 2.1: Implement evaluation logic
Create a function in the `Interpreter` struct to handle the runtime execution.

```rust
fn eval_my_module_builtin(&mut self, name: &str, positional_args: &[Value], named_args: &HashMap<String, Value>) -> Result<ControlFlow, String> {
    let get_arg = |idx: usize, arg_name: &str| -> Option<&Value> { ... };

    match name {
        "my_func" => {
            let arg = get_arg(0, "arg").ok_or("Missing arg")?;
            if let Value::String(s) = arg {
                let res = do_something(s);
                Ok(ControlFlow::Value(Value::Result(Box::new(ValueResult::Ok(Value::Int64(res))))))
            } else {
                Err("Expected string".into())
            }
        }
        _ => Err(format!("Unknown operation {}", name))
    }
}
```

### Step 2.2: Register in `eval_expression`
Update the `HirExpression::Call` match arm to dispatch to your module.

```rust
if obj_name == "my_module" {
    let mut args = Vec::new(); // Collect evaluated args...
    return self.eval_my_module_builtin(&member.field, &args, &named_args);
}
```

---

## 3. Adding Methods to Existing Types

To add a method to a type (e.g., `String.trim()`), add logic to `check_call` and `eval_expression` that checks the type of the `object` in a member call.

1. **Typeck**: `if object_ty == Type::String { return check_string_method(...); }`
2. **Interpreter**: `if let Value::String(s) = obj { return eval_string_method(...); }`

---

## 4. Verification

1. **Type Checker Tests**: Add a test case in `crates/fero-typeck/tests/typeck_tests.rs` using `expect_ok` and `expect_err`.
2. **Interpreter Tests**: Add an execution test in `crates/fero-interpreter/tests/interpreter_tests.rs`.
3. **Example**: Add a new `.fero` file in `examples/` to demonstrate usage.
