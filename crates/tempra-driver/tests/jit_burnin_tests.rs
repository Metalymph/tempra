use std::fs;
use std::path::PathBuf;
use tempra_driver::{run_source_hir, run_source_vm_profiled};
use tempra_interpreter::value::Value;
use tempra_vm::profile::VmOptions;
use tempra_vm::value::VmValue;

/// Compare the HIR reference value with the VM/JIT result for the scalar subset
/// exercised by the burn-in corpus.
fn assert_hir_vm_match(hir: &Value, vm: &VmValue, context: &str) {
    match (hir, vm) {
        (Value::Unit, VmValue::Unit) => {}
        (Value::Bool(a), VmValue::Bool(b)) => assert_eq!(a, b, "Bool mismatch in {context}"),
        (Value::Int64(a), VmValue::Int64(b)) => assert_eq!(a, b, "Int64 mismatch in {context}"),
        (Value::UInt8(a), VmValue::UInt8(b)) => assert_eq!(a, b, "UInt8 mismatch in {context}"),
        (Value::Float64(a), VmValue::Float64(b)) => {
            assert_eq!(a, b, "Float64 mismatch in {context}")
        }
        (Value::String(a), VmValue::String(b)) => assert_eq!(a, b, "String mismatch in {context}"),
        (hir, vm) => panic!("unsupported HIR/VM comparison in {context}: HIR={hir:?} VM={vm:?}"),
    }
}

fn run_burnin_test(filename: &str, expected: i64, should_compile: bool) {
    run_burnin_test_with_options(filename, expected, should_compile, VmOptions::default());
}

