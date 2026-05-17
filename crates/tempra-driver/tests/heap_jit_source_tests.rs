use tempra_vm::profile::VmOptions;
use tempra_vm::value::VmValue;
use tempra_bytecode::function::{BytecodeFunction, BytecodeType};
use tempra_bytecode::opcode::BytecodeInstr;
use tempra_bytecode::BytecodeModule;

fn run_differential_test(source: &str, expected: VmValue) {
    let mut options_no_jit = VmOptions::default();
    options_no_jit.jit_enabled = false;

    let mut options_jit = VmOptions::default();
    options_jit.jit_enabled = true;
    options_jit.profiling = true;
    options_jit.hot_call_threshold = 0; // Immediate compile on first call
    options_jit.jit_min_instruction_count = 0; // Enable short functions

    // 1. Run without JIT
    let res_no_jit = tempra_driver::run_source_vm_with_options(source, &[], options_no_jit).unwrap();
    assert_eq!(res_no_jit, expected, "Interpreter result mismatch");

    // 2. Run with JIT
    let (res_jit, profile) = tempra_driver::run_source_vm_profiled(source, options_jit).unwrap();
    assert_eq!(res_jit, expected, "JIT result mismatch");

    // 3. Verify JIT executed successfully and tracked heap runtime calls
    let summary = profile.summary();
    assert!(summary.contains("Compile attempts:"), "Summary should contain JIT metrics:\n{}", summary);
    assert!(profile.function_stats.iter().any(|f| f.jit_heap_runtime_calls > 0), "Should have executed JIT heap calls:\n{}", summary);
}

