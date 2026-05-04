# Tempra Stdlib Contribution Tutorial

This guide explains how to add a new builtin module and API to the Tempra interpreter.

## Overview

Adding a new stdlib API involves changes in four main areas:

1. **HIR Representation**: (Optional) if adding new expression types.
2. **Type Checker**: Define the type rules for the new API.
3. **Interpreter**: Implement the runtime behavior.
4. **Tests & Examples**: Verify the implementation.

---

## 1. Type Checker (`crates/tempra-typeck/src/check_expr.rs`)

Tempra uses explicit dispatch for builtin namespaces like `fs.*`, `os.*`, and `env.*`.

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

### Step 1.2: Register in `resolve_callee` and `check_call`

First, ensure your namespace is recognized as a root in `resolve_name`:

```rust
if name == "my_module" {
    return Ok(ResolvedName::Namespace(name.to_string()));
}
```

Then, handle the call dispatch in `check_call`:

```rust
ResolvedCallee::Namespace(ns, method) => {
    if let HirExpression::Call(ref mut c) = expr {
        match ns.as_str() {
            "my_module" => return check_my_module_builtin(ctx, &method, &mut c.args, c.span),
            // ...
        }
    }
}
```

---

## 2. Interpreter (`crates/tempra-interpreter/src/eval_expr.rs`)

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
   - Use `Type::Array(_)` or `Type::Slice(_)` for collection methods.
2. **Interpreter**: `if let Value::String(s) = obj { return eval_string_method(...); }`
   - For strings, implement methods based on **Unicode scalar values** for `len()` or **UTF-8 bytes** for `size()`.
   - Iteration over strings yields single-scalar `Value::String`.

---

## 4. Verification

1. **Type Checker Tests**: Add a test case in `crates/tempra-typeck/tests/typeck_tests.rs` using `expect_ok` and `expect_err`.
2. **Interpreter Tests**: Add an execution test in `crates/tempra-interpreter/tests/interpreter_tests.rs`.
3. **Example**: Add a new `.tempra` file in `examples/` to demonstrate usage.

---

## 5. Byte Slice Model

`String.bytes()` returns a `Value::Slice` backed by `SliceParent::Bytes(Rc<Vec<u8>>)`.

This is distinct from `SliceParent::Values(Rc<Vec<Value>>)` used for array slices.

### Type annotation returned

```
Type::Slice(Box::new(Type::UInt8))
```

### Runtime value

```rust
Value::Slice(SliceValue {
    parent: SliceParent::Bytes(Rc::new(s.as_bytes().to_vec())),
    start: 0,
    end: s.len(),
})
```

Byte slices support `.len()`, `s[i]` (yields `Value::Int64`), `s[a..b]`, and `for b in s` iteration — identical to value slices.

> Do NOT expose implicit byte-to-string conversions. Byte access must remain explicit.

---

## 6. Array/Slice Parity Invariant

All read-only operations must behave identically for `Array(T)` and `Slice(T)`:

| Operation       | Array       | Slice       |
|-----------------|-------------|-------------|
| `.len()`        | ✅          | ✅          |
| `xs[i]`         | ✅          | ✅          |
| `xs[a..b]`      | `Slice(T)`  | `Slice(T)`  |
| `for x in xs`   | ✅          | ✅          |
| `print/dbg/fmt` | `[1, 2, 3]` | `[1, 2, 3]` |

Slices are always read-only. Any mutation through a slice must be rejected.
