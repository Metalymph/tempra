use tempra_driver::{compile_to_bytecode, emit_tir_source};
use tempra_vm::{VmOptions, VmValue};

fn assert_verified_execution(source: &str, expected: VmValue) {
    // emit-tir verifies independently of the VM compilation path and cache.
    emit_tir_source(source).expect("supported VM source must produce valid TIR");
    let dir = tempfile::tempdir().unwrap();
    let bytecode = compile_to_bytecode(source, &dir.path().join("fresh.tbc")).unwrap();
    let actual = tempra_vm::run_function_with_resolver(
        &bytecode,
        "main",
        vec![],
        VmOptions::default(),
        Some(tempra_native::registry::resolve),
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn result_question_success_produces_verified_tir() {
    assert_verified_execution(
        r#"
fn may_fail(ok: Bool) -> Int!String {
    if ok { return Ok(42) } else { return Err("bad") }
}
fn main() -> Int!String {
    let x = may_fail(true)?
    if x == 42 { return Ok(1) } else { return Ok(0) }
}
"#,
        VmValue::Enum("Result".into(), "Ok".into(), vec![VmValue::Int64(1)]),
    );
}

#[test]
fn result_question_error_produces_verified_tir() {
    assert_verified_execution(
        r#"
fn may_fail(ok: Bool) -> Int!String {
    if ok { return Ok(42) } else { return Err("bad") }
}
fn main() -> Int!String {
    let x = may_fail(false)?
    return Ok(x)
}
"#,
        VmValue::Enum(
            "Result".into(),
            "Err".into(),
            vec![VmValue::String("bad".into())],
        ),
    );
}

#[test]
fn result_initializers_and_patterns_produce_verified_tir() {
    assert_verified_execution(
        r#"
fn main() -> Int64 {
    let good: Int!String = Ok(10)
    var bad: Int!String = Err("bad")
    if good is Ok(v) {
        return match bad { Ok(_) => 0 Err(_) => v }
    }
    return 0
}
"#,
        VmValue::Int64(10),
    );
}

#[test]
fn option_patterns_produce_verified_tir() {
    assert_verified_execution(
        r#"
fn main() -> Int64 {
    let present: Int64? = Some(41)
    let absent: Int64? = None
    if present is Some(v) {
        if absent is None { return v + 1 }
    }
    return 0
}
"#,
        VmValue::Int64(42),
    );
}

#[test]
fn nested_result_constructor_uses_expected_payload_type() {
    assert_verified_execution(
        r#"
fn main() -> Result(Result(Int64, String), String) {
    return Ok(Err("inner"))
}
"#,
        VmValue::Enum(
            "Result".into(),
            "Ok".into(),
            vec![VmValue::Enum(
                "Result".into(),
                "Err".into(),
                vec![VmValue::String("inner".into())],
            )],
        ),
    );
}

#[test]
fn question_reconstructs_the_enclosing_result_type() {
    assert_verified_execution(
        r#"
fn input() -> Int!String { return Err("bad") }
fn main() -> Bool!String {
    let value = input()?
    return Ok(value == 42)
}
"#,
        VmValue::Enum(
            "Result".into(),
            "Err".into(),
            vec![VmValue::String("bad".into())],
        ),
    );
}

#[test]
fn option_match_produces_verified_tir() {
    assert_verified_execution(
        r#"
fn main() -> Int64 {
    let present: Int? = Some(42)
    return match present { Some(value) => value None => 0 }
}
"#,
        VmValue::Int64(42),
    );
}

#[test]
fn nested_terminating_branches_execute_through_tir_interpreter() {
    let source = r#"
fn classify(n: Int64) -> Int64 {
    if n < 0 {
        return -1
    } else {
        if n == 0 {
            return 0
        } else {
            if n > 100 { return 2 } else { return 1 }
        }
    }
}

fn main() -> Int64 {
    return classify(150)
}
"#;

    emit_tir_source(source).expect("nested terminating branches must produce valid TIR");
    let actual = tempra_driver::pipeline::interpret_tir_source(source, "main", vec![]).unwrap();
    assert_eq!(actual, tempra_tir_interp::TirRuntimeValue::Int64(2));
}
