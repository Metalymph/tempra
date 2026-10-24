use tempra_driver::{run_source_hir, run_source_vm};
use tempra_interpreter::value::{OptionValue, Value, ValueResult};
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
    Tuple(Vec<NormalizedValue>),
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
            let inner = arr
                .borrow()
                .iter()
                .map(|v| normalize_hir(v.clone()))
                .collect();
            NormalizedValue::Array(inner)
        }
        Value::Slice(s) => {
            let inner = s.to_vec().into_iter().map(normalize_hir).collect();
            NormalizedValue::Array(inner)
        }
        Value::Tuple(elements) => {
            let inner = elements.into_iter().map(normalize_hir).collect();
            NormalizedValue::Tuple(inner)
        }
        Value::Struct(s) => {
            let mut fields: Vec<_> = s
                .fields
                .into_iter()
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
            ValueResult::Ok(v) => {
                NormalizedValue::Enum("Result".into(), "Ok".into(), vec![normalize_hir(v)])
            }
            ValueResult::Err(v) => {
                NormalizedValue::Enum("Result".into(), "Err".into(), vec![normalize_hir(v)])
            }
        },
        Value::Option(opt) => match opt {
            OptionValue::Some(v) => {
                NormalizedValue::Enum("Option".into(), "Some".into(), vec![normalize_hir(*v)])
            }
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
            let inner = parent[start..end]
                .iter()
                .map(|v| normalize_vm(v.clone()))
                .collect();
            NormalizedValue::Array(inner)
        }
        VmValue::Tuple(elements) => {
            let inner = elements.into_iter().map(normalize_vm).collect();
            NormalizedValue::Tuple(inner)
        }
        VmValue::Struct { name, fields } => {
            let mut normalized_fields: Vec<_> = fields
                .into_iter()
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
        Err(e) => panic!(
            "HIR execution failed for source:\n{}\nError: {:?}",
            source, e
        ),
    };
    let vm_res = match run_source_vm(source, &[]) {
        Ok(v) => v,
        Err(e) => panic!(
            "VM execution failed for source:\n{}\nError: {:?}",
            source, e
        ),
    };

    let normalized_hir = normalize_hir(hir_res);
    let normalized_vm = normalize_vm(vm_res);

    assert_eq!(
        normalized_hir, normalized_vm,
        "Result mismatch for source:\n{}",
        source
    );
}

/// Checked Int64 failures surface through different error types in the HIR and
/// VM pipelines (`DriverError::RuntimeError` versus
/// `DriverError::VmError("Int64Arithmetic(...)")`). The differential gate
/// therefore asserts that both pipelines fail in the same semantic category
/// rather than demanding byte-identical diagnostics.
fn assert_differential_int64_error(source: &str, hir_message: &str, vm_variant: &str) {
    let hir_error = match run_source_hir(source, &[]) {
        Ok(value) => panic!(
            "HIR unexpectedly succeeded with {:?} for source:\n{}",
            value, source
        ),
        Err(error) => format!("{:?}", error),
    };
    let vm_error = match run_source_vm(source, &[]) {
        Ok(value) => panic!(
            "VM unexpectedly succeeded with {:?} for source:\n{}",
            value, source
        ),
        Err(error) => format!("{:?}", error),
    };

    assert!(
        hir_error.contains(hir_message),
        "HIR error {:?} did not contain {:?} for source:\n{}",
        hir_error,
        hir_message,
        source
    );
    assert!(
        vm_error.contains(vm_variant),
        "VM error {:?} did not contain {:?} for source:\n{}",
        vm_error,
        vm_variant,
        source
    );
}

#[test]
fn test_diff_arithmetic() {
    assert_differential("fn main() -> Int64 { return 1 + 2 * 3 }");
    assert_differential("fn main() -> Int64 { return (10 - 2) / 2 }");
}

