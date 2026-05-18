use std::path::PathBuf;

pub fn check_command(path: PathBuf, json: bool) -> Result<(), String> {
    if json {
        let diags = tempra_driver::pipeline::diagnose_file(&path);
        let file_str = path.to_string_lossy();
        let diags_json: Vec<serde_json::Value> = diags
            .iter()
            .map(|d| d.to_json_value(Some(&file_str)))
            .collect();
        let payload = serde_json::json!({
            "diagnostics": diags_json
        });
        println!("{}", serde_json::to_string_pretty(&payload).unwrap());

        let has_errors = diags
            .iter()
            .any(|d| matches!(d.severity, tempra_diagnostics::Severity::Error));
        if has_errors {
            return Err("".to_string());
        }
        Ok(())
    } else {
        tempra_driver::check_file(&path).map_err(|e| format!("{}", e))
    }
}
