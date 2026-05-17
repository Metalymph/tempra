use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};
use tempra_bytecode::BytecodeModule;
use tempra_driver::{compile_to_bytecode, run_source_hir};

const ITERATIONS: u32 = 20;
const WARMUP: u32 = 2;

struct PerfMetrics {
    case_name: String,
    hir_avg: Duration,
    vm_miss_avg: Duration,
    vm_hit_avg: Duration,
    vm_exec_avg: Duration,
    bytecode_bytes: usize,
    instr_count: usize,
    fn_count: usize,
    cache_artifact_bytes: usize,
}

fn get_cache_dir() -> std::path::PathBuf {
    let p = Path::new("target").join("perf_cache");
    if !p.exists() {
        fs::create_dir_all(&p).unwrap();
    }
    p
}

fn clear_cache() {
    let p = get_cache_dir();
    if p.exists() {
        fs::remove_dir_all(&p).unwrap();
        fs::create_dir_all(&p).unwrap();
    }
}

fn get_bytecode_metrics(module: &BytecodeModule) -> (usize, usize) {
    let fn_count = module.functions.len();
    let mut instr_count = 0;
    for f in &module.functions {
        instr_count += f.code.len();
    }
    (fn_count, instr_count)
}

fn run_benchmark(name: &str, source: &str) -> PerfMetrics {
    // 1. Warmup HIR
    for _ in 0..WARMUP {
        let _ = run_source_hir(source, &[]).unwrap();
    }

    // 2. Measure HIR
    let start_hir = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = run_source_hir(source, &[]).unwrap();
    }
    let hir_avg = start_hir.elapsed() / ITERATIONS;

    let _cache_dir = get_cache_dir();

    // 3. Measure VM Cache Miss (Compile + Execute)
    let mut miss_total = Duration::ZERO;
    let mut bytecode_bytes = 0;
    let mut cache_artifact_bytes = 0;
    let mut fn_count = 0;
    let mut instr_count = 0;

    for _ in 0..ITERATIONS {
        clear_cache();
        let hash = tempra_driver::cache::compute_source_hash(source);
        let cache_path = tempra_driver::cache::get_cache_path(&hash);
        // We need to point the driver to our custom cache dir
        // But run_source_vm uses hardcoded .tempra/cache
        // So we use compile_to_bytecode and then manual run

        let start_miss = Instant::now();
        let module = compile_to_bytecode(source, &cache_path).unwrap();
        let _ = tempra_vm::run_function(&module, "main", vec![]).unwrap();
        miss_total += start_miss.elapsed();

        if bytecode_bytes == 0 {
            // Measure first time
            let artifact = tempra_bytecode::BytecodeCacheArtifact {
                header: tempra_bytecode::BytecodeCacheHeader {
                    compiler_fingerprint: "perf".into(),
                    source_hash: hash,
                    ..Default::default()
                },
                module: module.clone(),
            };
            let encoded = tempra_bytecode::serialize::encode_artifact(&artifact).unwrap();
            bytecode_bytes = encoded.len(); // Approximate, since it includes header
            cache_artifact_bytes = fs::metadata(&cache_path)
                .map(|m| m.len() as usize)
                .unwrap_or(0);
            let (fc, ic) = get_bytecode_metrics(&module);
            fn_count = fc;
            instr_count = ic;
        }
    }
    let vm_miss_avg = miss_total / ITERATIONS;

    // 4. Measure VM Cache Hit
    // Pre-populate cache
    clear_cache();
    let hash = tempra_driver::cache::compute_source_hash(source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let module = compile_to_bytecode(source, &cache_path).unwrap();

    for _ in 0..WARMUP {
        let _ = tempra_vm::run_function(&module, "main", vec![]).unwrap();
    }

    let start_hit = Instant::now();
    for _ in 0..ITERATIONS {
        let artifact = tempra_driver::cache::read_cache_artifact(&cache_path).unwrap();
        let _ = tempra_vm::run_function(&artifact.module, "main", vec![]).unwrap();
    }
    let vm_hit_avg = start_hit.elapsed() / ITERATIONS;

    // 5. Measure VM Exec Only (No Load/Verify)
    let start_exec = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = tempra_vm::run_function(&module, "main", vec![]).unwrap();
    }
    let vm_exec_avg = start_exec.elapsed() / ITERATIONS;

    // 6. Profiling Summary (Run once)
    let options = tempra_vm::VmOptions {
        profiling: true,
        hot_call_threshold: 1, // Capture everything for small benchmarks
        hot_backedge_threshold: 1,
        jit_enabled: false,
        jit_min_instruction_count: 8,
        jit_retry_limit: 1,
    };
    let (_, profile) = tempra_vm::run_function_profiled(&module, "main", vec![], options).unwrap();
    println!("\n--- Profiling Summary: {} ---", name);
    println!("{}", profile.summary());

    PerfMetrics {
        case_name: name.to_string(),
        hir_avg,
        vm_miss_avg,
        vm_hit_avg,
        vm_exec_avg,
        bytecode_bytes,
        instr_count,
        fn_count,
        cache_artifact_bytes,
    }
}