#[test]
fn test_diff_int64_arithmetic_errors() {
    // Addition overflow.
    assert_differential_int64_error(
        "fn main() -> Int64 { return 9223372036854775807 + 1 }",
        "integer overflow",
        "Int64Arithmetic(Overflow)",
    );
    // Subtraction overflow.
    assert_differential_int64_error(
        "fn main() -> Int64 { return (0 - 9223372036854775807) - 2 }",
        "integer overflow",
        "Int64Arithmetic(Overflow)",
    );
    // Multiplication overflow.
    assert_differential_int64_error(
        "fn main() -> Int64 { return 9223372036854775807 * 2 }",
        "integer overflow",
        "Int64Arithmetic(Overflow)",
    );
    // Integer division by zero.
    assert_differential_int64_error(
        "fn main() -> Int64 { return 1 / 0 }",
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
    // i64::MIN / -1 overflow.
    assert_differential_int64_error(
        "fn main() -> Int64 { return (0 - 9223372036854775807 - 1) / (0 - 1) }",
        "integer overflow",
        "Int64Arithmetic(Overflow)",
    );
}

#[test]
fn test_diff_bool() {
    assert_differential("fn main() -> Bool { return true && false || !false }");
    assert_differential("fn main() -> Bool { return 10 > 5 && 2 <= 2 }");
}

#[test]
fn test_diff_logical_short_circuit_skips_rhs() {
    assert_differential("fn main() -> Bool { return false && (1 / 0 == 0) }");
    assert_differential("fn main() -> Bool { return true || (1 / 0 == 0) }");
}

#[test]
fn test_diff_logical_short_circuit_requires_rhs() {
    assert_differential_int64_error(
        "fn main() -> Bool { return true && (1 / 0 == 0) }",
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
    assert_differential_int64_error(
        "fn main() -> Bool { return false || (1 / 0 == 0) }",
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
}

#[test]
fn test_diff_nested_logical_short_circuit() {
    assert_differential(
        "fn main() -> Bool { return (false && (1 / 0 == 0)) || (true && (2 > 1)) }",
    );
    assert_differential("fn main() -> Bool { return false && (true || (1 / 0 == 0)) }");
}

#[test]
fn test_diff_logical_left_operand_is_evaluated_first() {
    assert_differential_int64_error(
        "fn main() -> Bool { return (1 / 0 == 0) && false }",
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
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
fn test_diff_call_arguments_are_evaluated_in_source_order() {
    let first_fails = r#"
        fn combine(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            return combine(9223372036854775807 + 1, 1 / 0)
        }
    "#;
    assert_differential_int64_error(first_fails, "integer overflow", "Int64Arithmetic(Overflow)");

    let second_fails = r#"
        fn combine(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            return combine(1 + 1, 1 / 0)
        }
    "#;
    assert_differential_int64_error(
        second_fails,
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
}

#[test]
fn test_diff_binary_lhs_is_evaluated_before_rhs() {
    assert_differential_int64_error(
        "fn main() -> Int64 { return (9223372036854775807 + 1) + (1 / 0) }",
        "integer overflow",
        "Int64Arithmetic(Overflow)",
    );
}

#[test]
fn test_diff_unselected_if_branch_is_not_executed() {
    let source = r#"
        fn selected_true(flag: Bool) -> Int64 {
            if flag {
                return 7
            } else {
                return 1 / 0
            }
        }

        fn selected_false(flag: Bool) -> Int64 {
            if flag {
                return 1 / 0
            } else {
                return 9
            }
        }

        fn main() -> Int64 {
            return selected_true(true) + selected_false(false)
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_nested_calls_preserve_first_failure_order() {
    let inner_first = r#"
        fn nested_add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            return nested_add(nested_add(9223372036854775807, 1), 1 / 0)
        }
    "#;
    assert_differential_int64_error(inner_first, "integer overflow", "Int64Arithmetic(Overflow)");

    let outer_first = r#"
        fn nested_add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            return nested_add(1 / 0, nested_add(9223372036854775807, 1))
        }
    "#;
    assert_differential_int64_error(
        outer_first,
        "integer division by zero",
        "Int64Arithmetic(DivisionByZero)",
    );
}

#[test]
fn test_diff_unreachable_continuation_after_terminating_if() {
    let source = r#"
        fn choose(flag: Bool) -> Int64 {
            if flag {
                return 11
            } else {
                return 22
            }
            return 1 / 0
        }

        fn main() -> Int64 {
            return choose(true) + choose(false)
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_nested_terminating_branch_values() {
    let source = r#"
        fn bucket(n: Int64) -> Int64 {
            if n < 10 {
                if n < 0 { return 0 } else { return 1 }
            } else {
                if n < 100 { return 2 } else { return 3 }
            }
        }

        fn main() -> Int64 {
            return bucket(-1) + bucket(5) + bucket(50) + bucket(500)
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_function_call_chain() {
    let source = r#"
        fn level3(a: Int64) -> Int64 { return a + 3 }
        fn level2(a: Int64) -> Int64 { return level3(a) + 2 }
        fn level1(a: Int64) -> Int64 { return level2(a) + 1 }
        fn main() -> Int64 { return level1(1) + level1(2) }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_nested_terminating_branches() {
    let source = r#"
        fn classify(n: Int64) -> Int64 {
            if n < 0 {
                return -1
            } else {
                if n == 0 {
                    return 0
                } else {
                    if n > 100 {
                        return 2
                    } else {
                        return 1
                    }
                }
            }
        }
        fn main() -> Int64 {
            return classify(-10) + classify(0) + classify(5) + classify(50) + classify(150)
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_array_ops() {
    assert_differential("fn main() -> Int64 { let xs = [1, 2, 3]; return xs[1] }");
    assert_differential("fn main() -> Int64 { let xs = [1, 2, 3]; return xs.len() }");
    assert_differential(
        "fn main() -> Slice(Int64) { let xs: [Int64] = [1, 2, 3, 4]; return xs[1..3] }",
    );
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
fn test_diff_match_bool() {
    let source = r#"
        fn main() -> Int64 {
            let x = true
            return match x {
                true => 1
                false => 0
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_match_int() {
    let source = r#"
        fn main() -> Int64 {
            let x = 2
            return match x {
                1 => 10
                2 => 20
                _ => 0
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_match_option() {
    let source = r#"
        fn main() -> Int64 {
            let x: Option(Int64) = Some(41)
            let zero: Int64 = 0
            return match x {
                Some(v) => v + 1
                None => zero
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_match_result() {
    let source = r#"
        fn main() -> Int64 {
            let r: Result(Int64, String) = Err("bad")
            let zero: Int64 = 0
            return match r {
                Ok(v) => v
                Err(e) => zero
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_diff_match_wildcard_binding() {
    let source = r#"
        fn main() -> Int64 {
            let x = 100
            return match x {
                1 => 1
                other => other
            }
        }
    "#;
    assert_differential(source);
}

#[test]
fn test_match_non_exhaustive_error() {
    let source = r#"
        fn main() -> Int64 {
            let x = 1
            return match x {
                1 => 10
            }
        }
    "#;
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err());
    let err = format!("{:?}", vm_res.err().unwrap());
    assert!(
        err.contains("non-exhaustive match"),
        "Unexpected error: {}",
        err
    );
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
    assert!(
        hir_res.is_ok(),
        "HIR should support closures: {:?}",
        hir_res.err()
    );

    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for closures");
}

#[test]
fn test_diff_string_for_in() {
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
    let vm_res = run_source_vm(source, &[]);
    assert_eq!(vm_res.unwrap(), VmValue::Int64(3));
}

#[test]
fn test_unsupported_match_guard() {
    let source = r#"
        fn main() -> Int64 {
            let x = 1
            return match x {
                1 if true => 10,
                _ => 0
            }
        }
    "#;
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for match guards");
    let err = format!("{:?}", vm_res.err().unwrap());
    assert!(
        err.contains("match guards are not yet supported"),
        "Unexpected error: {}",
        err
    );
}

#[test]
fn test_unsupported_nested_pattern() {
    let source = r#"
        fn main() -> Int64 {
            let x: Option(Option(Int64)) = Some(Some(1))
            let fallback: Int64 = 0
            return match x {
                Some(Some(v)) => v,
                _ => fallback
            }
        }
    "#;
    let vm_res = run_source_vm(source, &[]);
    assert!(vm_res.is_err(), "VM should have failed for nested patterns");
    let err = format!("{:?}", vm_res.err().unwrap());
    assert!(
        err.contains("not yet supported in experimental VM"),
        "Unexpected error: {}",
        err
    );
}

fn canonical_float_bits(value: f64) -> u64 {
    if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

/// Float64 parity is asserted on canonicalized IEEE 754 bits so `NaN` is a
/// stable value and signed zero is observable (unlike `f64 ==`).
fn assert_differential_float(source: &str, expected: f64) {
    let hir_result = run_source_hir(source, &[])
        .unwrap_or_else(|error| panic!("HIR execution failed for source:\n{source}\n{error:?}"));
    let vm_result = run_source_vm(source, &[])
        .unwrap_or_else(|error| panic!("VM execution failed for source:\n{source}\n{error:?}"));

    let expected = canonical_float_bits(expected);
    match (normalize_hir(hir_result), normalize_vm(vm_result)) {
        (NormalizedValue::Float(hir), NormalizedValue::Float(vm)) => {
            assert_eq!(
                canonical_float_bits(hir),
                expected,
                "HIR Float64 result mismatch for source:\n{source}"
            );
            assert_eq!(
                canonical_float_bits(vm),
                expected,
                "VM Float64 result mismatch for source:\n{source}"
            );
        }
        (hir, vm) => {
            panic!("expected Float64 results for source:\n{source}\nHIR: {hir:?}\nVM: {vm:?}")
        }
    }
}

fn assert_differential_bool(source: &str, expected: bool) {
    let hir_result = run_source_hir(source, &[])
        .unwrap_or_else(|error| panic!("HIR execution failed for source:\n{source}\n{error:?}"));
    let vm_result = run_source_vm(source, &[])
        .unwrap_or_else(|error| panic!("VM execution failed for source:\n{source}\n{error:?}"));

    assert_eq!(
        normalize_hir(hir_result),
        NormalizedValue::Bool(expected),
        "HIR Bool result mismatch for source:\n{source}"
    );
    assert_eq!(
        normalize_vm(vm_result),
        NormalizedValue::Bool(expected),
        "VM Bool result mismatch for source:\n{source}"
    );
}

#[test]
fn test_diff_float64_ieee_arithmetic() {
    let cases = [
        ("fn main() -> Float64 { return 1.5 + 2.5 }", 4.0_f64),
        ("fn main() -> Float64 { return 5.0 - 8.0 }", -3.0_f64),
        ("fn main() -> Float64 { return 1.5 * 2.0 }", 3.0_f64),
        ("fn main() -> Float64 { return 7.0 / 2.0 }", 3.5_f64),
        ("fn main() -> Float64 { return 1.0 / 0.0 }", f64::INFINITY),
        (
            "fn main() -> Float64 { return (0.0 - 1.0) / 0.0 }",
            f64::NEG_INFINITY,
        ),
        ("fn main() -> Float64 { return 0.0 / 0.0 }", f64::NAN),
        (
            "fn main() -> Float64 { return (0.0 - 1.0) * 0.0 }",
            -0.0_f64,
        ),
        ("fn main() -> Float64 { return -1.5 }", -1.5_f64),
        ("fn main() -> Float64 { return -0.0 }", -0.0_f64),
        (
            "fn main() -> Float64 { return -(1.0 / 0.0) }",
            f64::NEG_INFINITY,
        ),
    ];

    for (source, expected) in cases {
        assert_differential_float(source, expected);
    }
}

#[test]
fn test_diff_float64_nan_ordering_and_signed_zero() {
    let cases = [
        ("fn main() -> Bool { return 0.0 / 0.0 < 1.0 }", false),
        ("fn main() -> Bool { return 0.0 / 0.0 > 1.0 }", false),
        ("fn main() -> Bool { return 0.0 / 0.0 <= 1.0 }", false),
        ("fn main() -> Bool { return 0.0 / 0.0 >= 1.0 }", false),
        ("fn main() -> Bool { return 0.0 / 0.0 == 0.0 / 0.0 }", false),
        ("fn main() -> Bool { return 0.0 / 0.0 != 0.0 / 0.0 }", true),
        ("fn main() -> Bool { return 1.0 / 0.0 > 1.0 }", true),
        ("fn main() -> Bool { return (0.0 - 1.0) / 0.0 < 0.0 }", true),
        ("fn main() -> Bool { return 0.0 == (0.0 - 0.0) }", true),
    ];

    for (source, expected) in cases {
        assert_differential_bool(source, expected);
    }
}
