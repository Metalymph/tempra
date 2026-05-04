use std::path::PathBuf;

pub fn test_command(path: PathBuf) -> Result<(), String> {
    tempra_driver::pipeline::test_file(&path).map_err(|e| format!("{}", e))
}