fn print_table(results: &[PerfMetrics]) {
    println!("\n| Case | HIR Avg | VM Miss Avg | VM Hit Avg | VM Exec | BC Size | Instrs | Fns | Cache File |");
    println!("|------|---------|-------------|------------|---------|---------|--------|-----|------------|");
    for r in results {
        println!(
            "| {:<20} | {:>9?} | {:>11?} | {:>10?} | {:>7?} | {:>7} B | {:>6} | {:>3} | {:>10} B |",
            r.case_name,
            r.hir_avg,
            r.vm_miss_avg,
            r.vm_hit_avg,
            r.vm_exec_avg,
            r.bytecode_bytes,
            r.instr_count,
            r.fn_count,
            r.cache_artifact_bytes
        );
    }
    println!();
}

#[derive(Debug)]
struct JitPerfRow {
    case: String,
    mode: String,
    threshold: u64,
    duration_ns: u64,
    jit_compile_time_ns: u64,
    jit_runtime_calls: u64,
    jit_fallback_calls: u64,
    jit_rejected: u64,
    bytecode_instr_count: usize,
}

fn print_jit_table(rows: &[JitPerfRow]) {
    println!("\n| Case | Mode | Thresh | Duration (ns) | Compile (ns) | RT Calls | FB Calls | Reject | Instrs |");
    println!("|------|------|--------|---------------|--------------|----------|----------|--------|--------|");
    for r in rows {
        println!(
            "| {:<20} | {:<10} | {:>6} | {:>13} | {:>12} | {:>8} | {:>8} | {:>6} | {:>6} |",
            r.case,
            r.mode,
            r.threshold,
            r.duration_ns,
            r.jit_compile_time_ns,
            r.jit_runtime_calls,
            r.jit_fallback_calls,
            r.jit_rejected,
            r.bytecode_instr_count
        );
    }
    println!();
}

