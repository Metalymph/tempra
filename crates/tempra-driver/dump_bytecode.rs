use std::fs;
use tempfile::tempdir;

fn main() {
    let dir = tempdir().unwrap();
    let main_path = dir.path().join("main.tmpr");
    let main_src = "
import std.path;
pub fn main() -> Int64 {
    let res = std.path.join(\"a\", \"b\");
    return 0;
}
";
    fs::write(&main_path, main_src).unwrap();
    // Get the module
    let hash = [0u8; 32];
    let fingerprint = "fingerprint".to_string();
    let cache_path = dir.path().join("cache.tbc");
    let res = tempra_driver::compile_and_cache_module_graph(&main_path, &hash, &cache_path, &fingerprint).unwrap();
    let module = res.0;
    
    for f in &module.functions {
        println!("function: {}, is_extern: {}", f.name, f.is_extern);
        for instr in &f.code {
            println!("  {:?}", instr);
        }
    }
}