fn run_burnin_test_with_options(
    filename: &str,
    expected: i64,
    should_compile: bool,
    mut options: VmOptions,
) {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../examples/jit_burnin");
    path.push(filename);

    let source = fs::read_to_string(&path).expect(&format!("failed to read {:?}", path));

    // 1. Get reference result from HIR interpreter
    let hir_res = run_source_hir(&source, &[]).expect("HIR run failed");

    // 2. Run with JIT enabled
    options.jit_enabled = true;
    options.profiling = true;
    // If using defaults (100/1000), lower them for burn-in
    if options.hot_call_threshold == 100 {
        options.hot_call_threshold = 5;
    }
    if options.hot_backedge_threshold == 1000 {
        options.hot_backedge_threshold = 5;
    }
    options.jit_min_instruction_count = 0; // Don't skip small ones in burn-in

    let (res, profile) = run_source_vm_profiled(&source, options).expect("VM JIT run failed");

    // 3. The VM/JIT result must agree with both the HIR reference and the
    //    expected value.
    assert_hir_vm_match(&hir_res, &res, filename);
    assert_eq!(
        res,
        VmValue::Int64(expected),
        "Result mismatch in {}",
        filename
    );

    if should_compile {
        let successes: u64 = profile
            .function_stats
            .iter()
            .map(|f| f.jit_compile_successes)
            .sum();
        assert!(
            successes > 0,
            "Expected at least one function to compile in {}",
            filename
        );

        // A successful compilation is not proof that native code ran; require
        // the runtime-call counter to show native execution too.
        let native_calls: u64 = profile
            .function_stats
            .iter()
            .map(|f| f.jit_runtime_calls)
            .sum();
        assert!(
            native_calls > 0,
            "Expected native JIT execution in {}",
            filename
        );
    } else {
        let rejections: u64 = profile
            .function_stats
            .iter()
            .map(|f| f.jit_permanent_rejections)
            .sum();
        assert!(
            rejections > 0,
            "Expected at least one rejection in {}",
            filename
        );
    }
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_call_chain() {
    run_burnin_test("int64_call_chain.tmpr", 115, true);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_nested_loops() {
    run_burnin_test("int64_nested_loops.tmpr", 125, true);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_branch_heavy() {
    let mut options = VmOptions::default();
    options.hot_call_threshold = 2;
    run_burnin_test_with_options("int64_branch_heavy.tmpr", 3, true, options);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_mixed_fallback() {
    run_burnin_test("int64_mixed_fallback.tmpr", 205, false);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_deep_nested_branches() {
    let source = r#"
fn deeply_nested(x: Int64, y: Int64) -> Int64 {
    if x > 0 {
        if y > 0 {
            if x > y { return 1 } else { return 2 }
        } else {
            if x + y > 0 { return 3 } else { return 4 }
        }
    } else {
        if y > 0 {
            if x + y > 0 { return 5 } else { return 6 }
        } else {
            if x < y { return 7 } else { return 8 }
        }
    }
}

fn main() -> Int64 {
    var total = 0
    total = total + deeply_nested(10, 5)   // 1
    total = total + deeply_nested(5, 10)   // 2
    total = total + deeply_nested(10, -5)  // 3
    total = total + deeply_nested(5, -10) // 4
    total = total + deeply_nested(-5, 10) // 5
    total = total + deeply_nested(-10, 5) // 6
    total = total + deeply_nested(-10, -5) // 7
    total = total + deeply_nested(-5, -10) // 8
    return total
}
"#;
    let hir_res = run_source_hir(source, &[]).expect("HIR run failed");
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;
    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_hir_vm_match(&hir_res, &res, "jit_burnin_deep_nested_branches");
    assert_eq!(res, VmValue::Int64(36));
    let successes: u64 = profile
        .function_stats
        .iter()
        .map(|f| f.jit_compile_successes)
        .sum();
    assert!(successes > 0);
    let native_calls: u64 = profile
        .function_stats
        .iter()
        .map(|f| f.jit_runtime_calls)
        .sum();
    assert!(
        native_calls > 0,
        "deep nested branches must execute natively"
    );
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_bool_logic_chain() {
    let source = r#"
fn logic_chain(a: Int64, b: Int64, c: Int64) -> Int64 {
    var res = 0
    if a > 0 && b > 0 && c > 0 {
        res = 1
    } else {
        if a < 0 || b < 0 || c < 0 {
            res = 2
        } else {
            res = 3
        }
    }
    return res
}

fn main() -> Int64 {
    var total = 0
    total = total + logic_chain(1, 1, 1)   // 1
    total = total + logic_chain(-1, 0, 0)  // 2
    total = total + logic_chain(0, 0, 0)   // 3
    return total
}
"#;
    let hir_res = run_source_hir(source, &[]).expect("HIR run failed");
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 1;
    let (res, _) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_hir_vm_match(&hir_res, &res, "jit_burnin_bool_logic_chain");
    assert_eq!(res, VmValue::Int64(1 + 2 + 3)); // 6
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_rejection_struct() {
    run_burnin_test("int64_rejection_struct.tmpr", 300, false);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_burnin_rejection_string() {
    run_burnin_test("int64_rejection_string.tmpr", 50, false);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_warm_run_reuses_compiled_functions() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            var x = 0
            for i in [1,2,3,4,5,6,7,8,9,10] {
                x = add(x, i)
            }
            return x
        }
    "#;

    let hir_res = run_source_hir(source, &[]).expect("HIR run failed");
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let (res, profile) = run_source_vm_profiled(source, options).expect("VM run failed");
    assert_hir_vm_match(&hir_res, &res, "jit_warm_run_reuses_compiled_functions");
    assert_eq!(res, VmValue::Int64(55)); // sum(1..=10)
    let add_stats = profile
        .function_stats
        .iter()
        .find(|f| f.function_name == "add")
        .unwrap();

    assert_eq!(
        add_stats.jit_compile_successes, 1,
        "Should compile exactly once"
    );
    assert!(
        add_stats.jit_runtime_calls > 5,
        "Should use JIT version multiple times"
    );
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_repeated_execution_stress() {
    let source = r#"
        fn main() -> Int64 {
            var x = 0
            for i in [1,2,3,4,5,6,7,8,9,10] {
                x = x + 1
            }
            return x
        }
    "#;
    let hir_res = run_source_hir(source, &[]).expect("HIR run failed");
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 2;

    for _ in 0..20 {
        let (res, _) = run_source_vm_profiled(source, options).unwrap();
        assert_hir_vm_match(&hir_res, &res, "jit_repeated_execution_stress");
        assert_eq!(res, VmValue::Int64(10));
    }
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_differential_micro_tests() {
    let cases = [
        ("fn main() -> Int64 { return 1 + 2 * 3 }", 7),
        ("fn main() -> Int64 {\n if true { return 10 } else { return 20 }\n }", 10),
        ("fn main() -> Int64 {\n var x = 0\n if 5 > 3 { x = 1 } else { x = 2 }\n return x\n }", 1),
        ("fn main() -> Int64 {\n var x = 1\n for i in [1,2,3] { x = x * 2 }\n return x\n }", 8),
        ("fn add(a: Int64, b: Int64) -> Int64 { return a + b }\n fn main() -> Int64 { return add(10, 20) }", 30),
    ];

    for (source, expected) in cases {
        let hir_res = run_source_hir(source, &[]).expect("HIR run failed");
        let mut options = VmOptions::default();
        options.jit_enabled = true;
        options.hot_call_threshold = 0; // Eager
        options.jit_min_instruction_count = 0;

        let (res, _) = run_source_vm_profiled(source, options).expect("JIT run failed");
        assert_hir_vm_match(&hir_res, &res, source);
        assert_eq!(
            res,
            VmValue::Int64(expected),
            "Mismatch in source: {}",
            source
        );
    }
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn jit_profile_summary_validation() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            var x = 0
            for i in [1,2,3,4,5,6,7,8,9,10] {
                x = add(x, i)
            }
            return x
        }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 2;
    options.jit_min_instruction_count = 0;

    let (_, profile) = run_source_vm_profiled(source, options).unwrap();
    let summary = profile.summary();

    assert!(
        summary.contains("JIT Summary:"),
        "Summary missing JIT section"
    );
    // In Phase 15.5, the summary section labels are:
    // "  Successes:        N"
    // "  Runtime calls:    N"
    // etc.
    assert!(
        summary.contains("Successes:"),
        "Summary missing successes. Summary was: {}",
        summary
    );
    assert!(
        summary.contains("Runtime calls:"),
        "Summary missing runtime calls. Summary was: {}",
        summary
    );
}