#[cfg(feature = "jit-cranelift")]
fn run_jit_benchmark(name: &str, source: &str, threshold: u64) -> Vec<JitPerfRow> {
    let mut rows = Vec::new();
    let hash = tempra_driver::cache::compute_source_hash(source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let module = compile_to_bytecode(source, &cache_path).unwrap();
    let (_, instr_count) = get_bytecode_metrics(&module);

    // 1. HIR
    let start_hir = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = tempra_driver::run_source_hir(source, &[]).unwrap();
    }
    let hir_avg = start_hir.elapsed().as_nanos() as u64 / (ITERATIONS as u64);
    rows.push(JitPerfRow {
        case: name.to_string(),
        mode: "HIR".into(),
        threshold,
        duration_ns: hir_avg,
        jit_compile_time_ns: 0,
        jit_runtime_calls: 0,
        jit_fallback_calls: 0,
        jit_rejected: 0,
        bytecode_instr_count: instr_count,
    });

    // 2. VM No JIT
    let mut options = tempra_vm::VmOptions {
        profiling: true,
        jit_enabled: false,
        hot_call_threshold: threshold,
        hot_backedge_threshold: threshold,
        jit_min_instruction_count: 8,
        jit_retry_limit: 1,
    };
    let start_vm = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = tempra_vm::run_function_profiled(&module, "main", vec![], options).unwrap();
    }
    let vm_avg = start_vm.elapsed().as_nanos() as u64 / (ITERATIONS as u64);
    rows.push(JitPerfRow {
        case: name.to_string(),
        mode: "VM".into(),
        threshold,
        duration_ns: vm_avg,
        jit_compile_time_ns: 0,
        jit_runtime_calls: 0,
        jit_fallback_calls: 0,
        jit_rejected: 0,
        bytecode_instr_count: instr_count,
    });

    // 3. VM JIT Cold (we run it once per iteration to capture compilation overhead each time)
    options.jit_enabled = true;
    let start_cold = Instant::now();
    let mut cold_compile = 0;
    let mut cold_rt = 0;
    let mut cold_fb = 0;
    let mut cold_rej = 0;
    for _ in 0..ITERATIONS {
        let (_, profile) =
            tempra_vm::run_function_profiled(&module, "main", vec![], options).unwrap();
        cold_compile += profile
            .function_stats
            .iter()
            .map(|f| f.jit_compile_time_ns)
            .sum::<u64>();
        cold_rt += profile
            .function_stats
            .iter()
            .map(|f| f.jit_runtime_calls)
            .sum::<u64>();
        cold_fb += profile
            .function_stats
            .iter()
            .map(|f| f.jit_fallback_calls)
            .sum::<u64>();
        cold_rej += profile
            .function_stats
            .iter()
            .map(|f| f.jit_rejected_count)
            .sum::<u64>();
    }
    let cold_avg = start_cold.elapsed().as_nanos() as u64 / (ITERATIONS as u64);
    rows.push(JitPerfRow {
        case: name.to_string(),
        mode: "JIT-Cold".into(),
        threshold,
        duration_ns: cold_avg,
        jit_compile_time_ns: cold_compile / (ITERATIONS as u64),
        jit_runtime_calls: cold_rt / (ITERATIONS as u64),
        jit_fallback_calls: cold_fb / (ITERATIONS as u64),
        jit_rejected: cold_rej / (ITERATIONS as u64),
        bytecode_instr_count: instr_count,
    });

    // 4. VM JIT Warm (use the same interpreter instance across iterations so it stays compiled)
    let start_warm = Instant::now();
    let mut interpreter = tempra_vm::Interpreter::with_options(&module, options);
    let mut warm_compile = 0;
    let mut warm_rt = 0;
    let mut warm_fb = 0;
    let mut warm_rej = 0;

    // Warmup
    let main_id = tempra_bytecode::function::FunctionId(
        module
            .functions
            .iter()
            .position(|f| f.name == "main")
            .unwrap() as u32,
    );
    let _ = interpreter.run_function_id(main_id, vec![]).unwrap();

    for _ in 0..ITERATIONS {
        let _ = interpreter.run_function_id(main_id, vec![]).unwrap();
        let profile = interpreter.build_profile();
        warm_compile += profile
            .function_stats
            .iter()
            .map(|f| f.jit_compile_time_ns)
            .sum::<u64>();
        warm_rt += profile
            .function_stats
            .iter()
            .map(|f| f.jit_runtime_calls)
            .sum::<u64>();
        warm_fb += profile
            .function_stats
            .iter()
            .map(|f| f.jit_fallback_calls)
            .sum::<u64>();
        warm_rej += profile
            .function_stats
            .iter()
            .map(|f| f.jit_rejected_count)
            .sum::<u64>();
    }
    let warm_avg = start_warm.elapsed().as_nanos() as u64 / (ITERATIONS as u64);
    rows.push(JitPerfRow {
        case: name.to_string(),
        mode: "JIT-Warm".into(),
        threshold,
        duration_ns: warm_avg,
        jit_compile_time_ns: warm_compile / (ITERATIONS as u64),
        jit_runtime_calls: warm_rt / (ITERATIONS as u64),
        jit_fallback_calls: warm_fb / (ITERATIONS as u64),
        jit_rejected: warm_rej / (ITERATIONS as u64),
        bytecode_instr_count: instr_count,
    });

    rows
}

