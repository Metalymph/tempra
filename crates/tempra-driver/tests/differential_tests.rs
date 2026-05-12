use tempra_driver::{run_source_hir, run_source_vm};
use tempra_interpreter::value::{Value, ValueResult, OptionValue};
use tempra_vm::value::VmValue;

#[derive(Debug, PartialEq, Clone)]
enum NormalizedValue {
    Unit,
    Bool(bool),
    Int(i64),
    UInt64(u64),
    UInt8(u8),
    Float(f64),
    String(String),
    Array(Vec<NormalizedValue>),
    Struct(String, Vec<(String, NormalizedValue)>),
    Enum(String, String, Vec<NormalizedValue>),
    Unsupported(String),
}

fn normalize_hir(val: Value) -> NormalizedValue {
    match val {
        Value::Unit => NormalizedValue::Unit,
        Value::Bool(b) => NormalizedValue::Bool(b),
        Value::Int64(i) => NormalizedValue::Int(i),
        Value::UInt8(u) => NormalizedValue::UInt8(u),
        Value::Float64(f) => NormalizedValue::Float(f),
        Value::String(s) => NormalizedValue::String(s),
        Value::Array(arr) => {
            let inner = arr.borrow().iter().map(|v| normalize_hir(v.clone())).collect();
            NormalizedValue::Array(inner)
        }
        Value::Slice(s) => {
            let inner = s.to_vec().into_iter().map(normalize_hir).collect();
            NormalizedValue::Array(inner)
        }
        Value::Struct(s) => {
            let mut fields: Vec<_> = s.fields.into_iter()
                .map(|(k, v)| (k, normalize_hir(v)))
                .collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));
            NormalizedValue::Struct(s.name, fields)
        }
        Value::Enum(e) => {
            let payloads = e.payloads.into_iter().map(normalize_hir).collect();
            NormalizedValue::Enum(e.enum_name, e.variant_name, payloads)
        }
        Value::Result(res) => match *res {
            ValueResult::Ok(v) => NormalizedValue::Enum("Result".into(), "Ok".into(), vec![normalize_hir(v)]),
            ValueResult::Err(v) => NormalizedValue::Enum("Result".into(), "Err".into(), vec![normalize_hir(v)]),
        },
        Value::Option(opt) => match opt {
            OptionValue::Some(v) => NormalizedValue::Enum("Option".into(), "Some".into(), vec![normalize_hir(*v)]),
            OptionValue::None => NormalizedValue::Enum("Option".into(), "None".into(), vec![]),
        },
        _ => NormalizedValue::Unsupported(format!("{:?}", val)),
    }
}

fn normalize_vm(val: VmValue) -> NormalizedValue {
    match val {
        VmValue::Unit => NormalizedValue::Unit,
        VmValue::Bool(b) => NormalizedValue::Bool(b),
        VmValue::Int64(i) => NormalizedValue::Int(i),
        VmValue::UInt64(u) => NormalizedValue::UInt64(u),
        VmValue::UInt8(u) => NormalizedValue::UInt8(u),
        VmValue::Float64(f) => NormalizedValue::Float(f),
        VmValue::String(s) => NormalizedValue::String(s),
        VmValue::Array(arr) => {
            let inner = arr.iter().map(|v| normalize_vm(v.clone())).collect();
            NormalizedValue::Array(inner)
        }
        VmValue::Slice { parent, start, end } => {
            let inner = parent[start..end].iter().map(|v| normalize_vm(v.clone())).collect();
            NormalizedValue::Array(inner)
        }
        VmValue::Struct { name, fields } => {
            let mut normalized_fields: Vec<_> = fields.into_iter()
                .map(|(k, v)| (k, normalize_vm(v)))
                .collect();
            normalized_fields.sort_by(|a, b| a.0.cmp(&b.0));
            NormalizedValue::Struct(name, normalized_fields)
        }
        VmValue::Enum(enum_name, variant_name, payloads) => {
            let normalized_payloads = payloads.into_iter().map(normalize_vm).collect();
            NormalizedValue::Enum(enum_name, variant_name, normalized_payloads)
        }
    }
}

