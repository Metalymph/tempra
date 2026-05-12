use std::time::{Duration, Instant};
use std::fs;
use std::path::Path;
use tempra_driver::{run_source_hir, compile_to_bytecode};
use tempra_bytecode::BytecodeModule;

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
            cache_artifact_bytes = fs::metadata(&cache_path).map(|m| m.len() as usize).unwrap_or(0);
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
        call_threshold: 1, // Capture everything for small benchmarks
        backedge_threshold: 1,
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
        println!("| {:<20} | {:>9?} | {:>11?} | {:>10?} | {:>7?} | {:>7} B | {:>6} | {:>3} | {:>10} B |",
            r.case_name, r.hir_avg, r.vm_miss_avg, r.vm_hit_avg, r.vm_exec_avg, r.bytecode_bytes, r.instr_count, r.fn_count, r.cache_artifact_bytes);
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
        if i < 999 { array_literal.push_str(", "); }
    }
    array_literal.push(']');

    let arithmetic_loop = format!(r#"
        fn main() -> Int64 {{
            let xs = {}
            var sum = 0
            for x in xs {{
                sum = sum + x
            }}
            return sum
        }}
    "#, array_literal);
    results.push(run_benchmark("arithmetic_loop_1000", &arithmetic_loop));

    // Case 2: For-Expr Collect
    let for_expr = format!(r#"
        fn main() -> Int64 {{
            let xs = {}
            let ys = for x in xs {{ x + 1 }}
            return ys[999]
        }}
    "#, array_literal);
    results.push(run_benchmark("for_expr_collect_1000", &for_expr));

    // Case 3: Struct Access Loop
    let struct_access = format!(r#"
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
    "#, array_literal);
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