#[test]
#[ignore]
#[cfg(feature = "jit-cranelift")]
fn benchmark_jit_performance() {
    let mut all_rows = Vec::new();

    // Case 1: int64_call_chain
    let call_chain = r#"
        fn a(x: Int64) -> Int64 { return x + 1 }
        fn b(x: Int64) -> Int64 { return a(x) * 2 }
        fn c(x: Int64) -> Int64 { return b(x) - 3 }
        fn main() -> Int64 {
            var sum = 0
            let xs = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            for i in xs {
                for j in xs {
                    sum = sum + c(i)
                }
            }
            return sum
        }
    "#;
    all_rows.extend(run_jit_benchmark("int64_call_chain", call_chain, 10));

    // Case 2: int64_nested_loop_sum
    let nested_loop = r#"
        fn add(a: Int64, b: Int64) -> Int64 { return a + b }
        fn main() -> Int64 {
            var sum = 0
            let xs = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            for i in xs {
                for j in xs {
                    sum = add(sum, add(i, j))
                }
            }
            return sum
        }
    "#;
    all_rows.extend(run_jit_benchmark("int64_nested_loop_sum_1", nested_loop, 1));
    all_rows.extend(run_jit_benchmark(
        "int64_nested_loop_sum_10",
        nested_loop,
        10,
    ));
    all_rows.extend(run_jit_benchmark(
        "int64_nested_loop_sum_100",
        nested_loop,
        100,
    ));

    // Case 3: int64_arithmetic_hot_function
    let arithmetic_hot = r#"
        fn compute(x: Int64) -> Int64 {
            return (x * 3 + 14) / 2 - x
        }
        fn main() -> Int64 {
            var sum = 0
            let xs = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            for i in xs {
                for j in xs {
                    sum = sum + compute(i)
                }
            }
            return sum
        }
    "#;
    all_rows.extend(run_jit_benchmark("int64_arithm_hot", arithmetic_hot, 10));

    // Case 4: mixed_supported_and_unsupported_fallback
    let mixed_fallback = r#"
        fn compute(x: Int64) -> Int64 {
            return x * 2
        }
        fn get_string_len(s: String) -> Int64 {
            return s.len()
        }
        fn main() -> Int64 {
            var sum = 0
            let xs = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            for i in xs {
                for j in xs {
                    if i < 5 {
                        sum = sum + compute(i)
                    } else {
                        sum = sum + get_string_len("test")
                    }
                }
            }
            return sum
        }
    "#;
    all_rows.extend(run_jit_benchmark("mixed_fallback", mixed_fallback, 10));

    print_jit_table(&all_rows);

    // Calculate time-to-profitability
    println!("### Time-To-Profitability Estimates\n");
    println!("| Case | Compile Cost (ns) | VM Exec (ns) | Warm JIT Exec (ns) | Speedup/Call (ns) | Est. Calls to Profit |");
    println!("|------|-------------------|--------------|--------------------|-------------------|----------------------|");

    // Group by case name
    let mut cases = std::collections::HashSet::new();
    for row in &all_rows {
        cases.insert(row.case.clone());
    }

    let mut sorted_cases: Vec<_> = cases.into_iter().collect();
    sorted_cases.sort();

    for case in sorted_cases {
        let vm_row = all_rows
            .iter()
            .find(|r| r.case == case && r.mode == "VM")
            .unwrap();
        let cold_row = all_rows
            .iter()
            .find(|r| r.case == case && r.mode == "JIT-Cold")
            .unwrap();
        let warm_row = all_rows
            .iter()
            .find(|r| r.case == case && r.mode == "JIT-Warm")
            .unwrap();

        let compile_cost = cold_row.jit_compile_time_ns;
        // Approximation: total duration divided by iteration loop count (1000 for most cases)
        // Since we don't have exactly "per call", we use the difference in total duration
        let speedup_total = if vm_row.duration_ns > warm_row.duration_ns {
            vm_row.duration_ns - warm_row.duration_ns
        } else {
            0
        };

        // Let's assume inner loop is roughly 1000 calls. Actually nested loop is 100x100 = 10000.
        // We will just do full run profitability.
        let calls_to_profit = if speedup_total > 0 {
            let speedup_per_run = speedup_total; // Speedup per one test function run
            let runs_to_profit = compile_cost as f64 / speedup_per_run as f64;
            format!("{:.2} full runs", runs_to_profit)
        } else {
            "Never".to_string()
        };

        println!(
            "| {:<20} | {:>17} | {:>12} | {:>18} | {:>17} | {:>20} |",
            case,
            compile_cost,
            vm_row.duration_ns,
            warm_row.duration_ns,
            speedup_total,
            calls_to_profit
        );
    }
    println!();
}

#[test]
#[ignore]
fn benchmark_vm_performance() {
    let mut results = Vec::new();

    // Case 1: Arithmetic Loop (Large)
    // We use a 1000 element array to simulate a loop of 1000 iterations
    let mut array_literal = String::from("[");
    for i in 0..1000 {
        array_literal.push_str(&i.to_string());
        if i < 999 {
            array_literal.push_str(", ");
        }
    }
    array_literal.push(']');

    let arithmetic_loop = format!(
        r#"
        fn main() -> Int64 {{
            let xs = {}
            var sum = 0
            for x in xs {{
                sum = sum + x
            }}
            return sum
        }}
    "#,
        array_literal
    );
    results.push(run_benchmark("arithmetic_loop_1000", &arithmetic_loop));

    // Case 2: For-Expr Collect
    let for_expr = format!(
        r#"
        fn main() -> Int64 {{
            let xs = {}
            let ys = for x in xs {{ x + 1 }}
            return ys[999]
        }}
    "#,
        array_literal
    );
    results.push(run_benchmark("for_expr_collect_1000", &for_expr));

    // Case 3: Struct Access Loop
    let struct_access = format!(
        r#"
        struct Counter {{
            var value: Int64
        }}
        fn main() -> Int64 {{
            var c = Counter {{ value: 0 }}
            let xs = {}
            for i in xs {{
                c.value = c.value + 1
            }}
            return c.value
        }}
    "#,
        array_literal
    );
    results.push(run_benchmark("struct_access_1000", &struct_access));

    // Case 4: String Basics
    let string_basics = r#"
        fn main() -> Int64 {
            let s = "héllo 👋"
            var sum = 0
            // Repeat some operations to have measurable time
            let xs = [1,2,3,4,5,6,7,8,9,10]
            for i in xs {
                sum = sum + s.len() + s.size()
            }
            return sum
        }
    "#;
    results.push(run_benchmark("string_basics_repeat", string_basics));

    print_table(&results);
}