fn assert_differential(source: &str) {
    let hir_res = match run_source_hir(source, &[]) {
        Ok(v) => v,
        Err(e) => panic!("HIR execution failed for source:\n{}\nError: {:?}", source, e),
    };
    let vm_res = match run_source_vm(source, &[]) {
        Ok(v) => v,
        Err(e) => panic!("VM execution failed for source:\n{}\nError: {:?}", source, e),
    };
    
    let normalized_hir = normalize_hir(hir_res);
    let normalized_vm = normalize_vm(vm_res);
    
    assert_eq!(normalized_hir, normalized_vm, "Result mismatch for source:\n{}", source);
}

#[test]
fn test_diff_arithmetic() {
    assert_differential("fn main() -> Int64 { return 1 + 2 * 3 }");
    assert_differential("fn main() -> Int64 { return (10 - 2) / 2 }");
}

#[test]
fn test_diff_bool() {
    assert_differential("fn main() -> Bool { return true && false || !false }");
    assert_differential("fn main() -> Bool { return 10 > 5 && 2 <= 2 }");
}

#[test]
fn test_diff_if_else() {
    let source = r#"
        fn main() -> Int64 {
            let x = 10
            if x > 5 {
                return 1
            } else {
                return 0
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_function_call() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 { return add(10, 20) }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_array_ops() {
    assert_differential("fn main() -> Int64 { let xs = [1, 2, 3]; return xs[1] }");
    assert_differential("fn main() -> Int64 { let xs = [1, 2, 3]; return xs.len() }");
    assert_differential("fn main() -> Slice(Int64) { let xs: [Int64] = [1, 2, 3, 4]; return xs[1..3] }");
}

#[test]
fn test_diff_struct_ops() {
    let source = r#"
        struct Point { var x: Int64, var y: Int64 }
        fn main() -> Int64 {
            var p = Point { x: 10, y: 20 }
            p.x = 30
            return p.x + p.y
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_methods() {
    let source = r#"
        struct User { age: Int64 }
        extend User {
            fn get_age(self) -> Int64 { return self.age }
        }
        fn main() -> Int64 {
            let u = User { age: 42 }
            return u.get_age()
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_for_expr() {
    let source = r#"
        fn main() -> [Int64] {
            let xs: [Int64] = [1, 2, 3]
            return for x in xs { x * 2 }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_string_basics() {
    assert_differential("fn main() -> Int64 { return \"hello\".len() }");
    assert_differential("fn main() -> Int64 { return \"hello\".size() }");
}

#[test]
fn test_diff_result_question() {
    let source = r#"
        fn fail() -> Result(Int64, String) { return Err("boom") }
        fn ok() -> Result(Int64, String) { return Ok(42) }
        
        fn test_ok() -> Result(Int64, String) {
            let x = ok()?
            return Ok(x + 1)
        }
        
        fn main() -> Result(Int64, String) {
            return test_ok()
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_is_narrowing() {
    let source = r#"
        fn main() -> Int64 {
            let x: Option(Int64) = Some(42)
            if x is Some(val) {
                return val
            } else {
                return 0
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_unsupported_match() {
    let source = r#"
        fn main() -> Int64 {
            let x: Option(Int64) = Some(1)
            let zero: Int64 = 0
            return match x {
                Some(v) => v
                None => zero
            }
        }
    "#;
    let hir_res = run_source_hir(source, &[]);
    if let Err(e) = hir_res {
        panic!("HIR failed for match source:\n{}\nError: {:?}", source, e);
    }
    
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for match");
    // Should be an unsupported error
    let err = format!("{:?}", vm_res.err().unwrap()).to_lowercase();
    assert!(err.contains("unsupported"), "Unexpected VM error: {}", err);
}

#[test]
fn test_unsupported_closure() {
    let source = r#"
        fn main() -> Int64 {
            let f = fn(x: Int64) -> Int64 { x + 1 }
            return f(10)
        }
    "#;
    let hir_res = run_source_hir(source, &[]);
    assert!(hir_res.is_ok(), "HIR should support closures: {:?}", hir_res.err());
    
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for closures");
}

#[test]
fn test_unsupported_string_iteration() {
    let source = r#"
        fn main() -> Int64 {
            let s = "abc"
            var count = 0
            for ch in s {
                count = count + 1
            }
            return count
        }
    "#;
    let hir_res = run_source_hir(source, &[]);
    assert!(hir_res.is_ok(), "HIR should support string iteration: {:?}", hir_res.err());
    
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for string iteration");
    assert!(format!("{:?}", vm_res.err().unwrap()).contains("supports Array/Slice"));
}
