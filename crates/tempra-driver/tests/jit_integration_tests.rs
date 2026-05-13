use tempra_driver::run_source_vm_with_options;
use tempra_vm::value::VmValue;
use tempra_vm::profile::VmOptions;

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_driver_jit_loop() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
            var sum = 0
            for i in xs {
                for j in xs {
                    sum = add(sum, j)
                }
            }
            return sum
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.hot_call_threshold = 10;
    options.hot_backedge_threshold = 10;
    
    let res = run_source_vm_with_options(source, &[], options).unwrap();
    // sum = 10 * (1+2+3+4+5+6+7+8+9+10) = 10 * 55 = 550
    assert_eq!(res, VmValue::Int64(550));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_driver_jit_observability() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            let xs = [1, 2, 3, 4, 5]
            var sum = 0
            for i in xs {
                for j in xs {
                    sum = add(sum, j)
                }
            }
            return sum
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 5;
    
    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(res, VmValue::Int64(75)); // 5 * (1+2+3+4+5) = 5 * 15 = 75
    
    let summary = profile.summary();
    assert!(summary.contains("JIT Summary:"));
    assert!(summary.contains("Compile attempts:"));
    assert!(summary.contains("Successes:"));
    assert!(summary.contains("Runtime calls:"));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_driver_jit_native_calls() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 {
            return a + b
        }
        fn main() -> Int64 {
            let s1 = add(10, 20)
            let s2 = add(s1, 30)
            let s3 = add(s2, 40)
            return s3
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0; // Compile immediately on first call
    options.hot_backedge_threshold = 100;
    
    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(res, VmValue::Int64(100)); // 10+20+30+40 = 100

    
    let summary = profile.summary();
    // main is hot because of backedges. It calls add.
    // add gets pre-compiled as a dependency of main.
    // So both should be compiled successfully!
    assert!(summary.contains("Successes:        2"));
    assert!(!summary.contains("JIT Rejections:"));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_driver_jit_fallback() {
    let source = r#"
        fn process_string(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            var sum = 0
            let arr = [1, 2, 3]
            for i in arr {
                sum = sum + process_string("test")
            }
            return sum
        }
    "#;
    
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 1;
    
    let (res, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(res, VmValue::Int64(12)); // 3 * 4 = 12
    
    let summary = profile.summary();
    assert!(summary.contains("JIT Summary:"));
    assert!(summary.contains("Compile attempts:"));
    assert!(summary.contains("Successes:        0"));
    assert!(summary.contains("JIT Rejections:"));
    assert!(summary.contains("Unsupported type"));
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_profile_records_compile_time() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 { return add(10, 20) }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    
    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let main_stats = profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();
    let add_stats = profile.function_stats.iter().find(|f| f.function_name == "add").unwrap();
    
    assert!(main_stats.jit_compile_time_ns > 0);
    // Add is compiled as a dependency, its compile time is aggregated into main's compile time for v0 policy
    assert_eq!(add_stats.jit_compile_time_ns, 0);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_profile_runtime_calls_increase() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            let s1 = add(1, 2)
            let s2 = add(s1, 3)
            return s2
        }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0; // main is hot immediately
    
    let hash = tempra_driver::cache::compute_source_hash(source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let module = tempra_driver::compile_to_bytecode(source, &cache_path).unwrap();
    
    let mut interpreter = tempra_vm::Interpreter::with_options(&module, options);
    let main_id = tempra_bytecode::function::FunctionId(module.functions.iter().position(|f| f.name == "main").unwrap() as u32);
    
    // Cold run (compiles)
    interpreter.run_function_id(main_id, vec![]).unwrap();
    // Warm run (executes natively)
    interpreter.run_function_id(main_id, vec![]).unwrap();
    // Warm run (executes natively)
    interpreter.run_function_id(main_id, vec![]).unwrap();
    
    let profile = interpreter.build_profile();
    let main_stats = profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();
    
    // main is called natively 2 times 
    assert_eq!(main_stats.jit_runtime_calls, 2);
    // On Mac/Linux, measuring Instant::now() might be 0ns if super fast.
    // So we just check that it compiles correctly and metrics aren't broken.
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_warm_run_uses_existing_compiled_function() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 { return add(1, 2) }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 0;
    
    let hash = tempra_driver::cache::compute_source_hash(source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let module = tempra_driver::compile_to_bytecode(source, &cache_path).unwrap();
    
    let mut interpreter = tempra_vm::Interpreter::with_options(&module, options);
    let main_id = tempra_bytecode::function::FunctionId(module.functions.iter().position(|f| f.name == "main").unwrap() as u32);
    
    // Cold run
    let _ = interpreter.run_function_id(main_id, vec![]).unwrap();
    let cold_profile = interpreter.build_profile();
    let cold_main = cold_profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();
    assert_eq!(cold_main.jit_compile_attempts, 1);
    
    // Warm run
    let _ = interpreter.run_function_id(main_id, vec![]).unwrap();
    let warm_profile = interpreter.build_profile();
    let warm_main = warm_profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();
    
    // Attempt count should remain exactly 1, no new compilation attempt
    assert_eq!(warm_main.jit_compile_attempts, 1);
    // It should have executed as a runtime call during the warm run
    assert_eq!(warm_main.jit_runtime_calls, 1);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_threshold_prevents_early_compile() {
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 { return add(1, 2) }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 5; // higher than the 1 call to main
    
    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let main_stats = profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();
    
    assert_eq!(main_stats.jit_compile_attempts, 0);
    assert_eq!(main_stats.jit_compile_successes, 0);
    assert_eq!(main_stats.jit_compile_time_ns, 0);
}
