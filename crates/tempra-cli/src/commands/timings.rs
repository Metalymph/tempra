use serde_json::json;
use std::path::PathBuf;

/// Profile a fresh single-source VM compilation without executing the program.
///
/// JSON schema v1 is independent of the measured durations and reports
/// failed stages as attempts. This is *not* the module graph or cache-hit path.
pub fn timings_command(path: PathBuf, json_output: bool) -> Result<(), String> {
    let source =
        std::fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let hash = tempra_driver::cache::compute_source_hash(&source);
    let cache_path = tempra_driver::cache::get_cache_path(&hash);
    let (result, report) =
        tempra_driver::compile_to_bytecode_with_analysis_timed(&source, &cache_path);

    if json_output {
        let output = json!({
            "schema_version": 1,
            "mode": "fresh_single_source_vm_compile",
            "status": if result.is_ok() { "ok" } else { "error" },
            "wall_ns": report.wall_ns,
            "phases": report.samples.iter().map(|item| json!({
                "phase": item.phase.as_str(),
                "elapsed_ns": item.elapsed_ns,
                "success": item.success,
            })).collect::<Vec<_>>(),
            "error": result.as_ref().err().map(ToString::to_string),
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&output).map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "fresh single-source VM compilation: {}",
            if result.is_ok() { "ok" } else { "error" }
        );
        for sample in &report.samples {
            println!(
                "  {:<24} {:>12} ns  {}",
                sample.phase.as_str(),
                sample.elapsed_ns,
                if sample.success { "ok" } else { "failed" }
            );
        }
        println!("  {:<24} {:>12} ns", "wall_total", report.wall_ns);
    }

    match result {
        Ok(_) => Ok(()),
        Err(error) if json_output => {
            // Main preserves JSON as the only stdout output; nonzero exit
            // still signals failure.
            let _ = error;
            Err(String::new())
        }
        Err(error) => Err(error.to_string()),
    }
}
