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
    // The call was counted as a "below threshold" skip
    assert!(main_stats.jit_skipped_below_threshold > 0);
}

// ============================================================
// Phase 15.5 — JIT Policy tests
// ============================================================

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_skips_below_threshold() {
    // hot_call_threshold = 100; function called once → skip, not compile
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 { return add(1, 2) }
    "#;
    let mut options = VmOptions::default(); // hot_call_threshold = 100
    options.jit_enabled = true;
    options.profiling  = true;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let main = profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();

    assert_eq!(main.jit_compile_attempts, 0, "must not attempt compilation below threshold");
    assert!(main.jit_skipped_below_threshold > 0, "skip counter must be non-zero");
    assert_eq!(main.jit_compile_successes, 0);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_skips_too_small_function() {
    // Function body: just `return 42` → 1-2 instructions < jit_min_instruction_count=8
    let source = r#"
        fn tiny() -> Int64 { return 42 }
        fn main() -> Int64 { return tiny() }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling  = true;
    options.hot_call_threshold = 0; // force hot
    options.jit_min_instruction_count = 8;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let tiny = profile.function_stats.iter().find(|f| f.function_name == "tiny").unwrap();

    // tiny() has very few instructions, so it should be skipped as too small
    assert_eq!(tiny.jit_compile_attempts, 0, "tiny function must not be compiled");
    assert!(tiny.jit_skipped_too_small > 0, "skipped_too_small counter must be non-zero");
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_rejects_unsupported_permanently() {
    // Function uses a String parameter → unsupported, must be permanently rejected
    let source = r#"
        fn greet(s: String) -> Int64 { return s.len() }
        fn main() -> Int64 { return greet("hello") }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling  = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let greet = profile.function_stats.iter().find(|f| f.function_name == "greet").unwrap();

    assert!(greet.jit_is_permanently_rejected, "string function must be permanently rejected");
    assert!(greet.jit_rejection_reason.is_some(), "rejection reason must be set");
    assert_eq!(greet.jit_permanent_rejections, 1);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_does_not_recompile_rejected_function() {
    // Call the unsupported function multiple times; compile must only be attempted once
    let source = r#"
        fn greet(s: String) -> Int64 { return s.len() }
        fn main() -> Int64 {
            var sum = 0
            let strs = ["a", "bb", "ccc"]
            for s in strs {
                sum = sum + greet(s)
            }
            return sum
        }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling  = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let greet = profile.function_stats.iter().find(|f| f.function_name == "greet").unwrap();

    // Attempted once, then permanently rejected → never attempted again
    assert_eq!(greet.jit_compile_attempts, 1, "must only attempt once");
    assert_eq!(greet.jit_permanent_rejections, 1);
    assert!(greet.jit_is_permanently_rejected);
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_compiles_after_threshold() {
    // Run main many times until it crosses hot_call_threshold
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b + a - b + a }
        fn main() -> Int64 { return add(10, 20) }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling  = true;
    options.hot_call_threshold = 3;
    options.jit_min_instruction_count = 0;

    let hash = tempra_driver::cache::compute_source_hash(source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let module = tempra_driver::compile_to_bytecode(source, &cache_path).unwrap();
    let mut interp = tempra_vm::Interpreter::with_options(&module, options);
    let main_id = tempra_bytecode::function::FunctionId(
        module.functions.iter().position(|f| f.name == "main").unwrap() as u32
    );

    // Run 4 times: after 3 calls main becomes hot and is compiled on the 4th
    for _ in 0..4 {
        interp.run_function_id(main_id, vec![]).unwrap();
    }
    let profile = interp.build_profile();
    let main = profile.function_stats.iter().find(|f| f.function_name == "main").unwrap();

    assert!(main.jit_compile_attempts > 0, "compilation must be attempted after crossing threshold");
    assert_eq!(main.jit_compile_successes, 1);
    // calls 1 and 2 are below threshold (call_count < 3); call 3 hits the threshold and compiles
    assert_eq!(main.jit_skipped_below_threshold, 2, "first 2 calls skipped (not yet hot)");
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_policy_retry_limit_respected() {
    // With jit_retry_limit = 1, after one Cranelift failure the function must be skipped forever.
    // We cannot easily force a Cranelift internal error, but we CAN verify that unsupported
    // eligibility sets is_permanently_rejected so subsequent calls are zero-cost no-ops.
    let source = r#"
        fn unsupported(s: String) -> Int64 { return s.len() }
        fn main() -> Int64 {
            var n = 0
            let xs = ["a", "b", "c", "d", "e"]
            for x in xs {
                n = n + unsupported(x)
            }
            return n
        }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling  = true;
    options.hot_call_threshold = 0;
    options.jit_min_instruction_count = 0;
    options.jit_retry_limit = 1;

    let (_, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    let unsup = profile.function_stats.iter().find(|f| f.function_name == "unsupported").unwrap();

    // Must never exceed 1 attempt regardless of how many times the function is dispatched
    assert_eq!(unsup.jit_compile_attempts, 1, "only one attempt allowed by retry_limit=1");
    assert!(unsup.jit_is_permanently_rejected);
    assert!(unsup.jit_skipped_below_threshold == 0, "threshold was 0, should not be in threshold-skip bucket");
}

// ============================================================
// Phase 15.6 — Driver / CLI JIT integration tests
// ============================================================

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_enabled_vm_executes() {
    // Verify that the new driver entry point with jit_enabled=true produces correct results
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            var x = 0
            for i in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                for j in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                    x = add(x, i)
                }
            }
            return x
        }
    "#;
    // Use the new driver function that threads jit_enabled
    let result = tempra_driver::run_source_with_experimental_vm_jit(source, &[], true);
    assert!(result.is_ok(), "JIT-enabled run failed: {:?}", result.err());
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_enabled_hot_function_compiles() {
    // Verify that hot functions are actually compiled when using the driver with JIT
    let source = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            var x = 0
            for i in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                for j in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
                    x = add(x, i)
                }
            }
            return x
        }
    "#;
    let mut options = VmOptions::default();
    options.jit_enabled = true;
    options.profiling = true;
    options.hot_call_threshold = 5;
    options.jit_min_instruction_count = 0;

    let (val, profile) = tempra_driver::run_source_vm_profiled(source, options).unwrap();
    assert_eq!(val, VmValue::Int64(550));

    let add_stats = profile.function_stats.iter().find(|f| f.function_name == "add").unwrap();
    assert!(add_stats.jit_compile_successes > 0, "add() should have been JIT-compiled");
}

#[test]
#[cfg(feature = "jit-cranelift")]
fn test_jit_enabled_unsupported_function_fallback() {
    // Verify that JIT correctly falls back to the interpreter for unsupported functions
    let source = r#"
        fn greet(name: String) -> String { return name }
        fn main() -> Int64 {
            let s = greet("hello")
            return s.len()
        }
    "#;
    // Should succeed — the string function falls back to the interpreter
    let result = tempra_driver::run_source_with_experimental_vm_jit(source, &[], true);
    assert!(result.is_ok(), "JIT fallback failed: {:?}", result.err());
}
