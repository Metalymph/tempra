use std::path::PathBuf;

pub fn symbols_command(path: PathBuf, json: bool) -> Result<(), String> {
    let symbols = tempra_driver::pipeline::extract_symbols(&path).map_err(|e| format!("{}", e))?;
    let file_str = path.to_string_lossy();

    if json {
        let symbols_json: Vec<serde_json::Value> = symbols
            .iter()
            .map(|s| s.to_json_value(Some(&file_str)))
            .collect();
        let payload = serde_json::json!({
            "symbols": symbols_json
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());
    } else {
        for s in symbols {
            let sig_str = s.signature.as_deref().unwrap_or("");
            println!("{:?}: {} ({})", s.kind, s.name, sig_str);
        }
    }
    Ok(())
}