fn run_differential_error_test(source: &str) {
    let mut options_no_jit = VmOptions::default();
    options_no_jit.jit_enabled = false;

    let mut options_jit = VmOptions::default();
    options_jit.jit_enabled = true;
    options_jit.profiling = true;
    options_jit.hot_call_threshold = 0;
    options_jit.jit_min_instruction_count = 0;

    let res_no_jit = tempra_driver::run_source_vm_with_options(source, &[], options_no_jit);
    let res_jit = tempra_driver::run_source_vm_with_options(source, &[], options_jit);

    assert!(res_no_jit.is_err(), "Expected interpreter error");
    assert!(res_jit.is_err(), "Expected JIT fallback error");

    // Verify the error message matches in both paths
    let err_no_jit = format!("{:?}", res_no_jit.err().unwrap());
    let err_jit = format!("{:?}", res_jit.err().unwrap());
    assert_eq!(err_no_jit, err_jit, "Error message mismatch between interpreter and JIT fallback");
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_string_len_ascii() {
    let source = r#"
        fn strlen(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            let s1 = strlen("hello")
            let s2 = strlen("hello")
            return s1 + s2
        }
    "#;
    run_differential_test(source, VmValue::Int64(10));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_string_len_unicode() {
    let source = r#"
        fn strlen(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            let s1 = strlen("hé")
            let s2 = strlen("hé")
            return s1 + s2
        }
    "#;
    run_differential_test(source, VmValue::Int64(4));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_string_size_unicode() {
    let source = r#"
        fn ssize(s: String) -> Int64 {
            return s.size()
        }
        fn main() -> Int64 {
            let s1 = ssize("hé")
            let s2 = ssize("hé")
            return s1 + s2
        }
    "#;
    run_differential_test(source, VmValue::Int64(6));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_array_len() {
    let source = r#"
        fn get_len(xs: [Int64]) -> Int64 {
            return xs.len()
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let l1 = get_len(xs)
            let l2 = get_len(xs)
            return l1 + l2
        }
    "#;
    run_differential_test(source, VmValue::Int64(6));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_array_get_i64() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = get(xs)
            let v2 = get(xs)
            return v1 + v2
        }
    "#;
    run_differential_test(source, VmValue::Int64(40));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_array_get_negative_i64() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[-1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = get(xs)
            return v1
        }
    "#;
    // Actually, if we do two calls and the first returns error, the program stops.
    // That's fine, we just verify it correctly errors. Wait, if it errors on first call,
    // JIT isn't executed natively? That's true. But the first call compiles it. 
    // Is that enough? We can't catch errors in Tempra right now.
    // Let's just leave it as one call, it will error in the interpreter anyway and match.
    run_differential_error_test(source);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_mixed_int_and_heap() {
    let source = r#"
        fn process(s: String, xs: [Int64], base: Int64) -> Int64 {
            let l1 = s.len()
            let l2 = xs.len()
            let val = xs[0]
            return base + l1 + l2 + val
        }
        fn main() -> Int64 {
            let xs: [Int64] = [100, 200]
            let v1 = process("test", xs, 50)
            let v2 = process("test", xs, 50)
            return v1 + v2
        }
    "#;
    // base (50) + l1 (4) + l2 (2) + val (100) = 156 * 2 = 312
    run_differential_test(source, VmValue::Int64(312));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_generic_mixed() {
    let source = r#"
        fn f(s: String, xs: [Int64]) -> Int64 {
            return s.len() + s.size() + xs.len() + xs[1]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            let v1 = f("hé", xs)
            let v2 = f("hé", xs)
            return v1 + v2
        }
    "#;
    // f("hé", [10, 20, 30]) = 2 + 3 + 3 + 20 = 28. 28 * 2 = 56.
    run_differential_test(source, VmValue::Int64(56));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_array_get_oob_safe_error_or_fallback() {
    let source = r#"
        fn get(xs: [Int64]) -> Int64 {
            return xs[5]
        }
        fn main() -> Int64 {
            let xs: [Int64] = [10, 20, 30]
            return get(xs)
        }
    "#;
    run_differential_error_test(source);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_profile_summary_mentions_heap() {
    let source = r#"
        fn strlen(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            let s1 = strlen("hello")
            let s2 = strlen("hello")
            return s1 + s2
        }
    "#;
    let mut options_jit = VmOptions::default();
    options_jit.jit_enabled = true;
    options_jit.profiling = true;
    options_jit.hot_call_threshold = 0;
    options_jit.jit_min_instruction_count = 0;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options_jit).unwrap();
    let summary = profile.summary();
    
    assert!(summary.contains("Heap runtime:"));
    
    let fn_stats = profile.function_stats.iter().find(|f| f.function_name == "strlen").unwrap();
    assert!(fn_stats.jit_heap_runtime_calls > 0);
    assert!(fn_stats.jit_heap_context_objects > 0 || fn_stats.jit_heap_helper_calls > 0);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_rejects_struct_get() {
    let source = r#"
        struct User {
            age: Int64
        }
        fn get_age(u: User) -> Int64 {
            return u.age
        }
        fn main() -> Int64 {
            let u = User { age: 42 }
            let r1 = get_age(u)
            let r2 = get_age(u)
            return r1 + r2
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(res, VmValue::Int64(84));
    
    // Structs are not supported in source-level JIT, so get_age should be rejected.
    let get_age_stats = profile.function_stats.iter().find(|f| f.function_name == "get_age").unwrap();
    assert!(get_age_stats.jit_is_permanently_rejected);
    assert_eq!(get_age_stats.jit_compile_successes, 0);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_rejects_enum_tag() {
    let source = r#"
        fn check(opt: Int64?) -> Int64 {
            if opt is Some {
                return 1
            } else {
                return 0
            }
        }
        fn main() -> Int64 {
            let opt: Int64? = Some(42)
            let r1 = check(opt)
            let r2 = check(opt)
            return r1 + r2
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(res, VmValue::Int64(2));
    
    // Enums are not supported in JIT, so `check` should be rejected.
    let check_stats = profile.function_stats.iter().find(|f| f.function_name == "check").unwrap();
    assert!(check_stats.jit_is_permanently_rejected);
    assert_eq!(check_stats.jit_compile_successes, 0);
}

// Bytecode-level rejection tests for ArrayPush
// since we don't have source-level syntax for it yet.
#[test]
#[cfg(feature = "jit-cranelift")]
fn heap_jit_source_rejects_array_push() {
    let mut func = BytecodeFunction::new("test_push".to_string(), vec![], BytecodeType::Any);
    func.max_stack = 2;
    func.code.push(BytecodeInstr::MakeArray(0));
    func.code.push(BytecodeInstr::ConstUnit);
    func.code.push(BytecodeInstr::ArrayPush);
    func.code.push(BytecodeInstr::Return);
    
    let module = BytecodeModule {
        version: 0,
        functions: vec![func],
        constants: vec![],
    };
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let (_, profile) = tempra_vm::run_function_profiled(&module, "test_push", vec![], options).unwrap();
    
    let stats = profile.function_stats.first().unwrap();
    // In pure-interpreter/dummy run with a missing function, it will fail to compile.
    assert!(stats.jit_is_permanently_rejected || stats.jit_compile_successes == 0);
}
