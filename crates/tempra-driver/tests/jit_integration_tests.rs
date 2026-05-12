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
